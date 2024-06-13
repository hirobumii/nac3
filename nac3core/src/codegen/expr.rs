use std::{collections::HashMap, convert::TryInto, iter::once, iter::zip};

use crate::{
    codegen::{
        classes::{
            ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ListValue, NDArrayValue, ProxyValue,
            RangeValue, TypedArrayLikeAccessor, UntypedArrayLikeAccessor,
        },
        concrete_type::{ConcreteFuncArg, ConcreteTypeEnum, ConcreteTypeStore},
        gen_in_range_check, get_llvm_abi_type, get_llvm_type,
        irrt::*,
        llvm_intrinsics::{call_expect, call_float_floor, call_float_pow, call_float_powi},
        numpy,
        stmt::{gen_if_else_expr_callback, gen_raise, gen_var},
        CodeGenContext, CodeGenTask,
    },
    symbol_resolver::{SymbolValue, ValueEnum},
    toplevel::{
        helper::PrimDef,
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
        DefinitionId, TopLevelDef,
    },
    typecheck::{
        magic_methods::{binop_assign_name, binop_name, unaryop_name},
        typedef::{FunSignature, FuncArg, Type, TypeEnum, TypeVarId, Unifier, VarMap},
    },
};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    types::{AnyType, BasicType, BasicTypeEnum},
    values::{BasicValueEnum, CallSiteValue, FunctionValue, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};
use itertools::{chain, izip, Either, Itertools};
use nac3parser::ast::{
    self, Boolop, Comprehension, Constant, Expr, ExprKind, Location, Operator, StrRef,
};

use super::{llvm_intrinsics::call_memcpy_generic, need_sret, CodeGenerator};

pub fn get_subst_key(
    unifier: &mut Unifier,
    obj: Option<Type>,
    fun_vars: &VarMap,
    filter: Option<&Vec<TypeVarId>>,
) -> String {
    let mut vars = obj
        .map(|ty| {
            let TypeEnum::TObj { params, .. } = &*unifier.get_ty(ty) else { unreachable!() };
            params.clone()
        })
        .unwrap_or_default();
    vars.extend(fun_vars);
    let sorted = vars.keys().filter(|id| filter.map_or(true, |v| v.contains(id))).sorted();
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

impl<'ctx, 'a> CodeGenContext<'ctx, 'a> {
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

    fn get_subst_key(
        &mut self,
        obj: Option<Type>,
        fun: &FunSignature,
        filter: Option<&Vec<TypeVarId>>,
    ) -> String {
        get_subst_key(&mut self.unifier, obj, &fun.vars, filter)
    }

    pub fn get_attr_index(&mut self, ty: Type, attr: StrRef) -> usize {
        let obj_id = match &*self.unifier.get_ty(ty) {
            TypeEnum::TObj { obj_id, .. } => *obj_id,
            // we cannot have other types, virtual type should be handled by function calls
            _ => unreachable!(),
        };
        let def = &self.top_level.definitions.read()[obj_id.0];
        let index = if let TopLevelDef::Class { fields, .. } = &*def.read() {
            fields.iter().find_position(|x| x.0 == attr).unwrap().0
        } else {
            unreachable!()
        };
        index
    }

    pub fn gen_symbol_val<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        val: &SymbolValue,
        ty: Type,
    ) -> BasicValueEnum<'ctx> {
        match val {
            SymbolValue::I32(v) => self.ctx.i32_type().const_int(*v as u64, true).into(),
            SymbolValue::I64(v) => self.ctx.i64_type().const_int(*v as u64, true).into(),
            SymbolValue::U32(v) => self.ctx.i32_type().const_int(u64::from(*v), false).into(),
            SymbolValue::U64(v) => self.ctx.i64_type().const_int(*v, false).into(),
            SymbolValue::Bool(v) => self.ctx.i8_type().const_int(u64::from(*v), true).into(),
            SymbolValue::Double(v) => self.ctx.f64_type().const_float(*v).into(),
            SymbolValue::Str(v) => {
                let str_ptr = self
                    .builder
                    .build_global_string_ptr(v, "const")
                    .map(|v| v.as_pointer_value().into())
                    .unwrap();
                let size = generator.get_size_type(self.ctx).const_int(v.len() as u64, false);
                let ty = self.get_llvm_type(generator, self.primitives.str).into_struct_type();
                ty.const_named_struct(&[str_ptr, size.into()]).into()
            }
            SymbolValue::Tuple(ls) => {
                let vals = ls.iter().map(|v| self.gen_symbol_val(generator, v, ty)).collect_vec();
                let fields = vals.iter().map(BasicValueEnum::get_type).collect_vec();
                let ty = self.ctx.struct_type(&fields, false);
                let ptr = gen_var(self, ty.into(), Some("tuple")).unwrap();
                let zero = self.ctx.i32_type().const_zero();
                unsafe {
                    for (i, val) in vals.into_iter().enumerate() {
                        let p = self
                            .builder
                            .build_in_bounds_gep(
                                ptr,
                                &[zero, self.ctx.i32_type().const_int(i as u64, false)],
                                "elemptr",
                            )
                            .unwrap();
                        self.builder.build_store(p, val).unwrap();
                    }
                }
                self.builder.build_load(ptr, "tup_val").unwrap()
            }
            SymbolValue::OptionSome(v) => {
                let ty = match self.unifier.get_ty_immutable(ty).as_ref() {
                    TypeEnum::TObj { obj_id, params, .. }
                        if *obj_id == self.primitives.option.obj_id(&self.unifier).unwrap() =>
                    {
                        *params.iter().next().unwrap().1
                    }
                    _ => unreachable!("must be option type"),
                };
                let val = self.gen_symbol_val(generator, v, ty);
                let ptr = generator
                    .gen_var_alloc(self, val.get_type(), Some("default_opt_some"))
                    .unwrap();
                self.builder.build_store(ptr, val).unwrap();
                ptr.into()
            }
            SymbolValue::OptionNone => {
                let ty = match self.unifier.get_ty_immutable(ty).as_ref() {
                    TypeEnum::TObj { obj_id, params, .. }
                        if *obj_id == self.primitives.option.obj_id(&self.unifier).unwrap() =>
                    {
                        *params.iter().next().unwrap().1
                    }
                    _ => unreachable!("must be option type"),
                };
                let actual_ptr_type =
                    self.get_llvm_type(generator, ty).ptr_type(AddressSpace::default());
                actual_ptr_type.const_null().into()
            }
        }
    }

    /// See [`get_llvm_type`].
    pub fn get_llvm_type<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        ty: Type,
    ) -> BasicTypeEnum<'ctx> {
        get_llvm_type(
            self.ctx,
            &self.module,
            generator,
            &mut self.unifier,
            self.top_level,
            &mut self.type_cache,
            ty,
        )
    }

    /// See [`get_llvm_abi_type`].
    pub fn get_llvm_abi_type<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        ty: Type,
    ) -> BasicTypeEnum<'ctx> {
        get_llvm_abi_type(
            self.ctx,
            &self.module,
            generator,
            &mut self.unifier,
            self.top_level,
            &mut self.type_cache,
            &self.primitives,
            ty,
        )
    }

    /// Generates an LLVM variable for a [constant value][value] with a given [type][ty].
    pub fn gen_const<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        value: &Constant,
        ty: Type,
    ) -> Option<BasicValueEnum<'ctx>> {
        match value {
            Constant::Bool(v) => {
                assert!(self.unifier.unioned(ty, self.primitives.bool));
                let ty = self.ctx.i8_type();
                Some(ty.const_int(u64::from(*v), false).into())
            }
            Constant::Int(val) => {
                let ty = if self.unifier.unioned(ty, self.primitives.int32)
                    || self.unifier.unioned(ty, self.primitives.uint32)
                {
                    self.ctx.i32_type()
                } else if self.unifier.unioned(ty, self.primitives.int64)
                    || self.unifier.unioned(ty, self.primitives.uint64)
                {
                    self.ctx.i64_type()
                } else {
                    unreachable!()
                };
                Some(ty.const_int(*val as u64, false).into())
            }
            Constant::Float(v) => {
                assert!(self.unifier.unioned(ty, self.primitives.float));
                let ty = self.ctx.f64_type();
                Some(ty.const_float(*v).into())
            }
            Constant::Tuple(v) => {
                let ty = self.unifier.get_ty(ty);
                let types =
                    if let TypeEnum::TTuple { ty } = &*ty { ty.clone() } else { unreachable!() };
                let values = zip(types, v.iter())
                    .map_while(|(ty, v)| self.gen_const(generator, v, ty))
                    .collect_vec();

                if values.len() == v.len() {
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
                    let str_ptr = self
                        .builder
                        .build_global_string_ptr(v, "const")
                        .map(|v| v.as_pointer_value().into())
                        .unwrap();
                    let size = generator.get_size_type(self.ctx).const_int(v.len() as u64, false);
                    let ty = self.get_llvm_type(generator, self.primitives.str);
                    let val =
                        ty.into_struct_type().const_named_struct(&[str_ptr, size.into()]).into();
                    self.const_strings.insert(v.to_string(), val);
                    Some(val)
                }
            }
            Constant::Ellipsis => {
                let msg = self.gen_string(generator, "NotImplementedError");

                self.raise_exn(
                    generator,
                    "0:NotImplementedError",
                    msg,
                    [None, None, None],
                    self.current_loc,
                );

                None
            }
            _ => unreachable!(),
        }
    }

    /// Generates a binary operation `op` between two integral operands `lhs` and `rhs`.
    pub fn gen_int_ops<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        op: Operator,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        signed: bool,
    ) -> BasicValueEnum<'ctx> {
        let (BasicValueEnum::IntValue(lhs), BasicValueEnum::IntValue(rhs)) = (lhs, rhs) else {
            unreachable!()
        };
        let float = self.ctx.f64_type();
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
                    generator,
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
                    _ => unreachable!(),
                }
            }

            (Operator::FloorDiv, true) => {
                self.builder.build_int_signed_div(lhs, rhs, "floordiv").map(Into::into).unwrap()
            }
            (Operator::FloorDiv, false) => {
                self.builder.build_int_unsigned_div(lhs, rhs, "floordiv").map(Into::into).unwrap()
            }
            (Operator::Pow, s) => integer_power(generator, self, lhs, rhs, s).into(),
            // special implementation?
            (Operator::MatMult, _) => unreachable!(),
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
            unreachable!(
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

    pub fn build_call_or_invoke(
        &mut self,
        fun: FunctionValue<'ctx>,
        params: &[BasicValueEnum<'ctx>],
        call_name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let mut loc_params: Vec<BasicValueEnum<'ctx>> = Vec::new();
        let mut return_slot = None;

        let loc = self.debug_info.0.create_debug_location(
            self.ctx,
            self.current_loc.row as u32,
            self.current_loc.column as u32,
            self.debug_info.2,
            None,
        );
        self.builder.set_current_debug_location(loc);

        if fun.count_params() > 0 {
            let sret_id = Attribute::get_named_enum_kind_id("sret");
            let byref_id = Attribute::get_named_enum_kind_id("byref");
            let byval_id = Attribute::get_named_enum_kind_id("byval");

            let offset = if fun.get_enum_attribute(AttributeLoc::Param(0), sret_id).is_some() {
                return_slot = Some(
                    self.builder
                        .build_alloca(
                            fun.get_type().get_param_types()[0]
                                .into_pointer_type()
                                .get_element_type()
                                .into_struct_type(),
                            call_name,
                        )
                        .unwrap(),
                );
                loc_params.push((*return_slot.as_ref().unwrap()).into());
                1
            } else {
                0
            };
            for (i, param) in params.iter().enumerate() {
                let loc = AttributeLoc::Param((i + offset) as u32);
                if fun.get_enum_attribute(loc, byref_id).is_some()
                    || fun.get_enum_attribute(loc, byval_id).is_some()
                {
                    // lazy update
                    if loc_params.is_empty() {
                        loc_params.extend(params[0..i + offset].iter().copied());
                    }
                    let slot = gen_var(self, param.get_type(), Some(call_name)).unwrap();
                    loc_params.push(slot.into());
                    self.builder.build_store(slot, *param).unwrap();
                } else if !loc_params.is_empty() {
                    loc_params.push(*param);
                }
            }
        }
        let params = if loc_params.is_empty() { params } else { &loc_params };
        let params = fun
            .get_type()
            .get_param_types()
            .into_iter()
            .zip(params.iter())
            .map(|(ty, val)| match (ty, val.get_type()) {
                (BasicTypeEnum::PointerType(arg_ty), BasicTypeEnum::PointerType(val_ty))
                    if {
                        ty != val.get_type()
                            && arg_ty.get_element_type().is_struct_type()
                            && val_ty.get_element_type().is_struct_type()
                    } =>
                {
                    self.builder.build_bitcast(*val, arg_ty, "call_arg_cast").unwrap()
                }
                _ => *val,
            })
            .collect_vec();
        let result = if let Some(target) = self.unwind_target {
            let current = self.builder.get_insert_block().unwrap().get_parent().unwrap();
            let then_block = self.ctx.append_basic_block(current, &format!("after.{call_name}"));
            let result = self
                .builder
                .build_invoke(fun, &params, then_block, target, call_name)
                .map(CallSiteValue::try_as_basic_value)
                .map(Either::left)
                .unwrap();
            self.builder.position_at_end(then_block);
            result
        } else {
            let param: Vec<_> = params.iter().map(|v| (*v).into()).collect();
            self.builder
                .build_call(fun, &param, call_name)
                .map(CallSiteValue::try_as_basic_value)
                .map(Either::left)
                .unwrap()
        };
        if let Some(slot) = return_slot {
            Some(self.builder.build_load(slot, call_name).unwrap())
        } else {
            result
        }
    }

    /// Helper function for generating a LLVM variable storing a [String].
    pub fn gen_string<G, S>(&mut self, generator: &mut G, s: S) -> BasicValueEnum<'ctx>
    where
        G: CodeGenerator + ?Sized,
        S: Into<String>,
    {
        self.gen_const(generator, &Constant::Str(s.into()), self.primitives.str).unwrap()
    }

    pub fn raise_exn<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        name: &str,
        msg: BasicValueEnum<'ctx>,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let zelf = if let Some(exception_val) = self.exception_val {
            exception_val
        } else {
            let ty = self.get_llvm_type(generator, self.primitives.exception).into_pointer_type();
            let zelf_ty: BasicTypeEnum = ty.get_element_type().into_struct_type().into();
            let zelf = generator.gen_var_alloc(self, zelf_ty, Some("exn")).unwrap();
            *self.exception_val.insert(zelf)
        };
        let int32 = self.ctx.i32_type();
        let zero = int32.const_zero();
        unsafe {
            let id_ptr = self.builder.build_in_bounds_gep(zelf, &[zero, zero], "exn.id").unwrap();
            let id = self.resolver.get_string_id(name);
            self.builder.build_store(id_ptr, int32.const_int(id as u64, false)).unwrap();
            let ptr = self
                .builder
                .build_in_bounds_gep(zelf, &[zero, int32.const_int(5, false)], "exn.msg")
                .unwrap();
            self.builder.build_store(ptr, msg).unwrap();
            let i64_zero = self.ctx.i64_type().const_zero();
            for (i, attr_ind) in [6, 7, 8].iter().enumerate() {
                let ptr = self
                    .builder
                    .build_in_bounds_gep(
                        zelf,
                        &[zero, int32.const_int(*attr_ind, false)],
                        "exn.param",
                    )
                    .unwrap();
                let val = params[i].map_or(i64_zero, |v| {
                    self.builder.build_int_s_extend(v, self.ctx.i64_type(), "sext").unwrap()
                });
                self.builder.build_store(ptr, val).unwrap();
            }
        }
        gen_raise(generator, self, Some(&zelf.into()), loc);
    }

    pub fn make_assert<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        cond: IntValue<'ctx>,
        err_name: &str,
        err_msg: &str,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let err_msg = self.gen_string(generator, err_msg);
        self.make_assert_impl(generator, cond, err_name, err_msg, params, loc);
    }

    pub fn make_assert_impl<G: CodeGenerator + ?Sized>(
        &mut self,
        generator: &mut G,
        cond: IntValue<'ctx>,
        err_name: &str,
        err_msg: BasicValueEnum<'ctx>,
        params: [Option<IntValue<'ctx>>; 3],
        loc: Location,
    ) {
        let i1 = self.ctx.bool_type();
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
        self.raise_exn(generator, err_name, err_msg, params, loc);
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
    let TopLevelDef::Class { methods, .. } = def else { unreachable!() };

    // TODO: what about other fields that require alloca?
    let fun_id = methods.iter().find(|method| method.0 == "__init__".into()).map(|method| method.2);
    let ty = ctx.get_llvm_type(generator, signature.ret).into_pointer_type();
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
pub fn gen_func_instance<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
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
        unreachable!()
    };

    if let Some(sym) = instance_to_symbol.get(&key) {
        return Ok(sym.clone());
    }
    let symbol = format!("{}.{}", name, instance_to_symbol.len());
    instance_to_symbol.insert(key, symbol.clone());
    let mut filter = var_id.clone();
    if let Some((obj_ty, _)) = &obj {
        if let TypeEnum::TObj { params, .. } = &*ctx.unifier.get_ty(*obj_ty) {
            filter.extend(params.keys());
        }
    }
    let key = ctx.get_subst_key(obj.as_ref().map(|a| a.0), sign, Some(&filter));
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

    if let Some(obj) = &obj {
        let zelf = store.from_unifier_type(&mut ctx.unifier, &ctx.primitives, obj.0, &mut cache);
        let ConcreteTypeEnum::TFunc { args, .. } = &mut signature else { unreachable!() };

        args.insert(0, ConcreteFuncArg { name: "self".into(), ty: zelf, default_value: None });
    }
    let signature = store.add_cty(signature);

    ctx.registry.add_task(CodeGenTask {
        symbol_name: symbol.clone(),
        body: instance.body.clone(),
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
    let definition = ctx.top_level.definitions.read().get(fun.1 .0).cloned().unwrap();
    let id;
    let key;
    let param_vals;
    let is_extern;

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
                    return callback.run(ctx, obj, fun, params, generator);
                }
                is_extern = instance_to_stmt.is_empty();
                let old_key = ctx.get_subst_key(obj.as_ref().map(|a| a.0), fun.0, None);
                let mut keys = fun.0.args.clone();
                let mut mapping = HashMap::new();
                for (key, value) in params {
                    mapping.insert(key.unwrap_or_else(|| keys.remove(0).name), value);
                }
                // default value handling
                for k in keys {
                    if mapping.get(&k.name).is_some() {
                        continue;
                    }
                    mapping.insert(
                        k.name,
                        ctx.gen_symbol_val(generator, &k.default_value.unwrap(), k.ty).into(),
                    );
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
                        if let ValueEnum::Static(s) = v {
                            Some((i, s.clone()))
                        } else {
                            None
                        }
                    })
                    .collect_vec();
                id = {
                    let ids = static_params
                        .iter()
                        .map(|(i, v)| (*i, v.get_unique_identifier()))
                        .collect_vec();
                    let mut store = ctx.static_value_store.lock();
                    if let Some(index) = store.lookup.get(&ids) {
                        *index
                    } else {
                        let length = store.store.len();
                        store.lookup.insert(ids, length);
                        store.store.push(static_params.into_iter().collect());
                        length
                    }
                };
                // special case: extern functions
                key = if instance_to_stmt.is_empty() {
                    String::new()
                } else {
                    format!("{id}:{old_key}")
                };
                param_vals = real_params
                    .into_iter()
                    .map(|(p, t)| p.to_basic_value_enum(ctx, generator, t))
                    .collect::<Result<Vec<_>, String>>()?;
                instance_to_symbol.get(&key).cloned().ok_or_else(String::new)
            }
            TopLevelDef::Class { .. } => {
                return Ok(Some(generator.gen_constructor(ctx, fun.0, &def, params)?))
            }
        }
    }
    .or_else(|_: String| {
        generator.gen_func_instance(ctx, obj.clone(), (fun.0, &mut *definition.write(), key), id)
    })?;
    let fun_val = ctx.module.get_function(&symbol).unwrap_or_else(|| {
        let mut args = fun.0.args.clone();
        if let Some(obj) = &obj {
            args.insert(0, FuncArg { name: "self".into(), ty: obj.0, default_value: None });
        }
        let ret_type = if ctx.unifier.unioned(fun.0.ret, ctx.primitives.none) {
            None
        } else {
            Some(ctx.get_llvm_abi_type(generator, fun.0.ret))
        };
        let has_sret = ret_type.map_or(false, |ret_type| need_sret(ret_type));
        let mut byrefs = Vec::new();
        let mut params = args
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                match ctx.get_llvm_abi_type(generator, arg.ty) {
                    BasicTypeEnum::StructType(ty) if is_extern => {
                        byrefs.push((i, ty));
                        ty.ptr_type(AddressSpace::default()).into()
                    }
                    x => x,
                }
                .into()
            })
            .collect_vec();
        if has_sret {
            params.insert(0, ret_type.unwrap().ptr_type(AddressSpace::default()).into());
        }
        let fun_ty = match ret_type {
            Some(ret_type) if !has_sret => ret_type.fn_type(&params, false),
            _ => ctx.ctx.void_type().fn_type(&params, false),
        };
        let fun_val = ctx.module.add_function(&symbol, fun_ty, None);
        let offset = if has_sret {
            fun_val.add_attribute(
                AttributeLoc::Param(0),
                ctx.ctx.create_type_attribute(
                    Attribute::get_named_enum_kind_id("sret"),
                    ret_type.unwrap().as_any_type_enum(),
                ),
            );
            1
        } else {
            0
        };

        // The attribute ID used to mark arguments of a structure type.
        // Structure-Typed parameters of extern functions must **not** be marked as `byval`, as
        // `byval` explicitly specifies that the argument is to be passed on the stack, which breaks
        // on most ABIs where the first several arguments are expected to be passed in registers.
        let passing_attr_id =
            Attribute::get_named_enum_kind_id(if is_extern { "byref" } else { "byval" });
        for (i, ty) in byrefs {
            fun_val.add_attribute(
                AttributeLoc::Param((i as u32) + offset),
                ctx.ctx.create_type_attribute(passing_attr_id, ty.as_any_type_enum()),
            );
        }
        fun_val
    });

    // Convert boolean parameter values into i1
    let param_vals = fun_val
        .get_params()
        .iter()
        .zip(param_vals)
        .map(|(p, v)| {
            if p.is_int_value() && v.is_int_value() {
                let expected_ty = p.into_int_value().get_type();
                let param_val = v.into_int_value();

                if expected_ty.get_bit_width() == 1 && param_val.get_type().get_bit_width() != 1 {
                    generator.bool_to_i1(ctx, param_val)
                } else {
                    param_val
                }
                .into()
            } else {
                v
            }
        })
        .collect_vec();

    Ok(ctx.build_call_or_invoke(fun_val, &param_vals, "call"))
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

/// Allocates a List structure with the given [type][ty] and [length]. The name of the resulting
/// LLVM value is `{name}.addr`, or `list.addr` if [name] is not specified.
///
/// Returns an instance of [`PointerValue`] pointing to the List structure. The List structure is
/// defined as `type { ty*, size_t }` in LLVM, where the first element stores the pointer to the
/// data, and the second element stores the size of the List.
pub fn allocate_list<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: BasicTypeEnum<'ctx>,
    length: IntValue<'ctx>,
    name: Option<&str>,
) -> ListValue<'ctx> {
    let size_t = generator.get_size_type(ctx.ctx);
    // List structure; type { ty*, size_t }
    let arr_ty =
        ctx.ctx.struct_type(&[ty.ptr_type(AddressSpace::default()).into(), size_t.into()], false);

    let arr_str_ptr = ctx
        .builder
        .build_alloca(arr_ty, format!("{}.addr", name.unwrap_or("list")).as_str())
        .unwrap();
    let list = ListValue::from_ptr_val(arr_str_ptr, size_t, Some("list"));

    let length = ctx.builder.build_int_z_extend(length, size_t, "").unwrap();
    list.store_size(ctx, generator, length);
    list.create_data(ctx, ty, None);

    list
}

/// Generates LLVM IR for a [list comprehension expression][expr].
pub fn gen_comprehension<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    expr: &Expr<Option<Type>>,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let ExprKind::ListComp { elt, generators } = &expr.node else { unreachable!() };

    let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();

    let init_bb = ctx.ctx.append_basic_block(current, "listcomp.init");
    let test_bb = ctx.ctx.append_basic_block(current, "listcomp.test");
    let body_bb = ctx.ctx.append_basic_block(current, "listcomp.body");
    let cont_bb = ctx.ctx.append_basic_block(current, "listcomp.cont");

    ctx.builder.build_unconditional_branch(init_bb).unwrap();

    ctx.builder.position_at_end(init_bb);

    let Comprehension { target, iter, ifs, .. } = &generators[0];
    let iter_val = if let Some(v) = generator.gen_expr(ctx, iter)? {
        v.to_basic_value_enum(ctx, generator, iter.custom.unwrap())?
    } else {
        for bb in [test_bb, body_bb, cont_bb] {
            ctx.builder.position_at_end(bb);
            ctx.builder.build_unreachable().unwrap();
        }

        return Ok(None);
    };
    let int32 = ctx.ctx.i32_type();
    let size_t = generator.get_size_type(ctx.ctx);
    let zero_size_t = size_t.const_zero();
    let zero_32 = int32.const_zero();

    let index = generator.gen_var_alloc(ctx, size_t.into(), Some("index.addr"))?;
    ctx.builder.build_store(index, zero_size_t).unwrap();

    let elem_ty = ctx.get_llvm_type(generator, elt.custom.unwrap());
    let is_range = ctx.unifier.unioned(iter.custom.unwrap(), ctx.primitives.range);
    let list;
    let list_content;

    if is_range {
        let iter_val = RangeValue::from_ptr_val(iter_val.into_pointer_value(), Some("range"));
        let (start, stop, step) = destructure_range(ctx, iter_val);
        let diff = ctx.builder.build_int_sub(stop, start, "diff").unwrap();
        // add 1 to the length as the value is rounded to zero
        // the length may be 1 more than the actual length if the division is exact, but the
        // length is a upper bound only anyway so it does not matter.
        let length = ctx.builder.build_int_signed_div(diff, step, "div").unwrap();
        let length = ctx.builder.build_int_add(length, int32.const_int(1, false), "add1").unwrap();
        // in case length is non-positive
        let is_valid =
            ctx.builder.build_int_compare(IntPredicate::SGT, length, zero_32, "check").unwrap();

        let list_alloc_size = ctx
            .builder
            .build_select(
                is_valid,
                ctx.builder.build_int_z_extend_or_bit_cast(length, size_t, "z_ext_len").unwrap(),
                zero_size_t,
                "listcomp.alloc_size",
            )
            .unwrap();
        list = allocate_list(
            generator,
            ctx,
            elem_ty,
            list_alloc_size.into_int_value(),
            Some("listcomp.addr"),
        );
        list_content = list.data().base_ptr(ctx, generator);

        let i = generator.gen_store_target(ctx, target, Some("i.addr"))?.unwrap();
        ctx.builder
            .build_store(i, ctx.builder.build_int_sub(start, step, "start_init").unwrap())
            .unwrap();

        ctx.builder
            .build_conditional_branch(gen_in_range_check(ctx, start, stop, step), test_bb, cont_bb)
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
            .build_conditional_branch(gen_in_range_check(ctx, tmp, stop, step), body_bb, cont_bb)
            .unwrap();

        ctx.builder.position_at_end(body_bb);
    } else {
        let length = ctx
            .build_gep_and_load(
                iter_val.into_pointer_value(),
                &[zero_size_t, int32.const_int(1, false)],
                Some("length"),
            )
            .into_int_value();
        list = allocate_list(generator, ctx, elem_ty, length, Some("listcomp"));
        list_content = list.data().base_ptr(ctx, generator);
        let counter = generator.gen_var_alloc(ctx, size_t.into(), Some("counter.addr"))?;
        // counter = -1
        ctx.builder.build_store(counter, size_t.const_int(u64::MAX, true)).unwrap();
        ctx.builder.build_unconditional_branch(test_bb).unwrap();

        ctx.builder.position_at_end(test_bb);
        let tmp = ctx.builder.build_load(counter, "i").map(BasicValueEnum::into_int_value).unwrap();
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
        generator.gen_assign(ctx, target, val.into())?;
    }

    // Emits the content of `cont_bb`
    let emit_cont_bb =
        |ctx: &CodeGenContext<'ctx, '_>, generator: &dyn CodeGenerator, list: ListValue<'ctx>| {
            ctx.builder.position_at_end(cont_bb);
            list.store_size(
                ctx,
                generator,
                ctx.builder.build_load(index, "index").map(BasicValueEnum::into_int_value).unwrap(),
            );
        };

    for cond in ifs {
        let result = if let Some(v) = generator.gen_expr(ctx, cond)? {
            v.to_basic_value_enum(ctx, generator, cond.custom.unwrap())?.into_int_value()
        } else {
            // Bail if the predicate is an ellipsis - Emit cont_bb contents in case the
            // no element matches the predicate
            emit_cont_bb(ctx, generator, list);

            return Ok(None);
        };
        let result = generator.bool_to_i1(ctx, result);
        let succ = ctx.ctx.append_basic_block(current, "then");
        ctx.builder.build_conditional_branch(result, succ, test_bb).unwrap();

        ctx.builder.position_at_end(succ);
    }

    let Some(elem) = generator.gen_expr(ctx, elt)? else {
        // Similarly, bail if the generator expression is an ellipsis, but keep cont_bb contents
        emit_cont_bb(ctx, generator, list);

        return Ok(None);
    };
    let i = ctx.builder.build_load(index, "i").map(BasicValueEnum::into_int_value).unwrap();
    let elem_ptr = unsafe { ctx.builder.build_gep(list_content, &[i], "elem_ptr") }.unwrap();
    let val = elem.to_basic_value_enum(ctx, generator, elt.custom.unwrap())?;
    ctx.builder.build_store(elem_ptr, val).unwrap();
    ctx.builder
        .build_store(
            index,
            ctx.builder.build_int_add(i, size_t.const_int(1, false), "inc").unwrap(),
        )
        .unwrap();
    ctx.builder.build_unconditional_branch(test_bb).unwrap();

    emit_cont_bb(ctx, generator, list);

    Ok(Some(list.as_base_value().into()))
}

/// Generates LLVM IR for a binary operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_binop_expr_with_values<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: (&Option<Type>, BasicValueEnum<'ctx>),
    op: Operator,
    right: (&Option<Type>, BasicValueEnum<'ctx>),
    loc: Location,
    is_aug_assign: bool,
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let (left_ty, left_val) = left;
    let (right_ty, right_val) = right;

    let ty1 = ctx.unifier.get_representative(left_ty.unwrap());
    let ty2 = ctx.unifier.get_representative(right_ty.unwrap());

    // we can directly compare the types, because we've got their representatives
    // which would be unchanged until further unification, which we would never do
    // when doing code generation for function instances
    if ty1 == ty2 && [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1) {
        Ok(Some(ctx.gen_int_ops(generator, op, left_val, right_val, true).into()))
    } else if ty1 == ty2 && [ctx.primitives.uint32, ctx.primitives.uint64].contains(&ty1) {
        Ok(Some(ctx.gen_int_ops(generator, op, left_val, right_val, false).into()))
    } else if [Operator::LShift, Operator::RShift].contains(&op) {
        let signed = [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1);
        Ok(Some(ctx.gen_int_ops(generator, op, left_val, right_val, signed).into()))
    } else if ty1 == ty2 && ctx.primitives.float == ty1 {
        Ok(Some(ctx.gen_float_ops(op, left_val, right_val).into()))
    } else if ty1 == ctx.primitives.float && ty2 == ctx.primitives.int32 {
        // Pow is the only operator that would pass typecheck between float and int
        assert_eq!(op, Operator::Pow);
        let res = call_float_powi(
            ctx,
            left_val.into_float_value(),
            right_val.into_int_value(),
            Some("f_pow_i"),
        );
        Ok(Some(res.into()))
    } else if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
        || ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
    {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let is_ndarray1 = ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
        let is_ndarray2 = ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

        if is_ndarray1 && is_ndarray2 {
            let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty1);
            let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty2);

            assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

            let left_val =
                NDArrayValue::from_ptr_val(left_val.into_pointer_value(), llvm_usize, None);
            let right_val =
                NDArrayValue::from_ptr_val(right_val.into_pointer_value(), llvm_usize, None);

            let res = if op == Operator::MatMult {
                // MatMult is the only binop which is not an elementwise op
                numpy::ndarray_matmul_2d(
                    generator,
                    ctx,
                    ndarray_dtype1,
                    if is_aug_assign { Some(left_val) } else { None },
                    left_val,
                    right_val,
                )?
            } else {
                numpy::ndarray_elementwise_binop_impl(
                    generator,
                    ctx,
                    ndarray_dtype1,
                    if is_aug_assign { Some(left_val) } else { None },
                    (left_val.as_base_value().into(), false),
                    (right_val.as_base_value().into(), false),
                    |generator, ctx, (lhs, rhs)| {
                        gen_binop_expr_with_values(
                            generator,
                            ctx,
                            (&Some(ndarray_dtype1), lhs),
                            op,
                            (&Some(ndarray_dtype2), rhs),
                            ctx.current_loc,
                            is_aug_assign,
                        )?
                        .unwrap()
                        .to_basic_value_enum(
                            ctx,
                            generator,
                            ndarray_dtype1,
                        )
                    },
                )?
            };

            Ok(Some(res.as_base_value().into()))
        } else {
            let (ndarray_dtype, _) =
                unpack_ndarray_var_tys(&mut ctx.unifier, if is_ndarray1 { ty1 } else { ty2 });
            let ndarray_val = NDArrayValue::from_ptr_val(
                if is_ndarray1 { left_val } else { right_val }.into_pointer_value(),
                llvm_usize,
                None,
            );
            let res = numpy::ndarray_elementwise_binop_impl(
                generator,
                ctx,
                ndarray_dtype,
                if is_aug_assign { Some(ndarray_val) } else { None },
                (left_val, !is_ndarray1),
                (right_val, !is_ndarray2),
                |generator, ctx, (lhs, rhs)| {
                    gen_binop_expr_with_values(
                        generator,
                        ctx,
                        (&Some(ndarray_dtype), lhs),
                        op,
                        (&Some(ndarray_dtype), rhs),
                        ctx.current_loc,
                        is_aug_assign,
                    )?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, ndarray_dtype)
                },
            )?;

            Ok(Some(res.as_base_value().into()))
        }
    } else {
        let left_ty_enum = ctx.unifier.get_ty_immutable(left_ty.unwrap());
        let TypeEnum::TObj { fields, obj_id, .. } = left_ty_enum.as_ref() else {
            unreachable!("must be tobj")
        };
        let (op_name, id) = {
            let (binop_name, binop_assign_name) =
                (binop_name(op).into(), binop_assign_name(op).into());
            // if is aug_assign, try aug_assign operator first
            if is_aug_assign && fields.contains_key(&binop_assign_name) {
                (binop_assign_name, *obj_id)
            } else {
                (binop_name, *obj_id)
            }
        };

        let signature = if let Some(call) = ctx.calls.get(&loc.into()) {
            ctx.unifier.get_call_signature(*call).unwrap()
        } else {
            let left_enum_ty = ctx.unifier.get_ty_immutable(left_ty.unwrap());
            let TypeEnum::TObj { fields, .. } = left_enum_ty.as_ref() else {
                unreachable!("must be tobj")
            };

            let fn_ty = fields.get(&op_name).unwrap().0;
            let fn_ty_enum = ctx.unifier.get_ty_immutable(fn_ty);
            let TypeEnum::TFunc(sig) = fn_ty_enum.as_ref() else { unreachable!() };

            sig.clone()
        };
        let fun_id = {
            let defs = ctx.top_level.definitions.read();
            let obj_def = defs.get(id.0).unwrap().read();
            let TopLevelDef::Class { methods, .. } = &*obj_def else { unreachable!() };

            methods.iter().find(|method| method.0 == op_name).unwrap().2
        };
        generator
            .gen_call(
                ctx,
                Some((left_ty.unwrap(), left_val.into())),
                (&signature, fun_id),
                vec![(None, right_val.into())],
            )
            .map(|f| f.map(Into::into))
    }
}

/// Generates LLVM IR for a binary operator expression.
///
/// * `left` - The left-hand side of the binary operator.
/// * `op` - The operator applied on the operands.
/// * `right` - The right-hand side of the binary operator.
/// * `loc` - The location of the full expression.
/// * `is_aug_assign` - Whether the binary operator expression is also an assignment operator.
pub fn gen_binop_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: &Expr<Option<Type>>,
    op: Operator,
    right: &Expr<Option<Type>>,
    loc: Location,
    is_aug_assign: bool,
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let left_val = if let Some(v) = generator.gen_expr(ctx, left)? {
        v.to_basic_value_enum(ctx, generator, left.custom.unwrap())?
    } else {
        return Ok(None);
    };
    let right_val = if let Some(v) = generator.gen_expr(ctx, right)? {
        v.to_basic_value_enum(ctx, generator, right.custom.unwrap())?
    } else {
        return Ok(None);
    };

    gen_binop_expr_with_values(
        generator,
        ctx,
        (&left.custom, left_val),
        op,
        (&right.custom, right_val),
        loc,
        is_aug_assign,
    )
}

/// Generates LLVM IR for a unary operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_unaryop_expr_with_values<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    op: ast::Unaryop,
    operand: (&Option<Type>, BasicValueEnum<'ctx>),
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let (ty, val) = operand;
    let ty = ctx.unifier.get_representative(ty.unwrap());

    Ok(Some(if ty == ctx.primitives.bool {
        let val = val.into_int_value();
        if op == ast::Unaryop::Not {
            let not = ctx.builder.build_not(val, "not").unwrap();
            let not_bool =
                ctx.builder.build_and(not, not.get_type().const_int(1, false), "").unwrap();

            not_bool.into()
        } else {
            let llvm_i32 = ctx.ctx.i32_type();

            gen_unaryop_expr_with_values(
                generator,
                ctx,
                op,
                (
                    &Some(ctx.primitives.int32),
                    ctx.builder.build_int_z_extend(val, llvm_i32, "").map(Into::into).unwrap(),
                ),
            )?
            .unwrap()
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
                .build_xor(val, val.get_type().const_all_ones(), "not")
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
        let llvm_usize = generator.get_size_type(ctx.ctx);
        let (ndarray_dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);

        let val = NDArrayValue::from_ptr_val(val.into_pointer_value(), llvm_usize, None);

        // ndarray uses `~` rather than `not` to perform elementwise inversion, convert it before
        // passing it to the elementwise codegen function
        let op = if ndarray_dtype.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::Bool.id()) {
            if op == ast::Unaryop::Invert {
                ast::Unaryop::Not
            } else {
                unreachable!("ufunc {} not supported for ndarray[bool, N]", unaryop_name(op))
            }
        } else {
            op
        };

        let res = numpy::ndarray_elementwise_unaryop_impl(
            generator,
            ctx,
            ndarray_dtype,
            None,
            val,
            |generator, ctx, val| {
                gen_unaryop_expr_with_values(generator, ctx, op, (&Some(ndarray_dtype), val))?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, ndarray_dtype)
            },
        )?;

        res.as_base_value().into()
    } else {
        unimplemented!()
    }))
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
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let val = if let Some(v) = generator.gen_expr(ctx, operand)? {
        v.to_basic_value_enum(ctx, generator, operand.custom.unwrap())?
    } else {
        return Ok(None);
    };

    gen_unaryop_expr_with_values(generator, ctx, op, (&operand.custom, val))
}

/// Generates LLVM IR for a comparison operator expression using the [`Type`] and
/// [LLVM value][`BasicValueEnum`] of the operands.
pub fn gen_cmpop_expr_with_values<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    left: (Option<Type>, BasicValueEnum<'ctx>),
    ops: &[ast::Cmpop],
    comparators: &[(Option<Type>, BasicValueEnum<'ctx>)],
) -> Result<Option<ValueEnum<'ctx>>, String> {
    debug_assert_eq!(comparators.len(), ops.len());

    if comparators.len() == 1 {
        let left_ty = ctx.unifier.get_representative(left.0.unwrap());
        let right_ty = ctx.unifier.get_representative(comparators[0].0.unwrap());

        if left_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            || right_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
        {
            let llvm_usize = generator.get_size_type(ctx.ctx);

            let (Some(left_ty), lhs) = left else { unreachable!() };
            let (Some(right_ty), rhs) = comparators[0] else { unreachable!() };
            let op = ops[0];

            let is_ndarray1 =
                left_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());
            let is_ndarray2 =
                right_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id());

            return if is_ndarray1 && is_ndarray2 {
                let (ndarray_dtype1, _) = unpack_ndarray_var_tys(&mut ctx.unifier, left_ty);
                let (ndarray_dtype2, _) = unpack_ndarray_var_tys(&mut ctx.unifier, right_ty);

                assert!(ctx.unifier.unioned(ndarray_dtype1, ndarray_dtype2));

                let left_val =
                    NDArrayValue::from_ptr_val(lhs.into_pointer_value(), llvm_usize, None);
                let res = numpy::ndarray_elementwise_binop_impl(
                    generator,
                    ctx,
                    ctx.primitives.bool,
                    None,
                    (left_val.as_base_value().into(), false),
                    (rhs, false),
                    |generator, ctx, (lhs, rhs)| {
                        let val = gen_cmpop_expr_with_values(
                            generator,
                            ctx,
                            (Some(ndarray_dtype1), lhs),
                            &[op],
                            &[(Some(ndarray_dtype2), rhs)],
                        )?
                        .unwrap()
                        .to_basic_value_enum(
                            ctx,
                            generator,
                            ctx.primitives.bool,
                        )?;

                        Ok(generator.bool_to_i8(ctx, val.into_int_value()).into())
                    },
                )?;

                Ok(Some(res.as_base_value().into()))
            } else {
                let (ndarray_dtype, _) = unpack_ndarray_var_tys(
                    &mut ctx.unifier,
                    if is_ndarray1 { left_ty } else { right_ty },
                );
                let res = numpy::ndarray_elementwise_binop_impl(
                    generator,
                    ctx,
                    ctx.primitives.bool,
                    None,
                    (lhs, !is_ndarray1),
                    (rhs, !is_ndarray2),
                    |generator, ctx, (lhs, rhs)| {
                        let val = gen_cmpop_expr_with_values(
                            generator,
                            ctx,
                            (Some(ndarray_dtype), lhs),
                            &[op],
                            &[(Some(ndarray_dtype), rhs)],
                        )?
                        .unwrap()
                        .to_basic_value_enum(
                            ctx,
                            generator,
                            ctx.primitives.bool,
                        )?;

                        Ok(generator.bool_to_i8(ctx, val.into_int_value()).into())
                    },
                )?;

                Ok(Some(res.as_base_value().into()))
            };
        }
    }

    let cmp_val = izip!(chain(once(&left), comparators.iter()), comparators.iter(), ops.iter(),)
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
                    _ if left_ty == ctx.primitives.bool => unreachable!(),
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
                    _ => unreachable!(),
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
                    _ => unreachable!(),
                };
                ctx.builder.build_float_compare(op, lhs, rhs, "cmp").unwrap()
            } else {
                unimplemented!()
            };

            Ok(prev?.map(|v| ctx.builder.build_and(v, current, "cmp").unwrap()).or(Some(current)))
        })?;

    Ok(Some(match cmp_val {
        Some(v) => v.into(),
        None => return Ok(None),
    }))
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
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let left_val = if let Some(v) = generator.gen_expr(ctx, left)? {
        v.to_basic_value_enum(ctx, generator, left.custom.unwrap())?
    } else {
        return Ok(None);
    };
    let comparator_vals = comparators
        .iter()
        .map(|cmptor| {
            Ok(if let Some(v) = generator.gen_expr(ctx, cmptor)? {
                Some((
                    cmptor.custom,
                    v.to_basic_value_enum(ctx, generator, cmptor.custom.unwrap())?,
                ))
            } else {
                None
            })
        })
        .take_while(|v| if let Ok(v) = v { v.is_some() } else { true })
        .collect::<Result<Vec<_>, String>>()?;
    let comparator_vals = if comparator_vals.len() == comparators.len() {
        comparator_vals.into_iter().map(Option::unwrap).collect_vec()
    } else {
        return Ok(None);
    };

    gen_cmpop_expr_with_values(
        generator,
        ctx,
        (left.custom, left_val),
        ops,
        comparator_vals.as_slice(),
    )
}

/// Generates code for a subscript expression on an `ndarray`.
///
/// * `ty` - The `Type` of the `NDArray` elements.
/// * `ndims` - The `Type` of the `NDArray` number-of-dimensions `Literal`.
/// * `v` - The `NDArray` value.
/// * `slice` - The slice expression used to subscript into the `ndarray`.
fn gen_ndarray_subscript_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    ndims: Type,
    v: NDArrayValue<'ctx>,
    slice: &Expr<Option<Type>>,
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let llvm_i1 = ctx.ctx.bool_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let TypeEnum::TLiteral { values, .. } = &*ctx.unifier.get_ty_immutable(ndims) else {
        unreachable!()
    };

    let ndims = values
        .iter()
        .map(|ndim| match *ndim {
            SymbolValue::U64(v) => Ok(v),
            SymbolValue::U32(v) => Ok(u64::from(v)),
            SymbolValue::I32(v) => u64::try_from(v)
                .map_err(|_| format!("Expected non-negative literal for ndarray.ndims, got {v}")),
            SymbolValue::I64(v) => u64::try_from(v)
                .map_err(|_| format!("Expected non-negative literal for ndarray.ndims, got {v}")),
            _ => unreachable!(),
        })
        .collect::<Result<Vec<_>, _>>()?;

    assert!(!ndims.is_empty());

    let ndarray_ndims_ty = ctx
        .unifier
        .get_fresh_literal(ndims.iter().map(|v| SymbolValue::U64(v - 1)).collect(), None);
    let ndarray_ty =
        make_ndarray_ty(&mut ctx.unifier, &ctx.primitives, Some(ty), Some(ndarray_ndims_ty));
    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, ty).as_basic_type_enum();

    // Check that len is non-zero
    let len = v.load_ndims(ctx);
    ctx.make_assert(
        generator,
        ctx.builder.build_int_compare(IntPredicate::SGT, len, llvm_usize.const_zero(), "").unwrap(),
        "0:IndexError",
        "too many indices for array: array is {0}-dimensional but 1 were indexed",
        [Some(len), None, None],
        slice.location,
    );

    // Normalizes a possibly-negative index to its corresponding positive index
    let normalize_index = |generator: &mut G,
                           ctx: &mut CodeGenContext<'ctx, '_>,
                           index: IntValue<'ctx>,
                           dim: u64| {
        gen_if_else_expr_callback(
            generator,
            ctx,
            |_, ctx| {
                Ok(ctx
                    .builder
                    .build_int_compare(IntPredicate::SGE, index, index.get_type().const_zero(), "")
                    .unwrap())
            },
            |_, _| Ok(Some(index)),
            |generator, ctx| {
                let llvm_i32 = ctx.ctx.i32_type();

                let len = unsafe {
                    v.dim_sizes().get_typed_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(dim, true),
                        None,
                    )
                };

                let index = ctx
                    .builder
                    .build_int_add(
                        len,
                        ctx.builder.build_int_s_extend(index, llvm_usize, "").unwrap(),
                        "",
                    )
                    .unwrap();

                Ok(Some(ctx.builder.build_int_truncate(index, llvm_i32, "").unwrap()))
            },
        )
        .map(|v| v.map(BasicValueEnum::into_int_value))
    };

    // Converts a slice expression into a slice-range tuple
    let expr_to_slice = |generator: &mut G,
                         ctx: &mut CodeGenContext<'ctx, '_>,
                         node: &ExprKind<Option<Type>>,
                         dim: u64| {
        match node {
            ExprKind::Constant { value: Constant::Int(v), .. } => {
                let Some(index) =
                    normalize_index(generator, ctx, llvm_i32.const_int(*v as u64, true), dim)?
                else {
                    return Ok(None);
                };

                Ok(Some((index, index, llvm_i32.const_int(1, true))))
            }

            ExprKind::Slice { lower, upper, step } => {
                let dim_sz = unsafe {
                    v.dim_sizes().get_typed_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(dim, false),
                        None,
                    )
                };

                handle_slice_indices(lower, upper, step, ctx, generator, dim_sz)
            }

            _ => {
                let Some(index) = generator.gen_expr(ctx, slice)? else { return Ok(None) };
                let index = index
                    .to_basic_value_enum(ctx, generator, slice.custom.unwrap())?
                    .into_int_value();
                let Some(index) = normalize_index(generator, ctx, index, dim)? else {
                    return Ok(None);
                };

                Ok(Some((index, index, llvm_i32.const_int(1, true))))
            }
        }
    };

    Ok(Some(match &slice.node {
        ExprKind::Tuple { elts, .. } => {
            let slices = elts
                .iter()
                .enumerate()
                .map(|(dim, elt)| expr_to_slice(generator, ctx, &elt.node, dim as u64))
                .take_while_inclusive(|slice| slice.as_ref().is_ok_and(Option::is_some))
                .collect::<Result<Vec<_>, _>>()?;
            if slices.len() < elts.len() {
                return Ok(None);
            }

            let slices = slices.into_iter().map(Option::unwrap).collect_vec();

            numpy::ndarray_sliced_copy(generator, ctx, ty, v, &slices)?.as_base_value().into()
        }

        ExprKind::Slice { .. } => {
            let Some(slice) = expr_to_slice(generator, ctx, &slice.node, 0)? else {
                return Ok(None);
            };

            numpy::ndarray_sliced_copy(generator, ctx, ty, v, &[slice])?.as_base_value().into()
        }

        _ => {
            let index = if let Some(index) = generator.gen_expr(ctx, slice)? {
                index.to_basic_value_enum(ctx, generator, slice.custom.unwrap())?.into_int_value()
            } else {
                return Ok(None);
            };
            let Some(index) = normalize_index(generator, ctx, index, 0)? else { return Ok(None) };
            let index_addr = generator.gen_var_alloc(ctx, index.get_type().into(), None)?;
            ctx.builder.build_store(index_addr, index).unwrap();

            if ndims.len() == 1 && ndims[0] == 1 {
                // Accessing an element from a 1-dimensional `ndarray`

                return Ok(Some(
                    v.data()
                        .get(
                            ctx,
                            generator,
                            &ArraySliceValue::from_ptr_val(
                                index_addr,
                                llvm_usize.const_int(1, false),
                                None,
                            ),
                            None,
                        )
                        .into(),
                ));
            }

            // Accessing an element from a multi-dimensional `ndarray`

            // Create a new array, remove the top dimension from the dimension-size-list, and copy the
            // elements over
            let subscripted_ndarray = generator.gen_var_alloc(ctx, llvm_ndarray_t.into(), None)?;
            let ndarray = NDArrayValue::from_ptr_val(subscripted_ndarray, llvm_usize, None);

            let num_dims = v.load_ndims(ctx);
            ndarray.store_ndims(
                ctx,
                generator,
                ctx.builder.build_int_sub(num_dims, llvm_usize.const_int(1, false), "").unwrap(),
            );

            let ndarray_num_dims = ndarray.load_ndims(ctx);
            ndarray.create_dim_sizes(ctx, llvm_usize, ndarray_num_dims);

            let ndarray_num_dims = ndarray.load_ndims(ctx);
            let v_dims_src_ptr = unsafe {
                v.dim_sizes().ptr_offset_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(1, false),
                    None,
                )
            };
            call_memcpy_generic(
                ctx,
                ndarray.dim_sizes().base_ptr(ctx, generator),
                v_dims_src_ptr,
                ctx.builder
                    .build_int_mul(ndarray_num_dims, llvm_usize.size_of(), "")
                    .map(Into::into)
                    .unwrap(),
                llvm_i1.const_zero(),
            );

            let ndarray_num_elems = call_ndarray_calc_size(
                generator,
                ctx,
                &ndarray.dim_sizes().as_slice_value(ctx, generator),
                (None, None),
            );
            ndarray.create_data(ctx, llvm_ndarray_data_t, ndarray_num_elems);

            let v_data_src_ptr = v.data().ptr_offset(
                ctx,
                generator,
                &ArraySliceValue::from_ptr_val(index_addr, llvm_usize.const_int(1, false), None),
                None,
            );
            call_memcpy_generic(
                ctx,
                ndarray.data().base_ptr(ctx, generator),
                v_data_src_ptr,
                ctx.builder
                    .build_int_mul(ndarray_num_elems, llvm_ndarray_data_t.size_of().unwrap(), "")
                    .map(Into::into)
                    .unwrap(),
                llvm_i1.const_zero(),
            );

            ndarray.as_base_value().into()
        }
    }))
}

/// See [`CodeGenerator::gen_expr`].
pub fn gen_expr<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    expr: &Expr<Option<Type>>,
) -> Result<Option<ValueEnum<'ctx>>, String> {
    ctx.current_loc = expr.location;
    let int32 = ctx.ctx.i32_type();
    let usize = generator.get_size_type(ctx.ctx);
    let zero = int32.const_int(0, false);

    let loc = ctx.debug_info.0.create_debug_location(
        ctx.ctx,
        ctx.current_loc.row as u32,
        ctx.current_loc.column as u32,
        ctx.debug_info.2,
        None,
    );
    ctx.builder.set_current_debug_location(loc);

    Ok(Some(match &expr.node {
        ExprKind::Constant { value, .. } => {
            let ty = expr.custom.unwrap();
            let Some(const_val) = ctx.gen_const(generator, value, ty) else { return Ok(None) };
            const_val.into()
        }
        ExprKind::Name { id, .. } if id == &"none".into() => {
            match (
                ctx.unifier.get_ty(expr.custom.unwrap()).as_ref(),
                ctx.unifier.get_ty(ctx.primitives.option).as_ref(),
            ) {
                (TypeEnum::TObj { obj_id, params, .. }, TypeEnum::TObj { obj_id: opt_id, .. })
                    if *obj_id == *opt_id =>
                {
                    ctx.get_llvm_type(generator, *params.iter().next().unwrap().1)
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into()
                }
                _ => unreachable!("must be option type"),
            }
        }
        ExprKind::Name { id, .. } => match ctx.var_assignment.get(id) {
            Some((ptr, None, _)) => {
                ctx.builder.build_load(*ptr, id.to_string().as_str()).map(Into::into).unwrap()
            }
            Some((_, Some(static_value), _)) => ValueEnum::Static(static_value.clone()),
            None => {
                let resolver = ctx.resolver.clone();
                resolver.get_symbol_value(*id, ctx).unwrap()
            }
        },
        ExprKind::List { elts, .. } => {
            // this shall be optimized later for constant primitive lists...
            // we should use memcpy for that instead of generating thousands of stores
            let elements = elts
                .iter()
                .map(|x| generator.gen_expr(ctx, x))
                .take_while(|v| !matches!(v, Ok(None)))
                .collect::<Result<Vec<_>, _>>()?;
            let elements = elements
                .into_iter()
                .zip(elts)
                .map(|(v, x)| v.unwrap().to_basic_value_enum(ctx, generator, x.custom.unwrap()))
                .collect::<Result<Vec<_>, _>>()?;

            if elements.len() < elts.len() {
                return Ok(None);
            }

            let ty = if elements.is_empty() {
                let TypeEnum::TList { ty } = &*ctx.unifier.get_ty(expr.custom.unwrap()) else {
                    unreachable!()
                };

                ctx.get_llvm_type(generator, *ty)
            } else {
                elements[0].get_type()
            };
            let length = generator.get_size_type(ctx.ctx).const_int(elements.len() as u64, false);
            let arr_str_ptr = allocate_list(generator, ctx, ty, length, Some("list"));
            let arr_ptr = arr_str_ptr.data();
            for (i, v) in elements.iter().enumerate() {
                let elem_ptr = arr_ptr.ptr_offset(
                    ctx,
                    generator,
                    &usize.const_int(i as u64, false),
                    Some("elem_ptr"),
                );
                ctx.builder.build_store(elem_ptr, *v).unwrap();
            }
            arr_str_ptr.as_base_value().into()
        }
        ExprKind::Tuple { elts, .. } => {
            let elements_val = elts
                .iter()
                .map(|x| generator.gen_expr(ctx, x))
                .take_while(|v| !matches!(v, Ok(None)))
                .collect::<Result<Vec<_>, _>>()?;
            let element_val = elements_val
                .into_iter()
                .zip(elts)
                .map(|(v, x)| v.unwrap().to_basic_value_enum(ctx, generator, x.custom.unwrap()))
                .collect::<Result<Vec<_>, _>>()?;

            if element_val.len() < elts.len() {
                return Ok(None);
            }

            let element_ty = element_val.iter().map(BasicValueEnum::get_type).collect_vec();
            let tuple_ty = ctx.ctx.struct_type(&element_ty, false);
            let tuple_ptr = ctx.builder.build_alloca(tuple_ty, "tuple").unwrap();
            for (i, v) in element_val.into_iter().enumerate() {
                unsafe {
                    let ptr = ctx
                        .builder
                        .build_in_bounds_gep(
                            tuple_ptr,
                            &[zero, int32.const_int(i as u64, false)],
                            "ptr",
                        )
                        .unwrap();
                    ctx.builder.build_store(ptr, v).unwrap();
                }
            }
            ctx.builder.build_load(tuple_ptr, "tup_val").map(Into::into).unwrap()
        }
        ExprKind::Attribute { value, attr, .. } => {
            // note that we would handle class methods directly in calls
            match generator.gen_expr(ctx, value)? {
                Some(ValueEnum::Static(v)) => v.get_field(*attr, ctx).map_or_else(
                    || {
                        let v = v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?;
                        let index = ctx.get_attr_index(value.custom.unwrap(), *attr);
                        Ok(ValueEnum::Dynamic(ctx.build_gep_and_load(
                            v.into_pointer_value(),
                            &[zero, int32.const_int(index as u64, false)],
                            None,
                        ))) as Result<_, String>
                    },
                    Ok,
                )?,
                Some(ValueEnum::Dynamic(v)) => {
                    let index = ctx.get_attr_index(value.custom.unwrap(), *attr);
                    ValueEnum::Dynamic(ctx.build_gep_and_load(
                        v.into_pointer_value(),
                        &[zero, int32.const_int(index as u64, false)],
                        None,
                    ))
                }
                None => return Ok(None),
            }
        }
        ExprKind::BoolOp { op, values } => {
            // requires conditional branches for short-circuiting...
            let left = if let Some(v) = generator.gen_expr(ctx, &values[0])? {
                v.to_basic_value_enum(ctx, generator, values[0].custom.unwrap())?.into_int_value()
            } else {
                return Ok(None);
            };
            let left = generator.bool_to_i1(ctx, left);
            let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
            let a_bb = ctx.ctx.append_basic_block(current, "a");
            let b_bb = ctx.ctx.append_basic_block(current, "b");
            let cont_bb = ctx.ctx.append_basic_block(current, "cont");
            ctx.builder.build_conditional_branch(left, a_bb, b_bb).unwrap();
            let (a, b) = match op {
                Boolop::Or => {
                    ctx.builder.position_at_end(a_bb);
                    let a = ctx.ctx.i8_type().const_int(1, false);
                    ctx.builder.build_unconditional_branch(cont_bb).unwrap();

                    ctx.builder.position_at_end(b_bb);
                    let b = if let Some(v) = generator.gen_expr(ctx, &values[1])? {
                        let b = v
                            .to_basic_value_enum(ctx, generator, values[1].custom.unwrap())?
                            .into_int_value();
                        let b = generator.bool_to_i8(ctx, b);
                        ctx.builder.build_unconditional_branch(cont_bb).unwrap();

                        Some(b)
                    } else {
                        None
                    };

                    (Some(a), b)
                }
                Boolop::And => {
                    ctx.builder.position_at_end(a_bb);
                    let a = if let Some(v) = generator.gen_expr(ctx, &values[1])? {
                        let a = v
                            .to_basic_value_enum(ctx, generator, values[1].custom.unwrap())?
                            .into_int_value();
                        let a = generator.bool_to_i8(ctx, a);
                        ctx.builder.build_unconditional_branch(cont_bb).unwrap();

                        Some(a)
                    } else {
                        None
                    };

                    ctx.builder.position_at_end(b_bb);
                    let b = ctx.ctx.i8_type().const_zero();
                    ctx.builder.build_unconditional_branch(cont_bb).unwrap();

                    (a, Some(b))
                }
            };

            ctx.builder.position_at_end(cont_bb);
            match (a, b) {
                (Some(a), Some(b)) => {
                    let phi = ctx.builder.build_phi(ctx.ctx.i8_type(), "").unwrap();
                    phi.add_incoming(&[(&a, a_bb), (&b, b_bb)]);
                    phi.as_basic_value().into()
                }
                (Some(a), None) => a.into(),
                (None, Some(b)) => b.into(),
                (None, None) => unreachable!(),
            }
        }
        ExprKind::BinOp { op, left, right } => {
            return gen_binop_expr(generator, ctx, left, *op, right, expr.location, false);
        }
        ExprKind::UnaryOp { op, operand } => return gen_unaryop_expr(generator, ctx, *op, operand),
        ExprKind::Compare { left, ops, comparators } => {
            return gen_cmpop_expr(generator, ctx, left, ops, comparators)
        }
        ExprKind::IfExp { test, body, orelse } => {
            let test = match generator.gen_expr(ctx, test)? {
                Some(v) => {
                    v.to_basic_value_enum(ctx, generator, test.custom.unwrap())?.into_int_value()
                }
                None => return Ok(None),
            };
            let test = generator.bool_to_i1(ctx, test);
            let body_ty = body.custom.unwrap();
            let is_none = ctx.unifier.get_representative(body_ty) == ctx.primitives.none;
            let result = if is_none {
                None
            } else {
                let llvm_ty = ctx.get_llvm_type(generator, body_ty);
                Some(ctx.builder.build_alloca(llvm_ty, "if_exp_result").unwrap())
            };
            let current = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
            let then_bb = ctx.ctx.append_basic_block(current, "then");
            let else_bb = ctx.ctx.append_basic_block(current, "else");
            let cont_bb = ctx.ctx.append_basic_block(current, "cont");
            ctx.builder.build_conditional_branch(test, then_bb, else_bb).unwrap();

            ctx.builder.position_at_end(then_bb);
            let a = generator.gen_expr(ctx, body)?;
            if let Some(a) = a {
                match result {
                    None => None,
                    Some(v) => {
                        let a = a.to_basic_value_enum(ctx, generator, body.custom.unwrap())?;
                        Some(ctx.builder.build_store(v, a))
                    }
                };
                ctx.builder.build_unconditional_branch(cont_bb).unwrap();
            }

            ctx.builder.position_at_end(else_bb);
            let b = generator.gen_expr(ctx, orelse)?;
            if let Some(b) = b {
                match result {
                    None => None,
                    Some(v) => {
                        let b = b.to_basic_value_enum(ctx, generator, orelse.custom.unwrap())?;
                        Some(ctx.builder.build_store(v, b))
                    }
                };
                ctx.builder.build_unconditional_branch(cont_bb).unwrap();
            }

            ctx.builder.position_at_end(cont_bb);
            if let Some(v) = result {
                ctx.builder.build_load(v, "if_exp_val_load").map(Into::into).unwrap()
            } else {
                return Ok(None);
            }
        }
        ExprKind::Call { func, args, keywords } => {
            let mut params = args
                .iter()
                .map(|arg| generator.gen_expr(ctx, arg))
                .take_while(|expr| !matches!(expr, Ok(None)))
                .map(|expr| Ok((None, expr?.unwrap())) as Result<_, String>)
                .collect::<Result<Vec<_>, _>>()?;

            if params.len() < args.len() {
                return Ok(None);
            }

            let kw_iter = keywords.iter().map(|kw| {
                Ok((
                    Some(*kw.node.arg.as_ref().unwrap()),
                    generator.gen_expr(ctx, &kw.node.value)?.unwrap(),
                )) as Result<_, String>
            });
            let kw_iter = kw_iter.collect::<Result<Vec<_>, _>>()?;
            params.extend(kw_iter);
            let call = ctx.calls.get(&expr.location.into());
            let signature = if let Some(call) = call {
                ctx.unifier.get_call_signature(*call).unwrap()
            } else {
                let ty = func.custom.unwrap();
                let TypeEnum::TFunc(sign) = &*ctx.unifier.get_ty(ty) else { unreachable!() };

                sign.clone()
            };
            let func = func.as_ref();
            match &func.node {
                ExprKind::Name { id, .. } => {
                    // TODO: handle primitive casts and function pointers
                    let fun = ctx.resolver.get_identifier_def(*id).map_err(|e| {
                        format!("{} (at {})", e.iter().next().unwrap(), func.location)
                    })?;
                    return Ok(generator
                        .gen_call(ctx, None, (&signature, fun), params)?
                        .map(Into::into));
                }
                ExprKind::Attribute { value, attr, .. } => {
                    let Some(val) = generator.gen_expr(ctx, value)? else { return Ok(None) };

                    let id = if let TypeEnum::TObj { obj_id, .. } =
                        &*ctx.unifier.get_ty(value.custom.unwrap())
                    {
                        *obj_id
                    } else {
                        unreachable!()
                    };
                    let fun_id = {
                        let defs = ctx.top_level.definitions.read();
                        let obj_def = defs.get(id.0).unwrap().read();
                        let TopLevelDef::Class { methods, .. } = &*obj_def else { unreachable!() };

                        methods.iter().find(|method| method.0 == *attr).unwrap().2
                    };
                    // directly generate code for option.unwrap
                    // since it needs to return static value to optimize for kernel invariant
                    if attr == &"unwrap".into()
                        && id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap()
                    {
                        match val {
                            ValueEnum::Static(v) => {
                                return match v.get_field("_nac3_option".into(), ctx) {
                                    // if is none, raise exception directly
                                    None => {
                                        let err_msg = ctx.gen_string(generator, "");
                                        let current_fun = ctx
                                            .builder
                                            .get_insert_block()
                                            .unwrap()
                                            .get_parent()
                                            .unwrap();
                                        let unreachable_block = ctx.ctx.append_basic_block(
                                            current_fun,
                                            "unwrap_none_unreachable",
                                        );
                                        let exn_block = ctx.ctx.append_basic_block(
                                            current_fun,
                                            "unwrap_none_exception",
                                        );
                                        ctx.builder.build_unconditional_branch(exn_block).unwrap();
                                        ctx.builder.position_at_end(exn_block);
                                        ctx.raise_exn(
                                            generator,
                                            "0:UnwrapNoneError",
                                            err_msg,
                                            [None, None, None],
                                            ctx.current_loc,
                                        );
                                        ctx.builder.position_at_end(unreachable_block);
                                        let ptr = ctx
                                            .get_llvm_type(generator, value.custom.unwrap())
                                            .into_pointer_type()
                                            .const_null();
                                        Ok(Some(
                                            ctx.builder
                                                .build_load(ptr, "unwrap_none_unreachable_load")
                                                .map(Into::into)
                                                .unwrap(),
                                        ))
                                    }
                                    Some(v) => Ok(Some(v)),
                                };
                            }
                            ValueEnum::Dynamic(BasicValueEnum::PointerValue(ptr)) => {
                                let not_null =
                                    ctx.builder.build_is_not_null(ptr, "unwrap_not_null").unwrap();
                                ctx.make_assert(
                                    generator,
                                    not_null,
                                    "0:UnwrapNoneError",
                                    "",
                                    [None, None, None],
                                    expr.location,
                                );
                                return Ok(Some(
                                    ctx.builder
                                        .build_load(ptr, "unwrap_some_load")
                                        .map(Into::into)
                                        .unwrap(),
                                ));
                            }
                            ValueEnum::Dynamic(_) => unreachable!("option must be static or ptr"),
                        }
                    }

                    // Reset current_loc back to the location of the call
                    ctx.current_loc = expr.location;

                    return Ok(generator
                        .gen_call(
                            ctx,
                            Some((value.custom.unwrap(), val)),
                            (&signature, fun_id),
                            params,
                        )?
                        .map(Into::into));
                }
                _ => unimplemented!(),
            }
        }
        ExprKind::Subscript { value, slice, .. } => {
            match &*ctx.unifier.get_ty(value.custom.unwrap()) {
                TypeEnum::TList { ty } => {
                    let v = if let Some(v) = generator.gen_expr(ctx, value)? {
                        v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?
                            .into_pointer_value()
                    } else {
                        return Ok(None);
                    };
                    let v = ListValue::from_ptr_val(v, usize, Some("arr"));
                    let ty = ctx.get_llvm_type(generator, *ty);
                    if let ExprKind::Slice { lower, upper, step } = &slice.node {
                        let one = int32.const_int(1, false);
                        let Some((start, end, step)) = handle_slice_indices(
                            lower,
                            upper,
                            step,
                            ctx,
                            generator,
                            v.load_size(ctx, None),
                        )?
                        else {
                            return Ok(None);
                        };
                        let length = calculate_len_for_slice_range(
                            generator,
                            ctx,
                            start,
                            ctx.builder
                                .build_select(
                                    ctx.builder
                                        .build_int_compare(IntPredicate::SLT, step, zero, "is_neg")
                                        .unwrap(),
                                    ctx.builder.build_int_sub(end, one, "e_min_one").unwrap(),
                                    ctx.builder.build_int_add(end, one, "e_add_one").unwrap(),
                                    "final_e",
                                )
                                .map(BasicValueEnum::into_int_value)
                                .unwrap(),
                            step,
                        );
                        let res_array_ret = allocate_list(generator, ctx, ty, length, Some("ret"));
                        let Some(res_ind) = handle_slice_indices(
                            &None,
                            &None,
                            &None,
                            ctx,
                            generator,
                            res_array_ret.load_size(ctx, None),
                        )?
                        else {
                            return Ok(None);
                        };
                        list_slice_assignment(
                            generator,
                            ctx,
                            ty,
                            res_array_ret,
                            res_ind,
                            v,
                            (start, end, step),
                        );
                        res_array_ret.as_base_value().into()
                    } else {
                        let len = v.load_size(ctx, Some("len"));
                        let raw_index = if let Some(v) = generator.gen_expr(ctx, slice)? {
                            v.to_basic_value_enum(ctx, generator, slice.custom.unwrap())?
                                .into_int_value()
                        } else {
                            return Ok(None);
                        };
                        let raw_index = ctx
                            .builder
                            .build_int_s_extend(raw_index, generator.get_size_type(ctx.ctx), "sext")
                            .unwrap();
                        // handle negative index
                        let is_negative = ctx
                            .builder
                            .build_int_compare(
                                IntPredicate::SLT,
                                raw_index,
                                generator.get_size_type(ctx.ctx).const_zero(),
                                "is_neg",
                            )
                            .unwrap();
                        let adjusted =
                            ctx.builder.build_int_add(raw_index, len, "adjusted").unwrap();
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
                            generator,
                            bound_check,
                            "0:IndexError",
                            "index {0} out of bounds 0:{1}",
                            [Some(raw_index), Some(len), None],
                            expr.location,
                        );
                        v.data().get(ctx, generator, &index, None).into()
                    }
                }
                TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::NDArray.id() => {
                    let (ty, ndims) = params.iter().map(|(_, ty)| ty).collect_tuple().unwrap();

                    let v = if let Some(v) = generator.gen_expr(ctx, value)? {
                        v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?
                            .into_pointer_value()
                    } else {
                        return Ok(None);
                    };
                    let v = NDArrayValue::from_ptr_val(v, usize, None);

                    return gen_ndarray_subscript_expr(generator, ctx, *ty, *ndims, v, slice);
                }
                TypeEnum::TTuple { .. } => {
                    let index: u32 =
                        if let ExprKind::Constant { value: Constant::Int(v), .. } = &slice.node {
                            (*v).try_into().unwrap()
                        } else {
                            unreachable!("tuple subscript must be const int after type check");
                        };
                    match generator.gen_expr(ctx, value)? {
                        Some(ValueEnum::Dynamic(v)) => {
                            let v = v.into_struct_value();
                            ctx.builder.build_extract_value(v, index, "tup_elem").unwrap().into()
                        }
                        Some(ValueEnum::Static(v)) => {
                            if let Some(v) = v.get_tuple_element(index) {
                                v
                            } else {
                                let tup = v
                                    .to_basic_value_enum(ctx, generator, value.custom.unwrap())?
                                    .into_struct_value();
                                ctx.builder
                                    .build_extract_value(tup, index, "tup_elem")
                                    .unwrap()
                                    .into()
                            }
                        }
                        None => return Ok(None),
                    }
                }
                _ => unreachable!("should not be other subscriptable types after type check"),
            }
        }
        ExprKind::ListComp { .. } => {
            if let Some(v) = gen_comprehension(generator, ctx, expr)? {
                v.into()
            } else {
                return Ok(None);
            }
        }
        _ => unimplemented!(),
    }))
}
