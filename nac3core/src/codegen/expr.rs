use std::{
    cmp::min,
    collections::HashMap,
    convert::TryInto,
    iter::{once, zip},
    sync::Arc,
};

use inkwell::{
    IntPredicate,
    basic_block::BasicBlock,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValueEnum, IntValue, PointerValue, StructValue},
};
use itertools::{Itertools, izip};

use nac3parser::ast::{
    self, Boolop, Cmpop, Comprehension, Constant, Expr, ExprKind, Keyword, Location, Operator,
    StrRef, Unaryop,
};

use super::{
    CodeGenContext, CodeGenTask, CodeGenerator, bool_to_int_type,
    concrete_type::{ConcreteFuncArg, ConcreteTypeEnum, ConcreteTypeStore},
    gen_in_range_check, get_llvm_abi_type, get_llvm_type,
    irrt::{
        calculate_len_for_slice_range, call_string_eq, handle_slice_indices, integer_power,
        list_slice_assignment,
    },
    llvm_intrinsics::{
        call_expect, call_float_floor, call_float_pow, call_float_powi, call_int_smax,
        call_memcpy_generic,
    },
    macros::codegen_unreachable,
    stmt::{
        gen_for_callback_incrementing, gen_if_callback, gen_if_else_expr_callback, gen_raise,
        gen_var,
    },
    types::{
        ExceptionType, ListType, OptionType, RangeType, StringType, TupleType, ndarray::NDArrayType,
    },
    values::{
        ArrayLikeIndexer, ArrayLikeValue, ListValue, ProxyValue, RangeValue,
        UntypedArrayLikeAccessor,
        ndarray::{NDArrayOut, RustNDIndex, ScalarOrNDArray},
    },
};
use crate::{
    codegen::{bool_to_i1, bool_to_i8, llvm_fns::FunctionDecl},
    symbol_resolver::{StaticValue, SymbolValue, ValueEnum},
    toplevel::{
        DefinitionId, FunAttribute, TopLevelDef,
        composer::erase_expr_type,
        helper::{PrimDef, arraylike_flatten_element_type, extract_ndims},
        numpy::unpack_ndarray_var_tys,
    },
    typecheck::{
        magic_methods::{Binop, BinopVariant, HasOpInfo},
        typedef::{FunSignature, Type, TypeEnum, TypeVarId, Unifier, VarMap},
    },
};

#[derive(Clone)]
pub struct RtValue<'ctx> {
    pub ty: Type,
    // None if `ty` is `primitives.none`,
    // or when `ty` is free (e.g. return type of a function that does nothing
    // but raises an exception).
    pub val: Option<ValueEnum<'ctx>>,
}

impl<'ctx> RtValue<'ctx> {
    #[must_use]
    pub fn r#static(ty: Type, val: Arc<dyn StaticValue + Send + Sync>) -> Self {
        Self { ty, val: Some(ValueEnum::Static(val)) }
    }
    #[must_use]
    pub fn dynamic(ty: Type, val: BasicValueEnum<'ctx>) -> Self {
        Self { ty, val: Some(val.into()) }
    }
    #[must_use]
    pub const fn none(none: Type) -> Self {
        Self { ty: none, val: None }
    }
    pub fn to_basic_value_enum(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        self.val.as_ref().unwrap().to_basic_value_enum(ctx, self.ty)
    }
}

pub fn get_subst_key(
    unifier: &mut Unifier,
    obj: Option<Type>,
    fun_vars: &VarMap,
    filter: Option<&Vec<TypeVarId>>,
) -> String {
    let mut vars = obj
        .map(|ty| {
            if let TypeEnum::TObj { params, .. } = &*unifier.get_ty(ty) {
                params.clone()
            } else {
                unreachable!()
            }
        })
        .unwrap_or_default();
    vars.extend(fun_vars);
    let sorted = vars.keys().filter(|id| filter.is_none_or(|v| v.contains(id))).sorted();
    sorted
        .map(|id| {
            unifier.internal_stringify(
                vars[id],
                &mut |id| id.to_string(),
                &mut |id| id.to_string(),
                &mut None,
            )
        })
        .join(", ")
}

impl<'ctx> CodeGenContext<'ctx, '_> {
    /// Builds a sequence of `getelementptr` and `load` instructions which stores the value of a
    /// struct field into an LLVM value.
    pub fn build_gep_and_load(
        &mut self,
        ptr: PointerValue<'ctx>,
        index: &[IntValue<'ctx>],
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let gep = unsafe { self.builder.build_gep(ptr, index, "") }.unwrap();
        self.builder.build_load(gep, name.unwrap_or_default()).unwrap()
    }

    /// Builds a sequence of `getelementptr inbounds` and `load` instructions which stores the value
    /// of a struct field into an LLVM value.
    ///
    /// Any out-of-bounds accesses to `ptr` will return in a `poison` value.
    pub fn build_in_bounds_gep_and_load(
        &mut self,
        ptr: PointerValue<'ctx>,
        index: &[IntValue<'ctx>],
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let gep = unsafe { self.builder.build_in_bounds_gep(ptr, index, "") }.unwrap();
        self.builder.build_load(gep, name.unwrap_or_default()).unwrap()
    }

    fn get_subst_key(
        &mut self,
        obj: Option<Type>,
        fun: &FunSignature,
        filter: Option<&Vec<TypeVarId>>,
    ) -> String {
        get_subst_key(&mut self.unifier, obj, &fun.vars, filter)
    }

    /// Checks the field and attributes of classes
    /// Returns the index of attr in class fields otherwise returns the attribute value
    pub fn get_attr_index(&mut self, ty: Type, attr: StrRef) -> (usize, Option<Constant>) {
        let obj_id = match &*self.unifier.get_ty(ty) {
            TypeEnum::TObj { obj_id, .. } => *obj_id,
            // we cannot have other types, virtual type should be handled by function calls
            _ => codegen_unreachable!(self),
        };
        let def = &self.top_level.definitions.read()[obj_id.0];
        let (index, value) = if let TopLevelDef::Class { fields, attributes, .. } = &*def.read() {
            fields.iter().find_position(|x| x.0 == attr).map_or_else(
                || {
                    let attribute_index = attributes.iter().find_position(|x| x.0 == attr).unwrap();
                    (attribute_index.0, Some(attribute_index.1.2.clone()))
                },
                |field_index| (field_index.0, None),
            )
        } else {
            codegen_unreachable!(self)
        };
        (index, value)
    }

    pub fn get_attr_index_object(&mut self, ty: Type, attr: StrRef) -> usize {
        match &*self.unifier.get_ty(ty) {
            TypeEnum::TObj { fields, .. } => {
                fields.iter().find_position(|x| *x.0 == attr).unwrap().0
            }
            _ => codegen_unreachable!(self),
        }
    }

    pub fn gen_symbol_val(&mut self, val: &SymbolValue, ty: Type) -> BasicValueEnum<'ctx> {
        match val {
            SymbolValue::I32(v) => self.i32.const_int(*v as u64, true).into(),
            SymbolValue::I64(v) => self.i64.const_int(*v as u64, true).into(),
            SymbolValue::U32(v) => self.i32.const_int(u64::from(*v), false).into(),
            SymbolValue::U64(v) => self.i64.const_int(*v, false).into(),
            SymbolValue::Bool(v) => self.i8.const_int(u64::from(*v), true).into(),
            SymbolValue::Double(v) => self.f64.const_float(*v).into(),
            SymbolValue::Str(v) => {
                StringType::new(self).construct_constant(self, v, None).as_abi_value(self).into()
            }
            SymbolValue::Tuple(ls) => {
                let vals = ls.iter().map(|v| self.gen_symbol_val(v, ty)).collect_vec();
                let fields = vals.iter().map(BasicValueEnum::get_type).collect_vec();
                TupleType::new(self, &fields)
                    .construct_from_objects(self, vals, Some("tup_val"))
                    .as_abi_value(self)
                    .into()
            }
            SymbolValue::OptionSome(v) => {
                let val = self.gen_symbol_val(v, ty);
                OptionType::from_unifier_type(self, ty)
                    .construct_some_value(self, &val, None)
                    .as_abi_value(self)
                    .into()
            }
            SymbolValue::OptionNone => OptionType::from_unifier_type(self, ty)
                .construct_empty(self, None)
                .as_abi_value(self)
                .into(),
        }
    }

    /// See [`get_llvm_type`].
    pub fn get_llvm_type(&mut self, ty: Type) -> BasicTypeEnum<'ctx> {
        get_llvm_type(&self.inner, &mut self.unifier, self.top_level, &mut self.type_cache, ty)
    }

    /// See [`get_llvm_abi_type`].
    pub fn get_llvm_abi_type(&mut self, ty: Type) -> BasicTypeEnum<'ctx> {
        get_llvm_abi_type(
            &self.inner,
            &mut self.unifier,
            self.top_level,
            &mut self.type_cache,
            &self.primitives,
            ty,
        )
    }

    /// Generates an LLVM variable for a [constant value][value] with a given [type][ty].
    pub fn gen_const(&mut self, value: &Constant, ty: Type) -> Option<BasicValueEnum<'ctx>> {
        match value {
            Constant::Bool(v) => {
                assert!(self.unifier.unioned(ty, self.primitives.bool));
                let ty = self.i8;
                Some(ty.const_int(u64::from(*v), false).into())
            }
            Constant::Int(val) => {
                let ty = if self.unifier.unioned(ty, self.primitives.int32)
                    || self.unifier.unioned(ty, self.primitives.uint32)
                {
                    self.i32
                } else if self.unifier.unioned(ty, self.primitives.int64)
                    || self.unifier.unioned(ty, self.primitives.uint64)
                {
                    self.i64
                } else {
                    codegen_unreachable!(self)
                };
                Some(ty.const_int(*val as u64, false).into())
            }
            Constant::Float(v) => {
                assert!(self.unifier.unioned(ty, self.primitives.float));
                let ty = self.f64;
                Some(ty.const_float(*v).into())
            }
            Constant::Tuple(v) => {
                let ty = self.unifier.get_ty(ty);
                let (types, is_vararg_ctx) = if let TypeEnum::TTuple { ty, is_vararg_ctx } = &*ty {
                    (ty.clone(), *is_vararg_ctx)
                } else {
                    codegen_unreachable!(self)
                };
                let values =
                    zip(types, v.iter()).map_while(|(ty, v)| self.gen_const(v, ty)).collect_vec();

                if is_vararg_ctx || values.len() == v.len() {
                    let types = values.iter().map(BasicValueEnum::get_type).collect_vec();
                    let ty = self.ctx.struct_type(&types, false);
                    Some(ty.const_named_struct(&values).into())
                } else {
                    None
                }
            }
            Constant::Str(v) => {
                assert!(self.unifier.unioned(ty, self.primitives.str));
                if let Some(v) = self.const_strings.get(v) {
                    Some(*v)
                } else {
                    let val = StringType::new(self)
                        .construct_constant(self, v, None)
                        .as_abi_value(self)
                        .into();
                    self.const_strings.insert(v.clone(), val);
                    Some(val)
                }
            }
            Constant::Ellipsis => {
                let msg = self.gen_string("NotImplementedError");

                self.raise_exn(
                    "0:NotImplementedError",
                    msg.into(),
                    [None, None, None],
                    self.current_loc,
                );

                None
            }
            _ => codegen_unreachable!(self),
        }
    }

    /// Generates a binary operation `op` between two integral operands `lhs` and `rhs`.
    pub fn gen_int_ops(
        &mut self,
        op: Operator,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        signed: bool,
    ) -> BasicValueEnum<'ctx> {
        let (BasicValueEnum::IntValue(lhs), BasicValueEnum::IntValue(rhs)) = (lhs, rhs) else {
            codegen_unreachable!(self)
        };
        let float = self.f64;
        match (op, signed) {
            (Operator::Add, _) => {
                self.builder.build_int_add(lhs, rhs, "add").map(Into::into).unwrap()
            }
            (Operator::Sub, _) => {
                self.builder.build_int_sub(lhs, rhs, "sub").map(Into::into).unwrap()
            }
            (Operator::Mult, _) => {
                self.builder.build_int_mul(lhs, rhs, "mul").map(Into::into).unwrap()
            }
            (Operator::Div, true) => {
                let left = self.builder.build_signed_int_to_float(lhs, float, "i2f").unwrap();
                let right = self.builder.build_signed_int_to_float(rhs, float, "i2f").unwrap();
                self.builder.build_float_div(left, right, "fdiv").map(Into::into).unwrap()
            }
            (Operator::Div, false) => {
                let left = self.builder.build_unsigned_int_to_float(lhs, float, "i2f").unwrap();
                let right = self.builder.build_unsigned_int_to_float(rhs, float, "i2f").unwrap();
                self.builder.build_float_div(left, right, "fdiv").map(Into::into).unwrap()
            }
            (Operator::Mod, true) => {
                self.builder.build_int_signed_rem(lhs, rhs, "mod").map(Into::into).unwrap()
            }
            (Operator::Mod, false) => {
                self.builder.build_int_unsigned_rem(lhs, rhs, "mod").map(Into::into).unwrap()
            }
            (Operator::BitOr, _) => self.builder.build_or(lhs, rhs, "or").map(Into::into).unwrap(),
            (Operator::BitXor, _) => {
                self.builder.build_xor(lhs, rhs, "xor").map(Into::into).unwrap()
            }
            (Operator::BitAnd, _) => {
                self.builder.build_and(lhs, rhs, "and").map(Into::into).unwrap()
            }

            // Sign-ness of bitshift operators are always determined by the left operand
            (Operator::LShift | Operator::RShift, signed) => {
                // RHS operand is always 32 bits
                assert_eq!(rhs.get_type().get_bit_width(), 32);

                let common_type = lhs.get_type();
                let rhs = if common_type.get_bit_width() > 32 {
                    if signed {
                        self.builder.build_int_s_extend(rhs, common_type, "").unwrap()
                    } else {
                        self.builder.build_int_z_extend(rhs, common_type, "").unwrap()
                    }
                } else {
                    rhs
                };

                let rhs_gez = self
                    .builder
                    .build_int_compare(IntPredicate::SGE, rhs, common_type.const_zero(), "")
                    .unwrap();
                self.make_assert(
                    rhs_gez,
                    "ValueError",
                    "negative shift count",
                    [None, None, None],
                    self.current_loc,
                );

                match op {
                    Operator::LShift => {
                        self.builder.build_left_shift(lhs, rhs, "lshift").map(Into::into).unwrap()
                    }
                    Operator::RShift => self
                        .builder
                        .build_right_shift(lhs, rhs, signed, "rshift")
                        .map(Into::into)
                        .unwrap(),
                    _ => codegen_unreachable!(self),
                }
            }

            (Operator::FloorDiv, true) => {
                self.builder.build_int_signed_div(lhs, rhs, "floordiv").map(Into::into).unwrap()
            }
            (Operator::FloorDiv, false) => {
                self.builder.build_int_unsigned_div(lhs, rhs, "floordiv").map(Into::into).unwrap()
            }
            (Operator::Pow, s) => integer_power(self, lhs, rhs, s).into(),
            // special implementation?
            (Operator::MatMult, _) => codegen_unreachable!(self),
        }
    }

    /// Generates a binary operation `op` between two floating-point operands `lhs` and `rhs`.
    pub fn gen_float_ops(
        &mut self,
        op: Operator,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let (BasicValueEnum::FloatValue(lhs), BasicValueEnum::FloatValue(rhs)) = (lhs, rhs) else {
            codegen_unreachable!(
                self,
                "Expected (FloatValue, FloatValue), got ({}, {})",
                lhs.get_type(),
                rhs.get_type()
            )
        };
        match op {
            Operator::Add => {
                self.builder.build_float_add(lhs, rhs, "fadd").map(Into::into).unwrap()
            }
            Operator::Sub => {
                self.builder.build_float_sub(lhs, rhs, "fsub").map(Into::into).unwrap()
            }
            Operator::Mult => {
                self.builder.build_float_mul(lhs, rhs, "fmul").map(Into::into).unwrap()
            }
            Operator::Div => {
                self.builder.build_float_div(lhs, rhs, "fdiv").map(Into::into).unwrap()
            }
            Operator::Mod => {
                self.builder.build_float_rem(lhs, rhs, "fmod").map(Into::into).unwrap()
            }
            Operator::FloorDiv => {
                let div = self.builder.build_float_div(lhs, rhs, "fdiv").unwrap();
                call_float_floor(self, div, Some("floor")).into()
            }
            Operator::Pow => call_float_pow(self, lhs, rhs, Some("f_pow")).into(),
            // special implementation?
            _ => unimplemented!(),
        }
    }

    fn build_call_or_invoke_impl(
        &self,
        fun: &FunctionDecl<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        call_name: &str,
        unwind_target: Option<BasicBlock<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        let loc = self.debug_info.0.create_debug_location(
            self.ctx,
            self.current_loc.row as u32,
            self.current_loc.column as u32,
            self.debug_info.2,
            None,
        );
        self.builder.set_current_debug_location(loc);

        let alloca = |ty| gen_var(self, ty, Some(call_name)).unwrap();

        unwind_target.map_or_else(
            || {
                let args: Vec<_> = args.iter().map(|v| (*v).into()).collect();
                self.fn_store.do_call(
                    fun,
                    &self.builder,
                    &args,
                    |value, args| self.builder.build_call(value, args, call_name).unwrap(),
                    alloca,
                )
            },
            |target| {
                let current = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let then_block =
                    self.ctx.append_basic_block(current, &format!("after.{call_name}"));
                let result = self.fn_store.do_call(
                    fun,
                    &self.builder,
                    args,
                    |value, args| {
                        self.builder
                            .build_invoke(value, args, then_block, target, call_name)
                            .unwrap()
                    },
                    alloca,
                );
                self.builder.position_at_end(then_block);
                result
            },
        )
    }

    /// Calls a declared function.
    pub fn build_call_or_invoke(
        &self,
        fun: &FunctionDecl<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        call_name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.build_call_or_invoke_impl(fun, args, call_name, self.unwind_target)
    }

    /// Calls a declared function, ignoring unwind info.
    pub fn build_call(
        &self,
        fun: &FunctionDecl<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        call_name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.build_call_or_invoke_impl(fun, args, call_name, None)
    }

    /// Helper function for generating a LLVM variable storing a [String].
    pub fn gen_string<S>(&mut self, s: S) -> StructValue<'ctx>
    where
        S: Into<String>,
    {
        self.gen_const(&Constant::Str(s.into()), self.primitives.str)
            .map(BasicValueEnum::into_struct_value)
            .unwrap()
    }

    pub fn raise_exn(
        &mut self,
        name: &str,
        msg: BasicValueEnum<'ctx>,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let llvm_i32 = self.i32;
        let llvm_i64 = self.i64;
        let llvm_exn = ExceptionType::get_instance(self);

        let zelf = if let Some(exception_val) = self.exception_val {
            llvm_exn.map_pointer_value(exception_val, Some("exn"))
        } else {
            let zelf = llvm_exn.alloca_var(self, Some("exn"));
            self.exception_val = Some(zelf.as_abi_value(self));
            zelf
        };

        let id = self.resolver.get_string_id(name);
        zelf.store_name(self, llvm_i32.const_int(id as u64, false));
        zelf.store_message(self, msg.into_struct_value());
        zelf.store_params(
            self,
            params
                .iter()
                .map(|p| {
                    p.map_or(llvm_i64.const_zero(), |v| {
                        self.builder.build_int_s_extend(v, self.i64, "sext").unwrap()
                    })
                })
                .collect_array()
                .as_ref()
                .unwrap(),
        );
        gen_raise(self, Some(&zelf), loc);
    }

    pub fn make_assert(
        &mut self,
        cond: IntValue<'ctx>,
        err_name: &str,
        err_msg: &str,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let err_msg = self.gen_string(err_msg);
        self.make_assert_impl(cond, err_name, err_msg.into(), params, loc);
    }

    pub fn make_assert_impl(
        &mut self,
        cond: IntValue<'ctx>,
        err_name: &str,
        err_msg: BasicValueEnum<'ctx>,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let i1 = self.i1;
        let i1_true = i1.const_all_ones();
        // we assume that the condition is most probably true, so the normal path is the most
        // probable path
        // even if this assumption is violated, it does not matter as exception unwinding is
        // slow anyway...
        let cond = call_expect(self, cond, i1_true, Some("expect"));
        let current_bb = self.builder.get_insert_block().unwrap();
        let current_fun = current_bb.get_parent().unwrap();
        let then_block = self.ctx.insert_basic_block_after(current_bb, "succ");
        let exn_block = self.ctx.append_basic_block(current_fun, "fail");
        self.builder.build_conditional_branch(cond, then_block, exn_block).unwrap();
        self.builder.position_at_end(exn_block);
        self.raise_exn(err_name, err_msg, params, loc);
        self.builder.position_at_end(then_block);
    }
}

/// See [`CodeGenerator::gen_constructor`].
pub fn gen_constructor<'ctx, 'a, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    signature: &FunSignature,
    def: &TopLevelDef,
    params: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let TopLevelDef::Class { methods, .. } = def else { codegen_unreachable!(ctx) };

    // TODO: what about other fields that require alloca?
    let fun_id = methods.iter().find(|method| method.0 == "__init__".into()).map(|method| method.2);
    let ty = ctx.get_llvm_type(signature.ret).into_pointer_type();
    let zelf_ty: BasicTypeEnum = ty.get_element_type().try_into().unwrap();
    let zelf: BasicValueEnum<'ctx> =
        ctx.builder.build_alloca(zelf_ty, "alloca").map(Into::into).unwrap();
    // call `__init__` if there is one
    if let Some(fun_id) = fun_id {
        let mut sign = signature.clone();
        sign.ret = ctx.primitives.none;
        generator.gen_call(ctx, Some((signature.ret, zelf.into())), (&sign, fun_id), params)?;
    }
    Ok(zelf)
}

/// See [`CodeGenerator::gen_func_instance`].
pub fn gen_func_instance(
    ctx: &mut CodeGenContext<'_, '_>,
    obj: Option<Type>,
    fun: (&FunSignature, &mut TopLevelDef, String),
    id: usize,
) -> Result<String, String> {
    let (
        sign,
        TopLevelDef::Function {
            name, instance_to_symbol, instance_to_stmt, var_id, resolver, ..
        },
        key,
    ) = fun
    else {
        codegen_unreachable!(ctx)
    };

    if let Some(sym) = instance_to_symbol.get(&key) {
        return Ok(sym.clone());
    }
    let symbol = format!("{}.{}", name, instance_to_symbol.len());
    instance_to_symbol.insert(key, symbol.clone());
    let mut filter = var_id.clone();
    if let Some(obj_ty) = &obj
        && let TypeEnum::TObj { params, .. } = &*ctx.unifier.get_ty(*obj_ty)
    {
        filter.extend(params.keys());
    }
    let key = ctx.get_subst_key(obj, sign, Some(&filter));
    let instance = instance_to_stmt.get(&key).unwrap();

    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();

    let subst = sign
        .vars
        .iter()
        .map(|(id, ty)| {
            (
                *instance.subst.get(id).unwrap(),
                store.from_unifier_type(&mut ctx.unifier, &ctx.primitives, *ty, &mut cache),
            )
        })
        .collect();

    let mut signature = store.from_signature(&mut ctx.unifier, &ctx.primitives, sign, &mut cache);
    let ConcreteTypeEnum::TFunc { args, .. } = &mut signature else { codegen_unreachable!(ctx) };

    if let Some(obj) = obj {
        let zelf = store.from_unifier_type(&mut ctx.unifier, &ctx.primitives, obj, &mut cache);

        args.insert(
            0,
            ConcreteFuncArg {
                name: "self".into(),
                ty: zelf,
                default_value: None,
                is_vararg: false,
            },
        );
    }

    let signature = store.add_cty(signature);

    ctx.registry.add_task(CodeGenTask {
        symbol_name: symbol.clone(),
        body: instance.body.clone(),
        export_symbol: false,
        resolver: resolver.as_ref().unwrap().clone(),
        calls: instance.calls.clone(),
        subst,
        signature,
        store,
        unifier_index: instance.unifier_id,
        id,
    });
    Ok(symbol)
}

/// See [`CodeGenerator::gen_call`].
pub fn gen_call<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    params: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let definition = ctx.top_level.definitions.read().get(fun.1.0).cloned().unwrap();
    let id;
    let key;
    let is_extern;
    let param_vals;

    let sign = fun.0;
    let has_varargs = sign.args.iter().any(|arg| arg.is_vararg);

    let symbol = {
        // make sure this lock guard is dropped at the end of this scope...
        let def = definition.read();
        match &*def {
            TopLevelDef::Function {
                instance_to_symbol,
                instance_to_stmt,
                codegen_callback,
                ..
            } => {
                if let Some(callback) = codegen_callback {
                    return callback.run(ctx, obj, fun, params);
                }
                let old_key = ctx.get_subst_key(obj.as_ref().map(|a| a.0), sign, None);
                let mut keys = sign.args.clone();
                let mut mapping = HashMap::<_, ValueEnum>::new();

                for (key, value) in params {
                    mapping.insert(key.unwrap_or_else(|| keys.remove(0).name), value);
                }

                // default value handling
                for k in keys {
                    mapping.entry(k.name).or_insert_with(|| {
                        ctx.gen_symbol_val(&k.default_value.unwrap(), k.ty).into()
                    });
                }

                // reorder the parameters
                let mut real_params = fun
                    .0
                    .args
                    .iter()
                    .map(|arg| (mapping.remove(&arg.name).unwrap(), arg.ty))
                    .collect_vec();
                if let Some(obj) = &obj {
                    real_params.insert(0, (obj.1.clone(), obj.0));
                }

                let static_params = real_params
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (v, _))| {
                        if let ValueEnum::Static(s) = v { Some((i, s.clone())) } else { None }
                    })
                    .collect_vec();
                id = {
                    let ids = static_params
                        .iter()
                        .map(|(i, v)| (*i, v.get_unique_identifier()))
                        .collect_vec();
                    let mut store = ctx.static_value_store.lock();
                    store.get_or_insert(ids, || static_params.into_iter().collect())
                };
                is_extern = instance_to_stmt.is_empty();
                // special case: extern functions
                key = if is_extern { String::new() } else { format!("{id}:{old_key}") };
                param_vals = real_params
                    .into_iter()
                    .map(|(p, t)| Ok::<_, String>((p.to_basic_value_enum(ctx, t)?, t)))
                    .collect::<Result<Vec<_>, _>>()?;
                instance_to_symbol.get(&key).cloned().ok_or_else(String::new)
            }
            TopLevelDef::Class { .. } => {
                return Ok(Some(generator.gen_constructor(ctx, sign, &def, params)?));
            }
            TopLevelDef::Module { .. } => unreachable!(),
        }
    }
    .or_else(|_: String| {
        let obj_ty = obj.as_ref().map(|x| x.0);
        generator.gen_func_instance(ctx, obj_ty, (sign, &mut *definition.write(), key), id)
    })?;

    let ret_type = if ctx.unifier.unioned(sign.ret, ctx.primitives.none) {
        None
    } else {
        Some(ctx.get_llvm_abi_type(sign.ret))
    };
    let args_type = obj
        .iter()
        .map(|a| a.0)
        .chain(sign.args.iter().map(|a| a.ty))
        .map(|ty| ctx.get_llvm_abi_type(ty));

    // We must declare the function before codegen.
    let f = if is_extern {
        let args_type = &args_type.collect_vec();
        ctx.declare_external(&symbol, ret_type, args_type, has_varargs, &[])
    } else {
        // TODO(ivan): reimplement support for variadic arguments as passing lists/tuples
        assert!(!has_varargs, "not yet implemented: varargs");
        let args_type = &args_type.map(Into::into).collect_vec();
        ctx.declare_internal(&symbol, ret_type, args_type, false).0
    };

    // Convert boolean parameter values into i1
    let param_vals = param_vals
        .into_iter()
        .map(|(v, t)| {
            if ctx.unifier.unioned(ctx.primitives.bool, t) {
                bool_to_i1(ctx, v.into_int_value()).into()
            } else {
                v
            }
        })
        .collect_vec();

    // The function instance should have already been constructed (at least declared) here.

    Ok(ctx.build_call_or_invoke(&f, &param_vals, "call"))
}

/// Generates three LLVM variables representing the start, stop, and step values of a [range] class
/// respectively.
pub fn destructure_range<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    range: RangeValue<'ctx>,
) -> (IntValue<'ctx>, IntValue<'ctx>, IntValue<'ctx>) {
    let start = range.load_start(ctx, None);
    let end = range.load_end(ctx, None);
    let step = range.load_step(ctx, None);
    (start, end, step)
}

/// Generates LLVM IR for a [list comprehension expression][expr].
pub fn gen_comprehension<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    expr: &Expr<Option<Type>>,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let ExprKind::ListComp { elt, generators } = &expr.node else { codegen_unreachable!(ctx) };

    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();

    let init_bb = ctx.ctx.append_basic_block(current, "listcomp.init");
    let test_bb = ctx.ctx.append_basic_block(current, "listcomp.test");
    let body_bb = ctx.ctx.append_basic_block(current, "listcomp.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "listcomp.cont");

    ctx.builder.build_unconditional_branch(init_bb).unwrap();

    ctx.builder.position_at_end(init_bb);

    let Comprehension { target, iter, ifs, .. } = &generators[0];

    let iter_ty = iter.custom.unwrap();
    let iter_val = generator.gen_expr(ctx, iter)?.to_basic_value_enum(ctx)?;

    let int32 = ctx.i32;
    let size_t = ctx.size_t;
    let zero_size_t = size_t.const_zero();
    let zero_32 = int32.const_zero();

    let index = gen_var(ctx, size_t.into(), Some("index.addr"))?;
    ctx.builder.build_store(index, zero_size_t).unwrap();

    let elem_ty = ctx.get_llvm_type(elt.custom.unwrap());
    let list;

    match &*ctx.unifier.get_ty(iter_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.range.obj_id(&ctx.unifier).unwrap() =>
        {
            let iter_val =
                RangeType::new(ctx).map_pointer_value(iter_val.into_pointer_value(), Some("range"));
            let (start, stop, step) = destructure_range(ctx, iter_val);
            let diff = ctx.builder.build_int_sub(stop, start, "diff").unwrap();
            // add 1 to the length as the value is rounded to zero
            // the length may be 1 more than the actual length if the division is exact, but the
            // length is a upper bound only anyway so it does not matter.
            let length = ctx.builder.build_int_signed_div(diff, step, "div").unwrap();
            let length =
                ctx.builder.build_int_add(length, int32.const_int(1, false), "add1").unwrap();
            // in case length is non-positive
            let is_valid =
                ctx.builder.build_int_compare(IntPredicate::SGT, length, zero_32, "check").unwrap();

            let list_alloc_size = ctx
                .builder
                .build_select(
                    is_valid,
                    ctx.builder
                        .build_int_z_extend_or_bit_cast(length, size_t, "z_ext_len")
                        .unwrap(),
                    zero_size_t,
                    "listcomp.alloc_size",
                )
                .unwrap();
            list = ListType::new(ctx, &elem_ty).construct(
                ctx,
                list_alloc_size.into_int_value(),
                Some("listcomp"),
            );

            let i = generator.gen_store_target(ctx, target, Some("i.addr"))?.unwrap();
            ctx.builder
                .build_store(i, ctx.builder.build_int_sub(start, step, "start_init").unwrap())
                .unwrap();

            ctx.builder
                .build_conditional_branch(
                    gen_in_range_check(ctx, start, stop, step),
                    test_bb,
                    cont_bb,
                )
                .unwrap();

            ctx.builder.position_at_end(test_bb);
            // add and test
            let tmp = ctx
                .builder
                .build_int_add(
                    ctx.builder.build_load(i, "i").map(BasicValueEnum::into_int_value).unwrap(),
                    step,
                    "start_loop",
                )
                .unwrap();
            ctx.builder.build_store(i, tmp).unwrap();
            ctx.builder
                .build_conditional_branch(
                    gen_in_range_check(ctx, tmp, stop, step),
                    body_bb,
                    cont_bb,
                )
                .unwrap();

            ctx.builder.position_at_end(body_bb);
        }
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            let length = ctx
                .build_gep_and_load(
                    iter_val.into_pointer_value(),
                    &[zero_size_t, int32.const_int(1, false)],
                    Some("length"),
                )
                .into_int_value();
            list = ListType::new(ctx, &elem_ty).construct(ctx, length, Some("listcomp"));

            let counter = gen_var(ctx, size_t.into(), Some("counter.addr"))?;
            // counter = -1
            ctx.builder.build_store(counter, size_t.const_all_ones()).unwrap();
            ctx.builder.build_unconditional_branch(test_bb).unwrap();

            ctx.builder.position_at_end(test_bb);
            let tmp =
                ctx.builder.build_load(counter, "i").map(BasicValueEnum::into_int_value).unwrap();
            let tmp = ctx.builder.build_int_add(tmp, size_t.const_int(1, false), "inc").unwrap();
            ctx.builder.build_store(counter, tmp).unwrap();
            let cmp = ctx.builder.build_int_compare(IntPredicate::SLT, tmp, length, "cmp").unwrap();
            ctx.builder.build_conditional_branch(cmp, body_bb, cont_bb).unwrap();

            ctx.builder.position_at_end(body_bb);
            let arr_ptr = ctx
                .build_gep_and_load(
                    iter_val.into_pointer_value(),
                    &[zero_size_t, zero_32],
                    Some("arr.addr"),
                )
                .into_pointer_value();
            let val = ctx.build_gep_and_load(arr_ptr, &[tmp], Some("val"));
            generator.gen_assign(ctx, target, &val.into(), elt.custom.unwrap())?;
        }
        _ => {
            panic!(
                "unsupported list comprehension iterator type: {}",
                ctx.unifier.stringify(iter_ty)
            );
        }
    }

    // Emits the content of `cont_bb`
    let emit_cont_bb = |ctx: &mut CodeGenContext<'ctx, '_>, list: ListValue<'ctx>| {
        ctx.builder.position_at_end(cont_bb);
        list.store_size(
            ctx,
            ctx.builder.build_load(index, "index").map(BasicValueEnum::into_int_value).unwrap(),
        );
    };

    for cond in ifs {
        let result = generator.gen_expr(ctx, cond)?.to_basic_value_enum(ctx)?.into_int_value();
        let result = bool_to_i1(ctx, result);
        let succ = ctx.ctx.append_basic_block(current, "then");
        ctx.builder.build_conditional_branch(result, succ, test_bb).unwrap();

        ctx.builder.position_at_end(succ);
    }

    let i = ctx.builder.build_load(index, "i").map(BasicValueEnum::into_int_value).unwrap();
    let elem_ptr = unsafe { list.data().ptr_offset_unchecked(ctx, &i, Some("elem_ptr")) };
    let val = generator.gen_expr(ctx, elt)?.to_basic_value_enum(ctx)?;
    ctx.builder.build_store(elem_ptr, val).unwrap();
    ctx.builder
        .build_store(
            index,
            ctx.builder.build_int_add(i, size_t.const_int(1, false), "inc").unwrap(),
        )
        .unwrap();
    ctx.builder.build_unconditional_branch(test_bb).unwrap();

    emit_cont_bb(ctx, list);

    Ok(Some(list.as_abi_value(ctx).into()))
}

pub fn gen_prim_binop_expr<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: (&Option<Type>, BasicValueEnum<'ctx>),
    op: Binop,
    right: (&Option<Type>, BasicValueEnum<'ctx>),
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let (left_ty, left_val) = left;
    let (right_ty, right_val) = right;

    let ty1 = ctx.unifier.get_representative(left_ty.unwrap());
    let ty2 = ctx.unifier.get_representative(right_ty.unwrap());

    // we can directly compare the types, because we've got their representatives
    // which would be unchanged until further unification, which we would never do
    // when doing code generation for function instances
    let result = if ty1 == ty2 && [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1) {
        Ok(ctx.gen_int_ops(op.base, left_val, right_val, true))
    } else if ty1 == ty2 && [ctx.primitives.uint32, ctx.primitives.uint64].contains(&ty1) {
        Ok(ctx.gen_int_ops(op.base, left_val, right_val, false))
    } else if [Operator::LShift, Operator::RShift].contains(&op.base) {
        let signed = [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1);
        Ok(ctx.gen_int_ops(op.base, left_val, right_val, signed))
    } else if ty1 == ty2 && ctx.primitives.float == ty1 {
        Ok(ctx.gen_float_ops(op.base, left_val, right_val))
    } else if ty1 == ctx.primitives.float && ty2 == ctx.primitives.int32 {
        // Pow is the only operator that would pass typecheck between float and int
        assert_eq!(op.base, Operator::Pow);
        let res = call_float_powi(
            ctx,
            left_val.into_float_value(),
            right_val.into_int_value(),
            Some("f_pow_i"),
        );
        Ok(res.into())
    } else if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id())
        || ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id())
    {
        let llvm_usize = ctx.size_t;

        if op.variant == BinopVariant::AugAssign {
            todo!("Augmented assignment operators not implemented for lists")
        }

        match op.base {
            Operator::Add => {
                debug_assert_eq!(ty1.obj_id(&ctx.unifier), Some(PrimDef::List.id()));
                debug_assert_eq!(ty2.obj_id(&ctx.unifier), Some(PrimDef::List.id()));

                let elem_ty1 =
                    if let TypeEnum::TObj { params, .. } = &*ctx.unifier.get_ty_immutable(ty1) {
                        ctx.unifier.get_representative(*params.iter().next().unwrap().1)
                    } else {
                        codegen_unreachable!(ctx)
                    };
                let elem_ty2 =
                    if let TypeEnum::TObj { params, .. } = &*ctx.unifier.get_ty_immutable(ty2) {
                        ctx.unifier.get_representative(*params.iter().next().unwrap().1)
                    } else {
                        codegen_unreachable!(ctx)
                    };
                debug_assert!(ctx.unifier.unioned(elem_ty1, elem_ty2));

                let llvm_elem_ty = ctx.get_llvm_type(elem_ty1);
                let sizeof_elem = ctx
                    .builder
                    .build_int_truncate_or_bit_cast(llvm_elem_ty.size_of().unwrap(), llvm_usize, "")
                    .unwrap();

                let lhs =
                    ListValue::from_pointer_value(left_val.into_pointer_value(), llvm_usize, None);
                let rhs =
                    ListValue::from_pointer_value(right_val.into_pointer_value(), llvm_usize, None);

                let lhs_size = lhs.load_size(ctx, None);
                let rhs_size = rhs.load_size(ctx, None);
                let size = ctx.builder.build_int_add(lhs_size, rhs_size, "").unwrap();

                let new_list = ListType::new(ctx, &llvm_elem_ty).construct(ctx, size, None);

                let lhs_len = ctx.builder.build_int_mul(lhs_size, sizeof_elem, "").unwrap();
                let rhs_len = ctx.builder.build_int_mul(rhs_size, sizeof_elem, "").unwrap();

                let list_ptr = new_list.data().base_ptr(ctx);
                call_memcpy_generic(ctx, list_ptr, lhs.data().base_ptr(ctx), lhs_len);

                let list_ptr =
                    unsafe { new_list.data().ptr_offset_unchecked(ctx, &lhs_size, None) };
                call_memcpy_generic(ctx, list_ptr, rhs.data().base_ptr(ctx), rhs_len);

                Ok(new_list.as_abi_value(ctx).into())
            }

            Operator::Mult => {
                let (elem_ty, list_val, int_val) =
                    if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()) {
                        let elem_ty = if let TypeEnum::TObj { params, .. } =
                            &*ctx.unifier.get_ty_immutable(ty1)
                        {
                            *params.iter().next().unwrap().1
                        } else {
                            codegen_unreachable!(ctx)
                        };

                        (elem_ty, left_val, right_val)
                    } else if ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()) {
                        let elem_ty = if let TypeEnum::TObj { params, .. } =
                            &*ctx.unifier.get_ty_immutable(ty2)
                        {
                            *params.iter().next().unwrap().1
                        } else {
                            codegen_unreachable!(ctx)
                        };

                        (elem_ty, right_val, left_val)
                    } else {
                        codegen_unreachable!(ctx)
                    };
                let list_val =
                    ListValue::from_pointer_value(list_val.into_pointer_value(), llvm_usize, None);
                let int_val = ctx
                    .builder
                    .build_int_s_extend(int_val.into_int_value(), llvm_usize, "")
                    .unwrap();
                // [...] * (i where i < 0) => []
                let int_val = call_int_smax(ctx, int_val, llvm_usize.const_zero(), None);

                let elem_llvm_ty = ctx.get_llvm_type(elem_ty);
                let sizeof_elem = ctx
                    .builder
                    .build_int_truncate_or_bit_cast(elem_llvm_ty.size_of().unwrap(), llvm_usize, "")
                    .unwrap();

                let size = list_val.load_size(ctx, None);
                let new_list = ListType::new(ctx, &elem_llvm_ty).construct(
                    ctx,
                    ctx.builder.build_int_mul(size, int_val, "").unwrap(),
                    None,
                );

                gen_for_callback_incrementing(
                    &mut (),
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (int_val, false),
                    |(), ctx, _, i| {
                        let size = list_val.load_size(ctx, None);
                        let offset = ctx.builder.build_int_mul(i, size, "").unwrap();
                        let ptr =
                            unsafe { new_list.data().ptr_offset_unchecked(ctx, &offset, None) };

                        let list_size = list_val.load_size(ctx, None);

                        let memcpy_sz =
                            ctx.builder.build_int_mul(list_size, sizeof_elem, "").unwrap();

                        call_memcpy_generic(ctx, ptr, list_val.data().base_ptr(ctx), memcpy_sz);

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                    |(), _| Ok(()),
                )?;

                Ok(new_list.as_abi_value(ctx).into())
            }

            _ => todo!("Operator not supported"),
        }
    } else {
        return Ok(None);
    };

    result.map(Some)
}

/// Generates LLVM IR for a binary operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_binop_expr_with_values<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: (&Option<Type>, BasicValueEnum<'ctx>),
    op: Binop,
    right: (&Option<Type>, BasicValueEnum<'ctx>),
    loc: Location,
) -> Result<BasicValueEnum<'ctx>, String> {
    if let Some(result) = gen_prim_binop_expr(ctx, left, op, right)? {
        return Ok(result);
    }

    let (left_ty, left_val) = left;
    let (right_ty, right_val) = right;

    let ty1 = ctx.unifier.get_representative(left_ty.unwrap());
    let ty2 = ctx.unifier.get_representative(right_ty.unwrap());

    if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
        || ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
    {
        let left = ScalarOrNDArray::from_value(ctx, (ty1, left_val));
        let right = ScalarOrNDArray::from_value(ctx, (ty2, right_val));

        let ty1_dtype = arraylike_flatten_element_type(&mut ctx.unifier, ty1);
        let ty2_dtype = arraylike_flatten_element_type(&mut ctx.unifier, ty2);

        // Inhomogeneous binary operations are not supported.
        assert!(ctx.unifier.unioned(ty1_dtype, ty2_dtype));

        let common_dtype = ty1_dtype;
        let llvm_common_dtype = left.get_dtype();

        let out = match op.variant {
            BinopVariant::Normal => NDArrayOut::NewNDArray { dtype: llvm_common_dtype },
            BinopVariant::AugAssign => {
                // Augmented assignment - `left` has to be an ndarray. If it were a scalar then NAC3
                // simply doesn't support it.
                if let ScalarOrNDArray::NDArray(out_ndarray) = left {
                    NDArrayOut::WriteToNDArray { ndarray: out_ndarray }
                } else {
                    panic!("left must be an ndarray")
                }
            }
        };

        let left = left.to_ndarray(ctx);
        let right = right.to_ndarray(ctx);

        if op.base == Operator::MatMult {
            let result =
                left.matmul(ctx, ty1, (ty2, right), (common_dtype, out)).split_unsized(ctx);
            Ok(result.to_basic_value_enum())
        } else {
            // For other operations, they are all elementwise operations.

            // There are only three cases:
            // - LHS is a scalar, RHS is an ndarray.
            // - LHS is an ndarray, RHS is a scalar.
            // - LHS is an ndarray, RHS is an ndarray.
            //
            // For all cases, the scalar operand is promoted to an ndarray,
            // the two are then broadcasted, and starmapped through.

            let result = NDArrayType::new_broadcast(
                ctx,
                llvm_common_dtype,
                &[left.get_type(), right.get_type()],
            )
            .broadcast_starmap(ctx, &[left, right], out, |ctx, scalars| {
                let left_value = scalars[0];
                let right_value = scalars[1];

                let result = gen_binop_expr_with_values(
                    generator,
                    ctx,
                    (&Some(ty1_dtype), left_value),
                    op,
                    (&Some(ty2_dtype), right_value),
                    ctx.current_loc,
                )?;

                Ok(result)
            })
            .unwrap();
            Ok(result.as_abi_value(ctx).into())
        }
    } else {
        let left_ty_enum = ctx.unifier.get_ty_immutable(left_ty.unwrap());
        let TypeEnum::TObj { fields, obj_id, .. } = left_ty_enum.as_ref() else {
            codegen_unreachable!(ctx, "must be tobj")
        };
        let (op_name, id) = {
            let normal_method_name = Binop::normal(op.base).op_info().method_name;
            let assign_method_name = Binop::aug_assign(op.base).op_info().method_name;

            // if is aug_assign, try aug_assign operator first
            if op.variant == BinopVariant::AugAssign
                && fields.contains_key(&assign_method_name.into())
            {
                (assign_method_name.into(), *obj_id)
            } else {
                (normal_method_name.into(), *obj_id)
            }
        };

        let signature = if let Some(call) = ctx.calls.get(&loc.into()) {
            ctx.unifier.get_call_signature(*call).unwrap()
        } else {
            let left_enum_ty = ctx.unifier.get_ty_immutable(left_ty.unwrap());
            let TypeEnum::TObj { fields, .. } = left_enum_ty.as_ref() else {
                codegen_unreachable!(ctx, "must be tobj")
            };

            let fn_ty = fields.get(&op_name).unwrap().0;
            let fn_ty_enum = ctx.unifier.get_ty_immutable(fn_ty);
            let TypeEnum::TFunc(sig) = fn_ty_enum.as_ref() else { codegen_unreachable!(ctx) };

            sig.clone()
        };
        let fun_id = {
            let defs = ctx.top_level.definitions.read();
            let TopLevelDef::Class { methods, .. } = &*defs[id.0].read() else {
                codegen_unreachable!(ctx)
            };

            methods.iter().find(|method| method.0 == op_name).unwrap().2
        };
        generator
            .gen_call(
                ctx,
                Some((left_ty.unwrap(), left_val.into())),
                (&signature, fun_id),
                vec![(None, right_val.into())],
            )
            .map(Option::unwrap)
    }
}

/// Generates LLVM IR for a binary operator expression.
///
/// * `left` - The left-hand side of the binary operator.
/// * `op` - The operator applied on the operands.
/// * `right` - The right-hand side of the binary operator.
/// * `loc` - The location of the full expression.
pub fn gen_binop_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: &Expr<Option<Type>>,
    op: Binop,
    right: &Expr<Option<Type>>,
    loc: Location,
    result_ty: Type,
) -> Result<RtValue<'ctx>, String> {
    let left_val = generator.gen_expr(ctx, left)?.to_basic_value_enum(ctx)?;
    let right_val = generator.gen_expr(ctx, right)?.to_basic_value_enum(ctx)?;

    let result = gen_binop_expr_with_values(
        generator,
        ctx,
        (&left.custom, left_val),
        op,
        (&right.custom, right_val),
        loc,
    )?;

    Ok(RtValue::dynamic(result_ty, result))
}

/// Generates LLVM IR for a unary operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_unaryop_expr_with_values<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    op: ast::Unaryop,
    operand: (&Option<Type>, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    let (ty, val) = operand;
    let ty = ctx.unifier.get_representative(ty.unwrap());

    Ok(if ty == ctx.primitives.bool {
        let val = val.into_int_value();
        if op == ast::Unaryop::Not {
            let not = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, val, val.get_type().const_zero(), "not")
                .unwrap();

            bool_to_int_type(&ctx.builder, not, val.get_type()).into()
        } else {
            let llvm_i32 = ctx.i32;

            gen_unaryop_expr_with_values(
                ctx,
                op,
                (
                    &Some(ctx.primitives.int32),
                    ctx.builder.build_int_z_extend(val, llvm_i32, "").map(Into::into).unwrap(),
                ),
            )?
        }
    } else if [
        ctx.primitives.int32,
        ctx.primitives.int64,
        ctx.primitives.uint32,
        ctx.primitives.uint64,
    ]
    .contains(&ty)
    {
        let val = val.into_int_value();
        match op {
            ast::Unaryop::USub => ctx.builder.build_int_neg(val, "neg").map(Into::into).unwrap(),
            ast::Unaryop::Invert => ctx.builder.build_not(val, "not").map(Into::into).unwrap(),
            ast::Unaryop::Not => ctx
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    val,
                    val.get_type().const_zero(),
                    "not",
                )
                .map(Into::into)
                .unwrap(),
            ast::Unaryop::UAdd => val.into(),
        }
    } else if ty == ctx.primitives.float {
        let val = val.into_float_value();
        match op {
            ast::Unaryop::USub => ctx.builder.build_float_neg(val, "neg").map(Into::into).unwrap(),
            ast::Unaryop::Not => ctx
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    val,
                    val.get_type().const_zero(),
                    "not",
                )
                .map(Into::into)
                .unwrap(),
            _ => val.into(),
        }
    } else if ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) {
        let (ndarray_dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);

        let ndarray = NDArrayType::from_unifier_type(ctx, ty)
            .map_pointer_value(val.into_pointer_value(), None);

        // ndarray uses `~` rather than `not` to perform elementwise inversion, convert it before
        // passing it to the elementwise codegen function
        let op = if ndarray_dtype.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::Bool.id()) {
            if op == ast::Unaryop::Invert {
                ast::Unaryop::Not
            } else {
                let ndims = extract_ndims(&ctx.unifier, ty);

                codegen_unreachable!(
                    ctx,
                    "ufunc {} not supported for ndarray[bool, {}]",
                    op.op_info().method_name,
                    ndims,
                )
            }
        } else {
            op
        };

        let mapped_ndarray = ndarray.map(
            ctx,
            NDArrayOut::NewNDArray { dtype: ndarray.get_type().element_type() },
            |ctx, scalar| gen_unaryop_expr_with_values(ctx, op, (&Some(ndarray_dtype), scalar)),
        )?;

        mapped_ndarray.as_abi_value(ctx).into()
    } else {
        unimplemented!()
    })
}

/// Generates LLVM IR for a unary operator expression.
///
/// * `op` - The operator applied on the operand.
/// * `operand` - The unary operand.
pub fn gen_unaryop_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    op: ast::Unaryop,
    operand: &Expr<Option<Type>>,
    result_ty: Type,
) -> Result<RtValue<'ctx>, String> {
    let val = generator.gen_expr(ctx, operand)?.to_basic_value_enum(ctx)?;

    let result = gen_unaryop_expr_with_values(ctx, op, (&operand.custom, val))?;
    Ok(RtValue::dynamic(result_ty, result))
}

/// Generates LLVM IR for a comparison operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_cmpop_expr_with_values<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: (Option<Type>, BasicValueEnum<'ctx>),
    ops: &[ast::Cmpop],
    comparators: &[(Option<Type>, BasicValueEnum<'ctx>)],
) -> Result<BasicValueEnum<'ctx>, String> {
    debug_assert_eq!(comparators.len(), ops.len());

    if comparators.len() == 1 {
        let left_ty = ctx.unifier.get_representative(left.0.unwrap());
        let right_ty = ctx.unifier.get_representative(comparators[0].0.unwrap());

        if left_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            || right_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
        {
            let (Some(left_ty), left) = left else { codegen_unreachable!(ctx) };
            let (Some(right_ty), right) = comparators[0] else { codegen_unreachable!(ctx) };
            let op = ops[0];

            let left_ty_dtype = arraylike_flatten_element_type(&mut ctx.unifier, left_ty);
            let right_ty_dtype = arraylike_flatten_element_type(&mut ctx.unifier, right_ty);

            let left = ScalarOrNDArray::from_value(ctx, (left_ty, left)).to_ndarray(ctx);
            let right = ScalarOrNDArray::from_value(ctx, (right_ty, right)).to_ndarray(ctx);

            let result_ndarray = NDArrayType::new_broadcast(
                ctx,
                ctx.i8.into(),
                &[left.get_type(), right.get_type()],
            )
            .broadcast_starmap(
                ctx,
                &[left, right],
                NDArrayOut::NewNDArray { dtype: ctx.i8.into() },
                |ctx, scalars| {
                    let left_scalar = scalars[0];
                    let right_scalar = scalars[1];

                    let val = gen_cmpop_expr_with_values(
                        generator,
                        ctx,
                        (Some(left_ty_dtype), left_scalar),
                        &[op],
                        &[(Some(right_ty_dtype), right_scalar)],
                    )?;

                    Ok(bool_to_i8(ctx, val.into_int_value()).into())
                },
            )?;

            return Ok(result_ndarray.as_abi_value(ctx).into());
        }
    }

    let cmp_val = izip!(once(&left).chain(comparators.iter()), comparators.iter(), ops.iter(),)
        .fold(Ok(None), |prev: Result<Option<_>, String>, (lhs, rhs, op)| {
            let (left_ty, lhs) = lhs;
            let (right_ty, rhs) = rhs;

            let left_ty = ctx.unifier.get_representative(left_ty.unwrap());
            let right_ty = ctx.unifier.get_representative(right_ty.unwrap());

            let current = if [
                ctx.primitives.int32,
                ctx.primitives.int64,
                ctx.primitives.uint32,
                ctx.primitives.uint64,
                ctx.primitives.bool,
            ]
            .contains(&left_ty)
            {
                assert!(ctx.unifier.unioned(left_ty, right_ty));

                let use_unsigned_ops =
                    [ctx.primitives.uint32, ctx.primitives.uint64].contains(&left_ty);

                let lhs = lhs.into_int_value();
                let rhs = rhs.into_int_value();

                let op = match op {
                    ast::Cmpop::Eq | ast::Cmpop::Is => IntPredicate::EQ,
                    ast::Cmpop::NotEq => IntPredicate::NE,
                    _ if left_ty == ctx.primitives.bool => codegen_unreachable!(ctx),
                    ast::Cmpop::Lt => {
                        if use_unsigned_ops {
                            IntPredicate::ULT
                        } else {
                            IntPredicate::SLT
                        }
                    }
                    ast::Cmpop::LtE => {
                        if use_unsigned_ops {
                            IntPredicate::ULE
                        } else {
                            IntPredicate::SLE
                        }
                    }
                    ast::Cmpop::Gt => {
                        if use_unsigned_ops {
                            IntPredicate::UGT
                        } else {
                            IntPredicate::SGT
                        }
                    }
                    ast::Cmpop::GtE => {
                        if use_unsigned_ops {
                            IntPredicate::UGE
                        } else {
                            IntPredicate::SGE
                        }
                    }
                    _ => codegen_unreachable!(ctx),
                };

                ctx.builder.build_int_compare(op, lhs, rhs, "cmp").unwrap()
            } else if left_ty == ctx.primitives.float {
                assert!(ctx.unifier.unioned(left_ty, right_ty));

                let lhs = lhs.into_float_value();
                let rhs = rhs.into_float_value();

                let op = match op {
                    ast::Cmpop::Eq | ast::Cmpop::Is => inkwell::FloatPredicate::OEQ,
                    ast::Cmpop::NotEq => inkwell::FloatPredicate::ONE,
                    ast::Cmpop::Lt => inkwell::FloatPredicate::OLT,
                    ast::Cmpop::LtE => inkwell::FloatPredicate::OLE,
                    ast::Cmpop::Gt => inkwell::FloatPredicate::OGT,
                    ast::Cmpop::GtE => inkwell::FloatPredicate::OGE,
                    _ => codegen_unreachable!(ctx),
                };
                ctx.builder.build_float_compare(op, lhs, rhs, "cmp").unwrap()
            } else if left_ty == ctx.primitives.str {
                assert!(ctx.unifier.unioned(left_ty, right_ty));

                let llvm_str = StringType::new(ctx);

                let lhs = llvm_str.map_struct_value(lhs.into_struct_value(), None);
                let rhs = llvm_str.map_struct_value(rhs.into_struct_value(), None);

                let result = call_string_eq(ctx, lhs, rhs);
                if *op == Cmpop::NotEq {
                    gen_unaryop_expr_with_values(
                        ctx,
                        Unaryop::Not,
                        (&Some(ctx.primitives.bool), result.into()),
                    )?.into_int_value()
                } else {
                    result
                }
            } else if [left_ty, right_ty]
                .iter()
                .any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()))
            {
                let llvm_usize = ctx.size_t;

                let gen_list_cmpop = |generator: &mut G,
                                      ctx: &mut CodeGenContext<'ctx, '_>|
                 -> Result<IntValue<'ctx>, String> {
                    let is_list1 =
                        left_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id());
                    let is_list2 =
                        right_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id());

                    let gen_bool_const = |ctx: &CodeGenContext<'ctx, '_>, val: bool| {
                        let llvm_i1 = ctx.i1;

                        match (op, val) {
                            (Cmpop::Eq, true) | (Cmpop::NotEq, false) => llvm_i1.const_all_ones(),
                            (Cmpop::Eq, false) | (Cmpop::NotEq, true) => llvm_i1.const_zero(),
                            (_, _) => codegen_unreachable!(ctx),
                        }
                    };

                    if !(is_list1 && is_list2) {
                        return Ok(bool_to_i8(ctx, gen_bool_const(ctx, false)));
                    }

                    let left_elem_ty = if let TypeEnum::TObj { params, .. } =
                        &*ctx.unifier.get_ty_immutable(left_ty)
                    {
                        *params.iter().next().unwrap().1
                    } else {
                        codegen_unreachable!(ctx)
                    };
                    let right_elem_ty = if let TypeEnum::TObj { params, .. } =
                        &*ctx.unifier.get_ty_immutable(right_ty)
                    {
                        *params.iter().next().unwrap().1
                    } else {
                        codegen_unreachable!(ctx)
                    };

                    if !ctx.unifier.unioned(left_elem_ty, right_elem_ty) {
                        return Ok(bool_to_i8(ctx, gen_bool_const(ctx, false)));
                    }

                    if ![Cmpop::Eq, Cmpop::NotEq].contains(op) {
                        todo!("Only __eq__ and __ne__ is implemented for lists")
                    }

                    let left_val =
                        ListValue::from_pointer_value(lhs.into_pointer_value(), llvm_usize, None);
                    let right_val =
                        ListValue::from_pointer_value(rhs.into_pointer_value(), llvm_usize, None);

                    Ok(gen_if_else_expr_callback(
                        generator,
                        ctx,
                        |_, ctx| {
                            let left_size = left_val.load_size(ctx, None);
                            let right_size = right_val.load_size(ctx, None);
                            Ok(ctx
                                .builder
                                .build_int_compare(
                                    IntPredicate::EQ,
                                    left_size,
                                    right_size,
                                    "",
                                )
                                .unwrap())
                        },
                        |generator, ctx| {
                            let acc_addr = gen_var(ctx, ctx.i1.into(), None)
                                .unwrap();
                            ctx.builder
                                .build_store(acc_addr, ctx.i1.const_all_ones())
                                .unwrap();

                            let left_size = left_val.load_size(ctx, None);
                            gen_for_callback_incrementing(
                                &mut (),
                                ctx,
                                None,
                                llvm_usize.const_zero(),
                                (left_size, false),
                                |(), ctx, hooks, i| {
                                    let left = unsafe {
                                        left_val.data().get_unchecked(ctx, &i, None)
                                    };
                                    let right = unsafe {
                                        right_val.data().get_unchecked(ctx, &i, None)
                                    };

                                    let res = gen_cmpop_expr_with_values(
                                        generator,
                                        ctx,
                                        (Some(left_elem_ty), left),
                                        &[Cmpop::Eq],
                                        &[(Some(right_elem_ty), right)],
                                    )?
                                    .into_int_value();

                                    gen_if_callback(
                                        &mut (),
                                        ctx,
                                        |(), ctx| {
                                            Ok(ctx
                                                .builder
                                                .build_int_compare(
                                                    IntPredicate::EQ,
                                                    res,
                                                    res.get_type().const_zero(),
                                                    "",
                                                )
                                                .unwrap())
                                        },
                                        |(), ctx| {
                                            ctx.builder
                                                .build_store(
                                                    acc_addr,
                                                    ctx.i1.const_zero(),
                                                )
                                                .unwrap();
                                            hooks.build_break_branch(&ctx.builder);

                                            Ok(())
                                        },
                                        |(), _| Ok(()),
                                    )
                                    .unwrap();

                                    Ok(())
                                },
                                llvm_usize.const_int(1, false),
                                |(), _| Ok(()),
                            )?;

                            let acc = ctx
                                .builder
                                .build_load(acc_addr, "")
                                .map(BasicValueEnum::into_int_value)
                                .unwrap();
                            let acc = if *op == Cmpop::NotEq {
                                gen_unaryop_expr_with_values(
                                    ctx,
                                    Unaryop::Not,
                                    (&Some(ctx.primitives.bool), acc.into()),
                                )?
                                .into_int_value()
                            } else {
                                acc
                            };

                            Ok(Some(bool_to_i8(ctx, acc)))
                        },
                        |_generator, ctx| {
                            Ok(Some(bool_to_i8(ctx, gen_bool_const(ctx, false))))
                        },
                    )?
                    .map(BasicValueEnum::into_int_value)
                    .unwrap())
                };

                gen_list_cmpop(generator, ctx)?
            } else if [left_ty, right_ty].iter().any(|ty| matches!(&*ctx.unifier.get_ty_immutable(*ty), TypeEnum::TTuple { .. })) {
                let TypeEnum::TTuple { ty: left_tys, .. } = &*ctx.unifier.get_ty_immutable(left_ty) else {
                    return Err(format!("'{}' not supported between instances of '{}' and '{}'", op.op_info().symbol, ctx.unifier.stringify(left_ty), ctx.unifier.stringify(right_ty)))
                };
                let TypeEnum::TTuple { ty: right_tys, .. } = &*ctx.unifier.get_ty_immutable(right_ty) else {
                    return Err(format!("'{}' not supported between instances of '{}' and '{}'", op.op_info().symbol, ctx.unifier.stringify(left_ty), ctx.unifier.stringify(right_ty)))
                };

                if ![Cmpop::Eq, Cmpop::NotEq].contains(op) {
                    todo!("Only __eq__ and __ne__ is implemented for tuples")
                }

                let llvm_i1 = ctx.i1;
                let llvm_i32 = ctx.i32;

                // Assume `true` by default
                let cmp_addr = gen_var(ctx, llvm_i1.into(), None).unwrap();
                ctx.builder.build_store(cmp_addr, llvm_i1.const_all_ones()).unwrap();

                let current_bb = ctx.builder.get_insert_block().unwrap();
                let post_foreach_cmp = ctx.ctx.insert_basic_block_after(current_bb, "foreach.cmp.end");

                ctx.builder.position_at_end(post_foreach_cmp);
                let cmp_phi = ctx.builder.build_phi(llvm_i1, "").unwrap();
                ctx.builder.position_at_end(current_bb);

                // Generate comparison between each element
                let min_len = min(left_tys.len(), right_tys.len());
                for i in 0..min_len {
                    let current_bb = ctx.builder.get_insert_block().unwrap();
                    let bb = ctx.ctx.insert_basic_block_after(current_bb, &format!("foreach.cmp.tuple.{i}e"));
                    ctx.builder.build_unconditional_branch(bb).unwrap();

                    ctx.builder.position_at_end(bb);
                    let left_ty = left_tys[i];
                    let left_elem = {
                        let plhs = gen_var(ctx, lhs.get_type(), None).unwrap();
                        ctx.builder.build_store(plhs, *lhs).unwrap();

                        ctx.build_in_bounds_gep_and_load(
                            plhs,
                            &[llvm_i32.const_zero(), llvm_i32.const_int(i as u64, false)],
                            None,
                        )
                    };
                    let right_ty = right_tys[i];
                    let right_elem = {
                        let prhs = gen_var(ctx, rhs.get_type(), None).unwrap();
                        ctx.builder.build_store(prhs, *rhs).unwrap();

                        ctx.build_in_bounds_gep_and_load(
                            prhs,
                            &[llvm_i32.const_zero(), llvm_i32.const_int(i as u64, false)],
                            None,
                        )
                    };

                    gen_if_callback(
                        generator,
                        ctx,
                        |generator, ctx| {
                            // Defer the `not` operation until the end - a != b <=> !(a == b)
                            let op = if *op == Cmpop::NotEq { Cmpop::Eq } else { *op };

                            let cmp = gen_cmpop_expr_with_values(
                                generator,
                                ctx,
                                (Some(left_ty), left_elem),
                                &[op],
                                &[(Some(right_ty), right_elem)],
                            )
                                .map(BasicValueEnum::into_int_value)?;

                            Ok(ctx.builder.build_not(
                                bool_to_i1(ctx, cmp),
                                "",
                            ).unwrap())
                        },
                        |_, ctx| {
                            let bb = ctx.builder.get_insert_block().unwrap();
                            cmp_phi.add_incoming(&[(&llvm_i1.const_zero(), bb)]);
                            ctx.builder.build_unconditional_branch(post_foreach_cmp).unwrap();

                            Ok(())
                        },
                        |_, _| Ok(()),
                    )?;
                }

                // Length of tuples is checked last as operators do not short-circuit by tuple
                // length in Python:
                //
                // >>> (1, 2) < ("a",)
                // TypeError: '<' not supported between instances of 'int' and 'str'
                let bb = ctx.builder.get_insert_block().unwrap();
                let is_len_eq = llvm_i1.const_int(
                    u64::from(left_tys.len() == right_tys.len()),
                    false,
                );
                cmp_phi.add_incoming(&[(&is_len_eq, bb)]);
                ctx.builder.build_unconditional_branch(post_foreach_cmp).unwrap();

                ctx.builder.position_at_end(post_foreach_cmp);
                let cmp_phi = cmp_phi.as_basic_value().into_int_value();

                // Invert the final value if __ne__
                if *op == Cmpop::NotEq {
                    gen_unaryop_expr_with_values(
                        ctx,
                        Unaryop::Not,
                        (&Some(ctx.primitives.bool), cmp_phi.into()),
                    )?.into_int_value()
                } else {
                    cmp_phi
                }
            } else if [left_ty, right_ty].iter().any(|ty| matches!(&*ctx.unifier.get_ty_immutable(*ty), TypeEnum::TVar { .. })) {
                if ctx.registry.codegen_options.debug {
                    ctx.make_assert(
                        ctx.i1.const_all_ones(),
                        "0:AssertionError",
                        "nac3core::codegen::expr::gen_cmpop_expr_with_values: Unexpected comparison between two typevar values",
                        [None, None, None],
                        ctx.current_loc,
                    );
                }

                ctx.i1.get_poison()
            } else {
                return Err(format!("'{}' not supported between instances of '{}' and '{}'",
                                   op.op_info().symbol,
                                   ctx.unifier.stringify(left_ty),
                                   ctx.unifier.stringify(right_ty)))
            };

            Ok(prev?.map(|v| ctx.builder.build_and(v, current, "cmp").unwrap()).or(Some(current)))
        })?.unwrap();

    Ok(cmp_val.into())
}

/// Generates LLVM IR for a comparison operator expression.
///
/// * `left` - The left-hand side of the comparison operator.
/// * `ops` - The (possibly chained) operators applied on the operands.
/// * `comparators` - The right-hand side of the binary operator.
pub fn gen_cmpop_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: &Expr<Option<Type>>,
    ops: &[ast::Cmpop],
    comparators: &[Expr<Option<Type>>],
    result_ty: Type,
) -> Result<RtValue<'ctx>, String> {
    let left_val = generator.gen_expr(ctx, left)?.to_basic_value_enum(ctx)?;

    let comparator_vals = comparators
        .iter()
        .map(|cmptor| {
            Ok((cmptor.custom, generator.gen_expr(ctx, cmptor)?.to_basic_value_enum(ctx)?))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let result = gen_cmpop_expr_with_values(
        generator,
        ctx,
        (left.custom, left_val),
        ops,
        comparator_vals.as_slice(),
    )?;

    Ok(RtValue::dynamic(result_ty, result))
}

fn gen_list_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    elts: &[Expr<Option<Type>>],
) -> Result<RtValue<'ctx>, String> {
    // this shall be optimized later for constant primitive lists...
    // we should use memcpy for that instead of generating thousands of stores
    let elements = elts
        .iter()
        .map(|x| generator.gen_expr(ctx, x).and_then(|v| v.to_basic_value_enum(ctx)))
        .collect::<Result<Vec<_>, _>>()?;

    let ty_inner = if elements.is_empty() {
        let ty_inner = if let TypeEnum::TObj { obj_id, params, .. } = &*ctx.unifier.get_ty(ty) {
            assert_eq!(*obj_id, PrimDef::List.id());

            *params.iter().next().unwrap().1
        } else {
            codegen_unreachable!(ctx)
        };

        if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(ty_inner) {
            None
        } else {
            Some(ctx.get_llvm_type(ty_inner))
        }
    } else {
        Some(elements[0].get_type())
    };
    let length = ctx.size_t.const_int(elements.len() as u64, false);
    let arr_str_ptr = if let Some(ty_inner) = ty_inner {
        ListType::new(ctx, &ty_inner).construct(ctx, length, Some("list"))
    } else {
        ListType::new_untyped(ctx).construct_empty(ctx, Some("list"))
    };
    let arr_ptr = arr_str_ptr.data();
    for (i, v) in elements.iter().enumerate() {
        let elem_ptr =
            arr_ptr.ptr_offset(ctx, &ctx.size_t.const_int(i as u64, false), Some("elem_ptr"));
        ctx.builder.build_store(elem_ptr, *v).unwrap();
    }
    Ok(RtValue::dynamic(ty, arr_str_ptr.as_abi_value(ctx).into()))
}

fn gen_tuple_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    elts: &[Expr<Option<Type>>],
) -> Result<RtValue<'ctx>, String> {
    let element_val = elts
        .iter()
        .map(|x| generator.gen_expr(ctx, x).and_then(|v| v.to_basic_value_enum(ctx)))
        .collect::<Result<Vec<_>, _>>()?;

    let element_ty = element_val.iter().map(BasicValueEnum::get_type).collect_vec();
    let tuple_ty = ctx.ctx.struct_type(&element_ty, false);
    let tuple_ptr = ctx.builder.build_alloca(tuple_ty, "tuple").unwrap();
    for (i, v) in element_val.into_iter().enumerate() {
        unsafe {
            let ptr = ctx
                .builder
                .build_in_bounds_gep(
                    tuple_ptr,
                    &[ctx.i32.const_int(0, false), ctx.i32.const_int(i as u64, false)],
                    "ptr",
                )
                .unwrap();
            ctx.builder.build_store(ptr, v).unwrap();
        }
    }
    let val = ctx.builder.build_load(tuple_ptr, "tup_val").unwrap();
    Ok(RtValue::dynamic(ty, val))
}

fn gen_attr_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    expr: &Expr<Option<Type>>,
    value: &Expr<Option<Type>>,
    attr: StrRef,
) -> Result<RtValue<'ctx>, String> {
    // note that we would handle class methods directly in calls

    // Change Class attribute access requests to accessing constants from Class Definition
    if let Some(c) = value.custom {
        if let TypeEnum::TFunc(_) = &*ctx.unifier.get_ty(c) {
            let result = ctx.top_level.definitions.read().iter().find_map(|def| {
                if let Some(rear_guard) = def.try_read()
                    && let TopLevelDef::Class { constructor: Some(constructor), attributes, .. } =
                        &*rear_guard
                    && *constructor == c
                {
                    return attributes.iter().find(|f| f.0 == attr).map(|f| f.2.clone());
                }
                None
            });
            match result {
                Some(val) => {
                    let mut modified_expr = expr.clone();
                    modified_expr.node = ExprKind::Constant { value: val, kind: None };

                    return generator.gen_expr(ctx, &modified_expr);
                }
                None => {
                    codegen_unreachable!(ctx, "Function Type should not have attributes")
                }
            }
        } else if let TypeEnum::TObj { obj_id, fields, params } = &*ctx.unifier.get_ty(c)
            && fields.is_empty()
            && params.is_empty()
        {
            let defs = ctx.top_level.definitions.read();
            let TopLevelDef::Class { attributes, .. } = &*defs[obj_id.0].read() else {
                codegen_unreachable!(ctx);
            };
            let Some(val) = attributes.iter().find(|f| f.0 == attr).map(|f| f.2.clone()) else {
                codegen_unreachable!(ctx);
            };
            let mut modified_expr = expr.clone();
            modified_expr.node = ExprKind::Constant { value: val, kind: None };

            return generator.gen_expr(ctx, &modified_expr);
        }
    }

    let res = match generator.gen_expr(ctx, value)?.val {
        Some(ValueEnum::Static(v)) => match v.get_field(attr, ctx) {
            Some(ValueEnum::Static(v)) => RtValue::r#static(ty, v),
            Some(ValueEnum::Dynamic(v)) => RtValue::dynamic(ty, v),
            None => {
                let v = v.to_basic_value_enum(ctx, value.custom.unwrap())?;
                let (index, _) = ctx.get_attr_index(value.custom.unwrap(), attr);
                let val = ctx.build_gep_and_load(
                    v.into_pointer_value(),
                    &[ctx.i32.const_int(0, false), ctx.i32.const_int(index as u64, false)],
                    None,
                );
                RtValue::dynamic(ty, val)
            }
        },
        Some(ValueEnum::Dynamic(v)) => {
            let (index, attr_value) = ctx.get_attr_index(value.custom.unwrap(), attr);
            if let Some(val) = attr_value {
                // Change to Constant Construct
                let mut modified_expr = expr.clone();
                modified_expr.node = ExprKind::Constant { value: val, kind: None };

                return generator.gen_expr(ctx, &modified_expr);
            }
            let result = ctx.build_gep_and_load(
                v.into_pointer_value(),
                &[ctx.i32.const_int(0, false), ctx.i32.const_int(index as u64, false)],
                None,
            );
            RtValue::dynamic(ty, result)
        }
        None => RtValue::none(ty),
    };
    Ok(res)
}

fn gen_boolop_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    op: Boolop,
    values: &[Expr<Option<Type>>],
) -> Result<RtValue<'ctx>, String> {
    // requires conditional branches for short-circuiting...
    let left = generator.gen_expr(ctx, &values[0])?.to_basic_value_enum(ctx)?.into_int_value();
    let left = bool_to_i1(ctx, left);
    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let a_begin_bb = ctx.ctx.append_basic_block(current, "a_begin");
    let a_end_bb = ctx.ctx.append_basic_block(current, "a_end");
    let b_begin_bb = ctx.ctx.append_basic_block(current, "b_begin");
    let b_end_bb = ctx.ctx.append_basic_block(current, "b_end");
    let cont_bb = ctx.ctx.append_basic_block(current, "cont");
    ctx.builder.build_conditional_branch(left, a_begin_bb, b_begin_bb).unwrap();

    ctx.builder.position_at_end(a_end_bb);
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();
    ctx.builder.position_at_end(b_end_bb);
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();
    let (a, b) = match op {
        Boolop::Or => {
            ctx.builder.position_at_end(a_begin_bb);
            let a = ctx.i8.const_int(1, false);
            ctx.builder.build_unconditional_branch(a_end_bb).unwrap();

            ctx.builder.position_at_end(b_begin_bb);
            let b = generator.gen_expr(ctx, &values[1])?.to_basic_value_enum(ctx)?.into_int_value();
            let b = bool_to_i8(ctx, b);
            ctx.builder.build_unconditional_branch(b_end_bb).unwrap();

            (a, b)
        }
        Boolop::And => {
            ctx.builder.position_at_end(a_begin_bb);
            let a = generator.gen_expr(ctx, &values[1])?.to_basic_value_enum(ctx)?.into_int_value();
            let a = bool_to_i8(ctx, a);
            ctx.builder.build_unconditional_branch(a_end_bb).unwrap();

            ctx.builder.position_at_end(b_begin_bb);
            let b = ctx.i8.const_zero();
            ctx.builder.build_unconditional_branch(b_end_bb).unwrap();

            (a, b)
        }
    };

    ctx.builder.position_at_end(cont_bb);
    let phi = ctx.builder.build_phi(ctx.i8, "").unwrap();
    phi.add_incoming(&[(&a, a_end_bb), (&b, b_end_bb)]);
    Ok(RtValue::dynamic(ty, phi.as_basic_value()))
}

fn gen_ifexp_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    test: &Expr<Option<Type>>,
    body: &Expr<Option<Type>>,
    orelse: &Expr<Option<Type>>,
) -> Result<RtValue<'ctx>, String> {
    let test = generator.gen_expr(ctx, test)?.to_basic_value_enum(ctx)?.into_int_value();
    let test = bool_to_i1(ctx, test);
    let body_ty = body.custom.unwrap();
    let is_none = ctx.unifier.get_representative(body_ty) == ctx.primitives.none;
    let result = if is_none {
        None
    } else {
        let llvm_ty = ctx.get_llvm_type(body_ty);
        Some(ctx.builder.build_alloca(llvm_ty, "if_exp_result").unwrap())
    };
    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
    let then_bb = ctx.ctx.append_basic_block(current, "then");
    let else_bb = ctx.ctx.append_basic_block(current, "else");
    let cont_bb = ctx.ctx.append_basic_block(current, "cont");
    ctx.builder.build_conditional_branch(test, then_bb, else_bb).unwrap();

    ctx.builder.position_at_end(then_bb);
    let a = generator.gen_expr(ctx, body)?;
    match result {
        None => None,
        Some(v) => {
            let a = a.to_basic_value_enum(ctx)?;
            Some(ctx.builder.build_store(v, a))
        }
    };
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();

    ctx.builder.position_at_end(else_bb);
    let b = generator.gen_expr(ctx, orelse)?;
    match result {
        None => None,
        Some(v) => {
            let b = b.to_basic_value_enum(ctx)?;
            Some(ctx.builder.build_store(v, b))
        }
    };
    ctx.builder.build_unconditional_branch(cont_bb).unwrap();

    ctx.builder.position_at_end(cont_bb);
    Ok(if let Some(v) = result {
        let val = ctx.builder.build_load(v, "if_exp_val_load").unwrap();
        RtValue::dynamic(ty, val)
    } else {
        RtValue::none(ty)
    })
}

fn gen_call_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    expr: &Expr<Option<Type>>,
    func: &Expr<Option<Type>>,
    args: &[Expr<Option<Type>>],
    keywords: &[Keyword<Option<Type>>],
) -> Result<RtValue<'ctx>, String> {
    let mut params: Vec<(Option<StrRef>, ValueEnum)> = vec![];
    for arg in args {
        let val = generator.gen_expr(ctx, arg)?.val.unwrap();
        params.push((None, val));
    }
    for kw in keywords {
        let val = generator.gen_expr(ctx, &kw.node.value)?.val.unwrap();
        params.push((Some(*kw.node.arg.as_ref().unwrap()), val));
    }
    let call = ctx.calls.get(&expr.location.into());
    let signature = if let Some(call) = call {
        ctx.unifier.get_call_signature(*call).unwrap()
    } else {
        let func_ty = func.custom.unwrap();
        let TypeEnum::TFunc(sign) = &*ctx.unifier.get_ty(func_ty) else {
            codegen_unreachable!(ctx)
        };

        sign.clone()
    };
    if let Some(builtin) = ctx.top_level.builtin_registry.match_builtin(&erase_expr_type(func)) {
        let callable = builtin.as_callable();
        let ret_val = generator.gen_call(ctx, None, (&signature, callable.id()), params)?;
        return Ok(if let Some(val) = ret_val {
            RtValue::dynamic(ty, val)
        } else {
            RtValue::none(ty)
        });
    }
    match &func.node {
        ExprKind::Name { id, .. } => {
            // TODO: handle primitive casts and function pointers
            let fun = ctx
                .resolver
                .get_identifier_def(*id)
                .map_err(|e| format!("{} (at {})", e.iter().next().unwrap(), func.location))?;
            let ret_val = generator.gen_call(ctx, None, (&signature, fun), params)?;
            Ok(ret_val.map_or_else(|| RtValue::none(ty), |val| RtValue::dynamic(ty, val)))
        }
        ExprKind::Attribute { value, attr, .. } => {
            // Handle Class Method calls
            // The attribute will be `DefinitionId` of the method if the call is to one of the parent methods
            let func_id = attr.to_string().parse::<usize>();

            // For a static method the constructor hasn't always been called, so we get the
            // class UnificationKey from the return type of the constructor signature.
            let (key, mut is_static) =
                if let TypeEnum::TFunc(sign) = &*ctx.unifier.get_ty(value.custom.unwrap()) {
                    // The class is not instantiated yet, so we can assume the method is static
                    (sign.ret, true)
                } else {
                    // The class is instantiated meaning the method may or may not be static;
                    // for now assume it is not, but resolve once the method data is available
                    (value.custom.unwrap(), false)
                };

            let TypeEnum::TObj { obj_id: id, .. } = &*ctx.unifier.get_ty(key) else {
                codegen_unreachable!(ctx)
            };

            // Use the `DefinitionID` from attribute if it is available
            let fun_id = if let Ok(func_id) = func_id {
                DefinitionId(func_id)
            } else {
                match &*ctx.top_level.definitions.read()[id.0].read() {
                    TopLevelDef::Class { methods, .. } => {
                        let fun_id = methods.iter().find(|method| method.0 == *attr).unwrap().2;

                        // A method call on a class instance could still be to a static method
                        // so we check if the function has been annotated as static
                        let is_static_method = if let TopLevelDef::Function { attributes, .. } =
                            &*ctx.top_level.definitions.read()[fun_id.0].read()
                        {
                            attributes.contains(&FunAttribute::StaticMethod)
                        } else {
                            false
                        };
                        is_static = is_static || is_static_method;
                        fun_id
                    }
                    TopLevelDef::Module { functions, .. } => {
                        functions.iter().find(|method| method.0 == *attr).unwrap().1
                    }
                    TopLevelDef::Function { .. } => codegen_unreachable!(ctx),
                }
            };

            // If the function is static, we can call it directly
            if is_static {
                ctx.current_loc = expr.location;
                let ret_val = generator.gen_call(ctx, None, (&signature, fun_id), params)?;
                return Ok(
                    ret_val.map_or_else(|| RtValue::none(ty), |val| RtValue::dynamic(ty, val))
                );
            }

            let val = generator.gen_expr(ctx, value)?.val.unwrap();

            // directly generate code for option.unwrap
            // since it needs to return static value to optimize for kernel invariant
            if attr == &"unwrap".into()
                && *id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap()
            {
                let res = match val {
                    ValueEnum::Static(v) => {
                        let field_opt = v.get_field("_nac3_option".into(), ctx);
                        if let Some(field_val) = field_opt {
                            let v_val = field_val.to_basic_value_enum(ctx, ty)?;
                            RtValue::dynamic(ty, v_val)
                        } else {
                            // if is none, raise exception directly
                            let err_msg = ctx.gen_string("");
                            let current_fun =
                                ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
                            let unreachable_block =
                                ctx.ctx.append_basic_block(current_fun, "unwrap_none_unreachable");
                            let exn_block =
                                ctx.ctx.append_basic_block(current_fun, "unwrap_none_exception");
                            ctx.builder.build_unconditional_branch(exn_block).unwrap();
                            ctx.builder.position_at_end(exn_block);
                            ctx.raise_exn(
                                "0:UnwrapNoneError",
                                err_msg.into(),
                                [None, None, None],
                                ctx.current_loc,
                            );
                            ctx.builder.position_at_end(unreachable_block);
                            let ptr = ctx.get_llvm_type(key).into_pointer_type().const_null();
                            let loaded_val = ctx
                                .builder
                                .build_load(ptr, "unwrap_none_unreachable_load")
                                .unwrap();
                            RtValue::dynamic(ty, loaded_val)
                        }
                    }
                    ValueEnum::Dynamic(BasicValueEnum::PointerValue(ptr)) => {
                        let option = OptionType::from_pointer_type(ptr.get_type(), ctx.size_t)
                            .map_pointer_value(ptr, None);
                        let not_null = option.is_some(ctx);
                        ctx.make_assert(
                            not_null,
                            "0:UnwrapNoneError",
                            "",
                            [None, None, None],
                            expr.location,
                        );
                        let loaded = unsafe { option.load(ctx) };
                        RtValue::dynamic(ty, loaded)
                    }
                    ValueEnum::Dynamic(_) => {
                        codegen_unreachable!(ctx, "option must be static or ptr")
                    }
                };
                return Ok(res);
            }

            // Reset current_loc back to the location of the call
            ctx.current_loc = expr.location;

            let ret_val =
                generator.gen_call(ctx, Some((key, val)), (&signature, fun_id), params)?;
            Ok(ret_val.map_or_else(|| RtValue::none(ty), |val| RtValue::dynamic(ty, val)))
        }
        _ => unimplemented!(),
    }
}

fn gen_subscript_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    expr: &Expr<Option<Type>>,
    value: &Expr<Option<Type>>,
    slice: &Expr<Option<Type>>,
) -> Result<RtValue<'ctx>, String> {
    let res = match &*ctx.unifier.get_ty(value.custom.unwrap()) {
        TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
            let ty_elem = params.iter().next().unwrap().1;

            let v_rt = generator.gen_expr(ctx, value)?;
            let v = v_rt.to_basic_value_enum(ctx)?.into_pointer_value();
            let v = ListValue::from_pointer_value(v, ctx.size_t, Some("arr"));
            let ty_elem_llvm = ctx.get_llvm_type(*ty_elem);
            if let ExprKind::Slice { lower, upper, step } = &slice.node {
                let one = ctx.i32.const_int(1, false);
                let size = v.load_size(ctx, None);
                let Some((start, end, step)) =
                    handle_slice_indices(lower, upper, step, ctx, generator, size)?
                else {
                    return Ok(RtValue::none(ty));
                };
                let cond = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::SLT,
                        step,
                        ctx.i32.const_int(0, false),
                        "is_neg",
                    )
                    .unwrap();
                let then = ctx.builder.build_int_sub(end, one, "e_min_one").unwrap();
                let else_ = ctx.builder.build_int_add(end, one, "e_add_one").unwrap();
                let end_slice = ctx
                    .builder
                    .build_select(cond, then, else_, "final_e")
                    .map(BasicValueEnum::into_int_value)
                    .unwrap();
                let length = calculate_len_for_slice_range(ctx, start, end_slice, step);
                let res_array_ret =
                    ListType::new(ctx, &ty_elem_llvm).construct(ctx, length, Some("ret"));
                let size = res_array_ret.load_size(ctx, None);
                let Some(res_ind) =
                    handle_slice_indices(&None, &None, &None, ctx, generator, size)?
                else {
                    return Ok(RtValue::none(ty));
                };
                list_slice_assignment(
                    ctx,
                    ty_elem_llvm,
                    res_array_ret,
                    res_ind,
                    v,
                    (start, end, step),
                );
                RtValue::dynamic(ty, res_array_ret.as_abi_value(ctx).into())
            } else {
                let len = v.load_size(ctx, Some("len"));
                let raw_index =
                    generator.gen_expr(ctx, slice)?.to_basic_value_enum(ctx)?.into_int_value();
                let raw_index =
                    ctx.builder.build_int_s_extend(raw_index, ctx.size_t, "sext").unwrap();
                // handle negative index
                let is_negative = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::SLT,
                        raw_index,
                        ctx.size_t.const_zero(),
                        "is_neg",
                    )
                    .unwrap();
                let adjusted = ctx.builder.build_int_add(raw_index, len, "adjusted").unwrap();
                let index = ctx
                    .builder
                    .build_select(is_negative, adjusted, raw_index, "index")
                    .map(BasicValueEnum::into_int_value)
                    .unwrap();
                // unsigned less than is enough, because negative index after adjustment is
                // bigger than the length (for unsigned cmp)
                let bound_check = ctx
                    .builder
                    .build_int_compare(IntPredicate::ULT, index, len, "inbound")
                    .unwrap();
                ctx.make_assert(
                    bound_check,
                    "0:IndexError",
                    "index {0} out of bounds 0:{1}",
                    [Some(raw_index), Some(len), None],
                    expr.location,
                );
                let result = v.data().get(ctx, &index, None);
                RtValue::dynamic(ty, result)
            }
        }
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            let ndarray_ty = value.custom.unwrap();
            let ndarray = generator.gen_expr(ctx, value)?.to_basic_value_enum(ctx)?;
            let ndarray = NDArrayType::from_unifier_type(ctx, ndarray_ty)
                .map_pointer_value(ndarray.into_pointer_value(), None);

            let indices = RustNDIndex::from_subscript_expr(generator, ctx, slice)?;
            let result = ndarray.index(ctx, &indices).split_unsized(ctx).to_basic_value_enum();
            RtValue::dynamic(ty, result)
        }
        TypeEnum::TTuple { .. } => {
            let index: u32 = if let ExprKind::Constant { value: Constant::Int(v), .. } = &slice.node
            {
                (*v).try_into().unwrap()
            } else {
                codegen_unreachable!(ctx, "tuple subscript must be const int after type check");
            };
            let rt_val = generator.gen_expr(ctx, value)?;
            match rt_val.val {
                Some(ValueEnum::Dynamic(v)) => {
                    let v = v.into_struct_value();
                    let result = ctx.builder.build_extract_value(v, index, "tup_elem").unwrap();
                    RtValue::dynamic(ty, result)
                }
                Some(ValueEnum::Static(v)) => {
                    if let Some(field_val) = v.get_tuple_element(index) {
                        let result = field_val.to_basic_value_enum(ctx, ty)?;
                        RtValue::dynamic(ty, result)
                    } else {
                        let tup =
                            v.to_basic_value_enum(ctx, value.custom.unwrap())?.into_struct_value();
                        let result =
                            ctx.builder.build_extract_value(tup, index, "tup_elem").unwrap();
                        RtValue::dynamic(ty, result)
                    }
                }
                None => RtValue::none(ty),
            }
        }
        _ => codegen_unreachable!(ctx, "should not be other subscriptable types after type check"),
    };
    Ok(res)
}

/// See [`CodeGenerator::gen_expr`].
pub fn gen_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    expr: &Expr<Option<Type>>,
) -> Result<RtValue<'ctx>, String> {
    ctx.current_loc = expr.location;

    let loc = ctx.debug_info.0.create_debug_location(
        ctx.ctx,
        ctx.current_loc.row as u32,
        ctx.current_loc.column as u32,
        ctx.debug_info.2,
        None,
    );
    ctx.builder.set_current_debug_location(loc);

    let ty = expr.custom.expect("expressions always have a well-defined type");

    match &expr.node {
        ExprKind::Constant { value, .. } => ctx
            .gen_const(value, ty)
            .map_or_else(|| Ok(RtValue::none(ty)), |const_val| Ok(RtValue::dynamic(ty, const_val))),
        ExprKind::Name { id, .. } if id == &"none".into() => match &*ctx.unifier.get_ty(ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
            {
                let val = OptionType::from_unifier_type(ctx, ty)
                    .construct_empty(ctx, None)
                    .as_abi_value(ctx);
                Ok(RtValue::dynamic(ty, val.into()))
            }
            _ => codegen_unreachable!(ctx, "must be option type"),
        },
        ExprKind::Name { id, .. } => match ctx.var_assignment.get(id) {
            Some((ptr, None, _)) => {
                let val = ctx.builder.build_load(*ptr, id.to_string().as_str()).unwrap();
                Ok(RtValue::dynamic(ty, val))
            }
            Some((_, Some(static_value), _)) => Ok(RtValue::r#static(ty, static_value.clone())),
            None => {
                let resolver = ctx.resolver.clone();
                let val = resolver.get_symbol_value(*id, ctx).unwrap();
                // get_symbol_value returns a ValueEnum
                Ok(RtValue { ty, val: Some(val) })
            }
        },
        ExprKind::List { elts, .. } => gen_list_expr(generator, ctx, ty, elts),
        ExprKind::Tuple { elts, .. } => gen_tuple_expr(generator, ctx, ty, elts),
        ExprKind::Attribute { value, attr, .. } => {
            gen_attr_expr(generator, ctx, ty, expr, value, *attr)
        }
        ExprKind::BoolOp { op, values } => gen_boolop_expr(generator, ctx, ty, *op, values),
        ExprKind::BinOp { op, left, right } => {
            gen_binop_expr(generator, ctx, left, Binop::normal(*op), right, expr.location, ty)
        }
        ExprKind::UnaryOp { op, operand } => gen_unaryop_expr(generator, ctx, *op, operand, ty),
        ExprKind::Compare { left, ops, comparators } => {
            gen_cmpop_expr(generator, ctx, left, ops, comparators, ty)
        }
        ExprKind::IfExp { test, body, orelse } => {
            gen_ifexp_expr(generator, ctx, ty, test, body, orelse)
        }
        ExprKind::Call { func, args, keywords } => {
            gen_call_expr(generator, ctx, ty, expr, func, args, keywords)
        }
        ExprKind::Subscript { value, slice, .. } => {
            gen_subscript_expr(generator, ctx, ty, expr, value, slice)
        }
        ExprKind::ListComp { .. } => (gen_comprehension(generator, ctx, expr)?)
            .map_or_else(|| Ok(RtValue::none(ty)), |v| Ok(RtValue::dynamic(ty, v))),
        _ => unimplemented!(),
    }
}

trait __ReturnType<'ctx> {
    type Value;
    fn into_ret_ty(this: Self) -> Option<BasicTypeEnum<'ctx>>;
    fn into_value(ret: Option<BasicValueEnum<'ctx>>) -> Self::Value;
}

macro_rules! impl_type {
    ($t:ty: |$self:ident| $into_ret:block, $v:ty: |$ret:ident| $into_val:block) => {
        impl<'ctx> __ReturnType<'ctx> for $t {
            type Value = $v;
            fn into_ret_ty($self: Self) -> Option<BasicTypeEnum<'ctx>> $into_ret
            fn into_value($ret: Option<BasicValueEnum<'ctx>>) -> Self::Value $into_val
        }
    };
    ($t:ident, $v:ident) => {
        impl_type!(
            [inkwell::types::$t<'ctx>; 1]: |this| { Some(this[0].into()) },
            inkwell::values::$v<'ctx>: |ret| { ret.unwrap().try_into().unwrap() }
        );
    };
}

#[doc(hidden)]
#[allow(private_bounds, reason = "macro internals")]
pub fn __handle_return_type<'ctx, T: __ReturnType<'ctx, Value = V>, V>(
    t: T,
) -> (Option<BasicTypeEnum<'ctx>>, impl FnOnce(Option<BasicValueEnum<'ctx>>) -> V) {
    (T::into_ret_ty(t), T::into_value)
}

impl_type!(ArrayType, ArrayValue);
impl_type!(FloatType, FloatValue);
impl_type!(IntType, IntValue);
impl_type!(PointerType, PointerValue);
impl_type!(StructType, StructValue);
impl_type!(VectorType, VectorValue);
impl_type!(ScalableVectorType, ScalableVectorValue);
impl_type!(BasicTypeEnum, BasicValueEnum);
impl_type!((): |_this| { None }, (): |ret| { assert!(ret.is_none()); });

#[doc(hidden)]
#[macro_export]
macro_rules! __codegen_call_extern_impl {
    (@ctx: [$($p:tt)*] ($ctx:expr) (:) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(@ret_ty: [$($p)* ($ctx)] $($t)*) };
    (@ctx: [$($p:tt)*] ($ctx:expr) $($t:tt)*) =>
        { compile_error!("expected `:` after context") };

    (@ret_ty: [$($p:tt)*] (void) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(@var_name: [$($p)* (())] $($t)*) };
    (@ret_ty: [$($p:tt)*] ($ret:expr) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(@var_name: [$($p)* ([$ret])] $($t)*) };

    (@var_name: [$($p:tt)*] (_) (=) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(attrs: [$($p)* (None)] $($t)*) };
    (@var_name: [$($p:tt)*] ($name:expr) (=) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(attrs: [$($p)* (Some($name))] $($t)*) };
    (@var_name: [$($p:tt)*] ($name:expr) (?) (=) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(attrs: [$($p)* ($name)] $($t)*) };
    (@var_name: [$($p:tt)*] ($name:expr) $($t:tt)*) =>
        { compile_error!("expected `=` after variable name") };

    (attrs: [$($p:tt)*] ([$($attr:literal)*]) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(@fn_name: [$($p)* ([$($attr),*])] $($t)*) };
    (attrs: [$($p:tt)*] ([$($attr:tt)*]) $($t:tt)*) =>
        { compile_error!(concat!("expected space-separated attrs, found ", stringify!([$($attr)*]))) };
    (attrs: [$($p:tt)*] $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(@fn_name: [$($p)* ([])] $($t)*)};

    (@fn_name: [$($p:tt)*] ($name:expr) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(fn_args: [$($p)* ($name)] $($t)*) };

    (fn_args: [$($p:tt)*] (($($arg:expr),* $(,)?)) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(final: [$($p)* ($($arg),*) (false) ()] $($t)*) };
    (fn_args: [$($p:tt)*] (($($arg:expr),* ; ...$varargs:expr)) $($t:tt)*) =>
        { $crate::__codegen_call_extern_impl!(final: [$($p)* ($($arg),*) (true) ($varargs)] $($t)*) };
    (fn_args: [$($p:tt)*] ($t:tt) $($rest:tt)*) =>
        { compile_error!(concat!("expected function args, found ", stringify!($t))) };

    (final: [$($p:tt)*] $(($t:tt))+) =>
        { compile_error!(concat!("extra tokens: ", stringify!($($t)+))) };
    (final: [($ctx:expr) ($ret_ty:expr) ($var_name:expr) ($fn_attrs:expr) ($fn_name:expr)
        ($($arg:expr),*) ($is_varargs:expr) ($($varargs:expr)?)]) =>
    {{
        let args = [$($arg.into()),*];
        let _: &[$crate::inkwell::values::BasicValueEnum<'_>] = &args;
        let types = args.map(|a| a.get_type());
        let (ret_ty, cast) = $crate::codegen::expr::__handle_return_type($ret_ty);
        $(let args = ::std::vec::Vec::from_iter(args.into_iter().chain($varargs));)?
        let result = $crate::codegen::expr::call_extern_c_fn(
            $ctx, &$fn_name, ret_ty, &types, &args, $is_varargs, $var_name, &$fn_attrs,
        );
        cast(result)
    }};

    // The first token for all @ branches is an expression.
    (@$stage:ident: [$($p:tt)*] ($this:tt) $($rest:tt)*) =>
        { compile_error!(concat!(stringify!($stage), ": expected expression, found ", stringify!($this))) };
    ($(@)?$stage:ident: [$($p:tt)*]) =>
        { compile_error!(concat!("missing ", stringify!($stage))) };
    ($($t:tt)*) =>
        { compile_error!(concat!("internal error: could not parse ", stringify!($($t)*))) };
}

/// Emits an external function call.
///
/// Operates on a `&mut CodeGenContext`, declares an external function with inferred
/// argument types and given return type, then builds a function call to get the corresponding
/// result.
///
/// Typical usage looks like a C function call with an assignment to a local variable,
/// except that "types", "variable names" and "function names" are now expressions rather
/// than identifiers.
///
/// ```ignore
/// call_extern!(ctx: llvm_i32 "var_name" = "fn_name"(arg0, arg1));
/// ```
///
/// This emits a function declaration and a call to the function, with adjustments needed for
/// the C ABI:
///
/// ```llvm
/// ; types are automatically inferred from the arguments %argX
/// declare i32 @fn_name(type0, type1)
///
/// %var_name = call i32 @fn_name(type0 %arg0, type1 %arg1)
/// ```
///
/// # Syntax
///
/// ```ignore
/// call_extern!(ctx: ret_ty var_name = ["attr0" "attr1"] fn_name(arg0, arg1; ...varargs))
/// ```
///
/// - `ctx`: A `&mut CodeGenContext` to build the call instruction into.
///
/// - `ret_ty`: The function return type. Either:
///   - one of the variants of [`BasicTypeEnum`] (the result will be a value of that type); or
///   - a [`BasicTypeEnum`] (the result will be a [`BasicValueEnum`]); or
///   - `void` (no return value).
///
/// - `var_name`: The variable name to assign to the result. Must be of type [`&str`].
///   You can use these instead:
///   - `_`:  Leave the variable unnamed; LLVM will assign a numerical index.
///   - `var_name?`: Optionally name the variable. `var_name` must be of type [`Option<&str>`].
///
/// - `["attr0" "attr1"]` _(optional)_: Function attributes ([docs][attr-docs]). Should be
///    a list of space-separated string literals.
///
/// - `fn_name`: The name of the function. `&fn_name` must coerce into [`&str`].
///
/// - `arg0, arg1`: Function arguments. The parameter types of the function are deduced from these values.
///   Each argument can be any `impl Into<BasicValueEnum<'ctx>>`.
///
/// - `varargs`: Variadic arguments. Can be any `impl IntoIterator<Item = BasicValueEnum<'ctx>>`.
///
/// Note that `ctx`, `ret_ty`, `var_name` and `fn_name` must be an expression consisting of a single
/// _token tree_. That is, you should parenthesize any expression that is not just a string literal
/// or variable identifier.
///
/// Even if you intend to discard the result, you must pass the exact same return type as your C function
/// definition, for ABI reasons.
///
/// # Examples
///
/// Call an external function with some attributes:
///
/// ```no_run
/// # use nac3core::codegen::{CodeGenContext, expr::call_extern};
/// # fn test(ctx: &mut CodeGenContext) {
/// let success = ctx.i32.const_zero();
/// call_extern!(ctx: void _ = ["noreturn"] "_Exit"(success));
/// # }
/// ```
///
/// Call a variadic function:
///
/// ```no_run
/// # use nac3core::codegen::{CodeGenContext, expr::call_extern};
/// # use nac3core::inkwell::{values::IntValue, builder::BuilderError};
/// # fn test<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>) -> Result<(), BuilderError> {
/// let int = ctx.i32;
/// let neg_one = int.const_all_ones();
/// let half = ctx.ctx.f32_type().const_float(0.5);
/// let format = ctx.builder.build_global_string_ptr("%d %.2f", "fmt_str")?.as_pointer_value();
///
/// // unlike positional args, variadic args need an explicit cast to BasicValueEnum
/// let varargs = [neg_one.into(), half.into()];
///
/// // at runtime, prints "-1 0.50"; written = 7
/// let written: IntValue<'ctx> = call_extern!(ctx: int "written" = "printf"(format; ...varargs));
/// # Ok(()) }
/// ```
///
/// [attr-docs]: https://llvm.org/docs/LangRef.html#fnattrs
#[doc(hidden)]
#[macro_export]
macro_rules! __codegen_call_extern {
    ($($t:tt)*) => { $crate::__codegen_call_extern_impl!(@ctx: [] $(($t))*) };
}

#[doc(inline)]
pub use __codegen_call_extern as call_extern;

/// Call an external C function, given a function signature and arguments.
///
/// You might want to use the [`call_extern`] macro instead, which is easier to use
/// as it deduces and converts types automatically.
///
/// For repeated function calls and dynamically added external bindings, you might want to use
/// [`CoreContext::declare_external`] and [`CodeGenContext::build_call_or_invoke`] directly.
///
/// [`CoreContext::declare_external`]: crate::codegen::CoreContext::declare_external
#[allow(clippy::too_many_arguments, reason = "most users use the call_extern macro instead")]
pub fn call_extern_c_fn<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    fn_name: &str,
    ret_type: Option<BasicTypeEnum<'ctx>>,
    param_types: &[BasicTypeEnum<'ctx>],
    args: &[BasicValueEnum<'ctx>],
    is_c_varargs: bool,
    value_name: Option<&str>,
    fn_attrs: &[&str],
) -> Option<BasicValueEnum<'ctx>> {
    let f = ctx.declare_external(fn_name, ret_type, param_types, is_c_varargs, fn_attrs);
    ctx.build_call(&f, args, value_name.unwrap_or(""))
}
