use crate::{
    codegen::{
        classes::{
            ArrayLikeIndexer, ArrayLikeValue, ListType, ListValue, NDArrayValue, ProxyType,
            ProxyValue, RangeValue, UntypedArrayLikeAccessor,
        },
        concrete_type::{ConcreteFuncArg, ConcreteTypeEnum, ConcreteTypeStore},
        gen_in_range_check, get_llvm_abi_type, get_llvm_type, get_va_count_arg_name,
        irrt::*,
        llvm_intrinsics::{
            call_expect, call_float_floor, call_float_pow, call_float_powi, call_int_smax,
            call_int_umin, call_memcpy_generic,
        },
        need_sret, numpy,
        object::ndarray::{NDArrayOut, ScalarOrNDArray},
        stmt::{
            gen_for_callback_incrementing, gen_if_callback, gen_if_else_expr_callback, gen_raise,
            gen_var,
        },
        CodeGenContext, CodeGenTask, CodeGenerator,
    },
    symbol_resolver::{SymbolValue, ValueEnum},
    toplevel::{helper::PrimDef, numpy::unpack_ndarray_var_tys, DefinitionId, TopLevelDef},
    typecheck::{
        magic_methods::{Binop, BinopVariant, HasOpInfo},
        typedef::{FunSignature, FuncArg, Type, TypeEnum, TypeVarId, Unifier, VarMap},
    },
};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    types::{AnyType, BasicType, BasicTypeEnum},
    values::{
        BasicValue, BasicValueEnum, CallSiteValue, FunctionValue, IntValue, PointerValue,
        StructValue,
    },
    AddressSpace, IntPredicate, OptimizationLevel,
};
use itertools::{chain, izip, Either, Itertools};
use nac3parser::ast::{
    self, Boolop, Cmpop, Comprehension, Constant, Expr, ExprKind, Location, Operator, StrRef,
    Unaryop,
};
use std::cmp::min;
use std::iter::{repeat, repeat_with};
use std::{collections::HashMap, convert::TryInto, iter::once, iter::zip};

use super::{
    model::*,
    object::{
        any::AnyObject,
        ndarray::{indexing::util::gen_ndarray_subscript_ndindices, NDArrayObject},
    },
};

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
            _ => unreachable!(),
        };
        let def = &self.top_level.definitions.read()[obj_id.0];
        let (index, value) = if let TopLevelDef::Class { fields, attributes, .. } = &*def.read() {
            if let Some(field_index) = fields.iter().find_position(|x| x.0 == attr) {
                (field_index.0, None)
            } else {
                let attribute_index = attributes.iter().find_position(|x| x.0 == attr).unwrap();
                (attribute_index.0, Some(attribute_index.1 .2.clone()))
            }
        } else {
            unreachable!()
        };
        (index, value)
    }

    pub fn get_attr_index_object(&mut self, ty: Type, attr: StrRef) -> usize {
        match &*self.unifier.get_ty(ty) {
            TypeEnum::TObj { fields, .. } => {
                fields.iter().find_position(|x| *x.0 == attr).unwrap().0
            }
            _ => unreachable!(),
        }
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
        generator: &G,
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
        generator: &G,
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
                let (types, is_vararg_ctx) = if let TypeEnum::TTuple { ty, is_vararg_ctx } = &*ty {
                    (ty.clone(), *is_vararg_ctx)
                } else {
                    unreachable!()
                };
                let values = zip(types, v.iter())
                    .map_while(|(ty, v)| self.gen_const(generator, v, ty))
                    .collect_vec();

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
                    msg.into(),
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
            .map(Some)
            .chain(repeat(None))
            .zip(params.iter())
            .map(|(ty, val)| match (ty, val.get_type()) {
                (Some(BasicTypeEnum::PointerType(arg_ty)), BasicTypeEnum::PointerType(val_ty))
                    if {
                        ty.unwrap() != val.get_type()
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
    pub fn gen_string<G, S>(&mut self, generator: &mut G, s: S) -> StructValue<'ctx>
    where
        G: CodeGenerator + ?Sized,
        S: Into<String>,
    {
        self.gen_const(generator, &Constant::Str(s.into()), self.primitives.str)
            .map(BasicValueEnum::into_struct_value)
            .unwrap()
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
        self.make_assert_impl(generator, cond, err_name, err_msg.into(), params, loc);
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
    let ConcreteTypeEnum::TFunc { args, .. } = &mut signature else { unreachable!() };

    if let Some(obj) = &obj {
        let zelf = store.from_unifier_type(&mut ctx.unifier, &ctx.primitives, obj.0, &mut cache);

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

    if let Some(vararg_arg) = sign.args.iter().find(|arg| arg.is_vararg) {
        let va_count_arg = get_va_count_arg_name(vararg_arg.name);

        args.insert(
            args.len() - 1,
            ConcreteFuncArg {
                name: va_count_arg,
                ty: store.from_unifier_type(
                    &mut ctx.unifier,
                    &ctx.primitives,
                    ctx.primitives.usize(),
                    &mut cache,
                ),
                default_value: None,
                is_vararg: false,
            },
        );
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
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let definition = ctx.top_level.definitions.read().get(fun.1 .0).cloned().unwrap();
    let id;
    let key;
    let param_vals;
    let is_extern;
    let vararg_arg;

    // Ensure that the function object only contains up to 1 vararg parameter
    debug_assert!(fun.0.args.iter().filter(|arg| arg.is_vararg).count() <= 1);

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
                vararg_arg = fun.0.args.iter().find(|arg| arg.is_vararg);
                let old_key = ctx.get_subst_key(obj.as_ref().map(|a| a.0), fun.0, None);
                let mut keys = fun.0.args.clone();
                let mut mapping = HashMap::<_, Vec<ValueEnum>>::new();

                for (key, value) in params {
                    // Find the matching argument
                    let matching_param = fun
                        .0
                        .args
                        .iter()
                        .find_or_last(|p| key.is_some_and(|k| k == p.name))
                        .unwrap();
                    if matching_param.is_vararg {
                        if key.is_none() && !keys.is_empty() {
                            keys.remove(0);
                        }

                        // vararg is lowered into two arguments - va_count and `...`
                        // Handle va_count first, for each argument encountered we increment it by 1
                        let va_count = get_va_count_arg_name(matching_param.name);
                        if let Some(params) = mapping.get_mut(&va_count) {
                            debug_assert_eq!(params.len(), 1);

                            let param = params[0]
                                .clone()
                                .to_basic_value_enum(ctx, generator, ctx.primitives.usize())?
                                .into_int_value();
                            params[0] = param.const_add(llvm_usize.const_int(1, false)).into();
                        } else {
                            mapping.insert(va_count, vec![llvm_usize.const_int(1, false).into()]);
                        }

                        if let Some(param) = mapping.get_mut(&matching_param.name) {
                            param.push(value);
                        } else {
                            mapping.insert(key.unwrap_or(matching_param.name), vec![value]);
                        }
                    } else {
                        mapping.insert(key.unwrap_or_else(|| keys.remove(0).name), vec![value]);
                    }
                }

                // default value handling
                for k in keys {
                    if mapping.contains_key(&k.name) {
                        continue;
                    }

                    if k.is_vararg {
                        mapping.insert(
                            get_va_count_arg_name(k.name),
                            vec![llvm_usize.const_zero().into()],
                        );

                        mapping.insert(k.name, Vec::default());
                    } else {
                        mapping.insert(
                            k.name,
                            vec![ctx
                                .gen_symbol_val(generator, &k.default_value.unwrap(), k.ty)
                                .into()],
                        );
                    }
                }

                // reorder the parameters
                let mut real_params = fun
                    .0
                    .args
                    .iter()
                    .map(|arg| (mapping.remove(&arg.name).unwrap(), arg.ty))
                    .collect_vec();
                if let Some(obj) = &obj {
                    real_params.insert(0, (vec![obj.1.clone()], obj.0));
                }
                if let Some(vararg) = vararg_arg {
                    let vararg_arg_name = get_va_count_arg_name(vararg.name);

                    real_params.insert(
                        real_params.len() - 1,
                        (mapping[&vararg_arg_name].clone(), ctx.primitives.usize()),
                    );
                }

                let static_params = real_params
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (v, _))| {
                        if v.len() != 1 {
                            None
                        } else if let ValueEnum::Static(s) = &v[0] {
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
                    .map(|(ps, t)| {
                        ps.into_iter().map(|p| p.to_basic_value_enum(ctx, generator, t)).collect()
                    })
                    .collect::<Result<Vec<Vec<_>>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
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
            args.insert(
                0,
                FuncArg { name: "self".into(), ty: obj.0, default_value: None, is_vararg: false },
            );
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
            .filter(|(_, arg)| !arg.is_vararg)
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
        let is_vararg = args.iter().any(|arg| arg.is_vararg);
        if is_vararg {
            params.push(generator.get_size_type(ctx.ctx).into());
        }
        let fun_ty = match ret_type {
            Some(ret_type) if !has_sret => ret_type.fn_type(&params, is_vararg),
            _ => ctx.ctx.void_type().fn_type(&params, is_vararg),
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
    let vararg_ty = vararg_arg.map(|vararg| ctx.get_llvm_abi_type(generator, vararg.ty));
    let param_vals = fun_val
        .get_params()
        .iter()
        .map(BasicValueEnum::get_type)
        .chain(repeat_with(|| vararg_ty.unwrap()))
        .zip(param_vals)
        .map(|(p, v)| {
            if p.is_int_type() && v.is_int_value() {
                let expected_ty = p.into_int_type();
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
/// Setting `ty` to [`None`] implies that the list is empty **and** does not have a known element
/// type, and will therefore set the `list.data` type as `size_t*`. It is undefined behavior to
/// generate a sized list with an unknown element type.
pub fn allocate_list<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Option<BasicTypeEnum<'ctx>>,
    length: IntValue<'ctx>,
    name: Option<&'ctx str>,
) -> ListValue<'ctx> {
    let llvm_usize = generator.get_size_type(ctx.ctx);
    let llvm_elem_ty = ty.unwrap_or(llvm_usize.into());

    // List structure; type { ty*, size_t }
    let arr_ty = ListType::new(generator, ctx.ctx, llvm_elem_ty);
    let list = arr_ty.new_value(generator, ctx, name);

    let length = ctx.builder.build_int_z_extend(length, llvm_usize, "").unwrap();
    list.store_size(ctx, generator, length);
    list.create_data(ctx, llvm_elem_ty, None);

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

    let iter_ty = iter.custom.unwrap();
    let iter_val = if let Some(v) = generator.gen_expr(ctx, iter)? {
        v.to_basic_value_enum(ctx, generator, iter_ty)?
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
    let list;

    match &*ctx.unifier.get_ty(iter_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.range.obj_id(&ctx.unifier).unwrap() =>
        {
            let iter_val = RangeValue::from_ptr_val(iter_val.into_pointer_value(), Some("range"));
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
            list = allocate_list(
                generator,
                ctx,
                Some(elem_ty),
                list_alloc_size.into_int_value(),
                Some("listcomp.addr"),
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
            list = allocate_list(generator, ctx, Some(elem_ty), length, Some("listcomp"));

            let counter = generator.gen_var_alloc(ctx, size_t.into(), Some("counter.addr"))?;
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
            generator.gen_assign(ctx, target, val.into(), elt.custom.unwrap())?;
        }
        _ => {
            panic!(
                "unsupported list comprehension iterator type: {}",
                ctx.unifier.stringify(iter_ty)
            );
        }
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
    let elem_ptr =
        unsafe { list.data().ptr_offset_unchecked(ctx, generator, &i, Some("elem_ptr")) };
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
    op: Binop,
    right: (&Option<Type>, BasicValueEnum<'ctx>),
    loc: Location,
) -> Result<Option<ValueEnum<'ctx>>, String> {
    let (left_ty, left_val) = left;
    let (right_ty, right_val) = right;

    let ty1 = ctx.unifier.get_representative(left_ty.unwrap());
    let ty2 = ctx.unifier.get_representative(right_ty.unwrap());

    // we can directly compare the types, because we've got their representatives
    // which would be unchanged until further unification, which we would never do
    // when doing code generation for function instances
    if ty1 == ty2 && [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1) {
        Ok(Some(ctx.gen_int_ops(generator, op.base, left_val, right_val, true).into()))
    } else if ty1 == ty2 && [ctx.primitives.uint32, ctx.primitives.uint64].contains(&ty1) {
        Ok(Some(ctx.gen_int_ops(generator, op.base, left_val, right_val, false).into()))
    } else if [Operator::LShift, Operator::RShift].contains(&op.base) {
        let signed = [ctx.primitives.int32, ctx.primitives.int64].contains(&ty1);
        Ok(Some(ctx.gen_int_ops(generator, op.base, left_val, right_val, signed).into()))
    } else if ty1 == ty2 && ctx.primitives.float == ty1 {
        Ok(Some(ctx.gen_float_ops(op.base, left_val, right_val).into()))
    } else if ty1 == ctx.primitives.float && ty2 == ctx.primitives.int32 {
        // Pow is the only operator that would pass typecheck between float and int
        assert_eq!(op.base, Operator::Pow);
        let res = call_float_powi(
            ctx,
            left_val.into_float_value(),
            right_val.into_int_value(),
            Some("f_pow_i"),
        );
        Ok(Some(res.into()))
    } else if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id())
        || ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id())
    {
        let llvm_usize = generator.get_size_type(ctx.ctx);

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
                        unreachable!()
                    };
                let elem_ty2 =
                    if let TypeEnum::TObj { params, .. } = &*ctx.unifier.get_ty_immutable(ty2) {
                        ctx.unifier.get_representative(*params.iter().next().unwrap().1)
                    } else {
                        unreachable!()
                    };
                debug_assert!(ctx.unifier.unioned(elem_ty1, elem_ty2));

                let llvm_elem_ty = ctx.get_llvm_type(generator, elem_ty1);
                let sizeof_elem = llvm_elem_ty.size_of().unwrap();

                let lhs = ListValue::from_ptr_val(left_val.into_pointer_value(), llvm_usize, None);
                let rhs = ListValue::from_ptr_val(right_val.into_pointer_value(), llvm_usize, None);

                let size = ctx
                    .builder
                    .build_int_add(lhs.load_size(ctx, None), rhs.load_size(ctx, None), "")
                    .unwrap();

                let new_list = allocate_list(generator, ctx, Some(llvm_elem_ty), size, None);

                let lhs_size = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(
                        lhs.load_size(ctx, None),
                        sizeof_elem.get_type(),
                        "",
                    )
                    .unwrap();
                let lhs_len = ctx.builder.build_int_mul(lhs_size, sizeof_elem, "").unwrap();

                let rhs_size = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(
                        rhs.load_size(ctx, None),
                        sizeof_elem.get_type(),
                        "",
                    )
                    .unwrap();
                let rhs_len = ctx.builder.build_int_mul(rhs_size, sizeof_elem, "").unwrap();

                let list_ptr = new_list.data().base_ptr(ctx, generator);
                call_memcpy_generic(
                    ctx,
                    list_ptr,
                    lhs.data().base_ptr(ctx, generator),
                    lhs_len,
                    ctx.ctx.bool_type().const_zero(),
                );

                let list_ptr = unsafe {
                    new_list.data().ptr_offset_unchecked(
                        ctx,
                        generator,
                        &lhs.load_size(ctx, None),
                        None,
                    )
                };
                call_memcpy_generic(
                    ctx,
                    list_ptr,
                    rhs.data().base_ptr(ctx, generator),
                    rhs_len,
                    ctx.ctx.bool_type().const_zero(),
                );

                Ok(Some(new_list.as_base_value().into()))
            }

            Operator::Mult => {
                let (elem_ty, list_val, int_val) =
                    if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()) {
                        let elem_ty = if let TypeEnum::TObj { params, .. } =
                            &*ctx.unifier.get_ty_immutable(ty1)
                        {
                            *params.iter().next().unwrap().1
                        } else {
                            unreachable!()
                        };

                        (elem_ty, left_val, right_val)
                    } else if ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()) {
                        let elem_ty = if let TypeEnum::TObj { params, .. } =
                            &*ctx.unifier.get_ty_immutable(ty2)
                        {
                            *params.iter().next().unwrap().1
                        } else {
                            unreachable!()
                        };

                        (elem_ty, right_val, left_val)
                    } else {
                        unreachable!()
                    };
                let list_val =
                    ListValue::from_ptr_val(list_val.into_pointer_value(), llvm_usize, None);
                let int_val = ctx
                    .builder
                    .build_int_s_extend(int_val.into_int_value(), llvm_usize, "")
                    .unwrap();
                // [...] * (i where i < 0) => []
                let int_val = call_int_smax(ctx, int_val, llvm_usize.const_zero(), None);

                let elem_llvm_ty = ctx.get_llvm_type(generator, elem_ty);
                let sizeof_elem = elem_llvm_ty.size_of().unwrap();

                let new_list = allocate_list(
                    generator,
                    ctx,
                    Some(elem_llvm_ty),
                    ctx.builder.build_int_mul(list_val.load_size(ctx, None), int_val, "").unwrap(),
                    None,
                );

                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (int_val, false),
                    |generator, ctx, _, i| {
                        let offset = ctx
                            .builder
                            .build_int_mul(i, list_val.load_size(ctx, None), "")
                            .unwrap();
                        let ptr = unsafe {
                            new_list.data().ptr_offset_unchecked(ctx, generator, &offset, None)
                        };

                        let list_size = ctx
                            .builder
                            .build_int_z_extend_or_bit_cast(
                                list_val.load_size(ctx, None),
                                sizeof_elem.get_type(),
                                "",
                            )
                            .unwrap();

                        let memcpy_sz =
                            ctx.builder.build_int_mul(list_size, sizeof_elem, "").unwrap();

                        call_memcpy_generic(
                            ctx,
                            ptr,
                            list_val.data().base_ptr(ctx, generator),
                            memcpy_sz,
                            ctx.ctx.bool_type().const_zero(),
                        );

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )?;

                Ok(Some(new_list.as_base_value().into()))
            }

            _ => todo!("Operator not supported"),
        }
    } else if ty1.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
        || ty2.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
    {
        let left =
            ScalarOrNDArray::split_object(generator, ctx, AnyObject { ty: ty1, value: left_val });
        let right =
            ScalarOrNDArray::split_object(generator, ctx, AnyObject { ty: ty2, value: right_val });

        // Inhomogeneous binary operations are not supported.
        assert!(ctx.unifier.unioned(left.get_dtype(), right.get_dtype()));

        let common_dtype = left.get_dtype();

        let out = match op.variant {
            BinopVariant::Normal => NDArrayOut::NewNDArray { dtype: common_dtype },
            BinopVariant::AugAssign => {
                // If this is an augmented assignment.
                // `left` has to be an ndarray. If it were a scalar then NAC3 simply doesn't support it.
                if let ScalarOrNDArray::NDArray(out_ndarray) = left {
                    NDArrayOut::WriteToNDArray { ndarray: out_ndarray }
                } else {
                    panic!("left must be an ndarray")
                }
            }
        };

        if op.base == Operator::MatMult {
            // Handle matrix multiplication.
            todo!()
        } else {
            // For other operations, they are all elementwise operations.

            // There are only three cases:
            // - LHS is a scalar, RHS is an ndarray.
            // - LHS is an ndarray, RHS is a scalar.
            // - LHS is an ndarray, RHS is an ndarray.
            //
            // For all cases, the scalar operand is promoted to an ndarray,
            // the two are then broadcasted, and starmapped through.

            let left = left.to_ndarray(generator, ctx);
            let right = right.to_ndarray(generator, ctx);

            let result = NDArrayObject::broadcast_starmap(
                generator,
                ctx,
                &[left, right],
                out,
                |generator, ctx, scalars| {
                    let left_value = scalars[0];
                    let right_value = scalars[1];

                    let result = gen_binop_expr_with_values(
                        generator,
                        ctx,
                        (&Some(left.dtype), left_value),
                        op,
                        (&Some(right.dtype), right_value),
                        ctx.current_loc,
                    )?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, common_dtype)?;

                    Ok(result)
                },
            )
            .unwrap();
            Ok(Some(ValueEnum::Dynamic(result.instance.value.as_basic_value_enum())))
        }
    } else {
        let left_ty_enum = ctx.unifier.get_ty_immutable(left_ty.unwrap());
        let TypeEnum::TObj { fields, obj_id, .. } = left_ty_enum.as_ref() else {
            unreachable!("must be tobj")
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
    op: Binop,
    right: &Expr<Option<Type>>,
    loc: Location,
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
        let ndarray = AnyObject { value: val, ty };
        let ndarray = NDArrayObject::from_object(generator, ctx, ndarray);

        // ndarray uses `~` rather than `not` to perform elementwise inversion, convert it before
        // passing it to the elementwise codegen function
        let op = if ndarray.dtype.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::Bool.id()) {
            if op == ast::Unaryop::Invert {
                ast::Unaryop::Not
            } else {
                unreachable!(
                    "ufunc {} not supported for ndarray[bool, N]",
                    op.op_info().method_name,
                )
            }
        } else {
            op
        };

        let mapped_ndarray = ndarray.map(
            generator,
            ctx,
            NDArrayOut::NewNDArray { dtype: ndarray.dtype },
            |generator, ctx, scalar| {
                gen_unaryop_expr_with_values(generator, ctx, op, (&Some(ndarray.dtype), scalar))?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, ndarray.dtype)
            },
        )?;

        ValueEnum::Dynamic(mapped_ndarray.instance.value.as_basic_value_enum())
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
            } else if left_ty == ctx.primitives.str {
                assert!(ctx.unifier.unioned(left_ty, right_ty));

                let llvm_i1 = ctx.ctx.bool_type();
                let llvm_i32 = ctx.ctx.i32_type();
                let llvm_usize = generator.get_size_type(ctx.ctx);

                let lhs = lhs.into_struct_value();
                let rhs = rhs.into_struct_value();

                let plhs = generator.gen_var_alloc(ctx, lhs.get_type().into(), None).unwrap();
                ctx.builder.build_store(plhs, lhs).unwrap();
                let prhs = generator.gen_var_alloc(ctx, lhs.get_type().into(), None).unwrap();
                ctx.builder.build_store(prhs, rhs).unwrap();

                let lhs_len = ctx.build_in_bounds_gep_and_load(
                    plhs,
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    None,
                ).into_int_value();
                let rhs_len = ctx.build_in_bounds_gep_and_load(
                    prhs,
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    None,
                ).into_int_value();

                let len = call_int_umin(ctx, lhs_len, rhs_len, None);

                let current_bb = ctx.builder.get_insert_block().unwrap();
                let post_foreach_cmp = ctx.ctx.insert_basic_block_after(current_bb, "foreach.cmp.end");

                ctx.builder.position_at_end(post_foreach_cmp);
                let cmp_phi = ctx.builder.build_phi(llvm_i1, "").unwrap();
                ctx.builder.position_at_end(current_bb);

                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (len, false),
                    |generator, ctx, _, i| {
                        let lhs_char = {
                            let plhs_data = ctx.build_in_bounds_gep_and_load(
                                plhs,
                                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                                None,
                            ).into_pointer_value();

                            ctx.build_in_bounds_gep_and_load(
                                plhs_data,
                                &[i],
                                None
                            ).into_int_value()
                        };
                        let rhs_char = {
                            let prhs_data = ctx.build_in_bounds_gep_and_load(
                                prhs,
                                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                                None,
                            ).into_pointer_value();

                            ctx.build_in_bounds_gep_and_load(
                                prhs_data,
                                &[i],
                                None
                            ).into_int_value()
                        };

                        gen_if_callback(
                            generator,
                            ctx,
                            |_, ctx| {
                                Ok(ctx.builder.build_int_compare(IntPredicate::NE, lhs_char, rhs_char, "").unwrap())
                            },
                            |_, ctx| {
                                let bb = ctx.builder.get_insert_block().unwrap();
                                cmp_phi.add_incoming(&[(&llvm_i1.const_zero(), bb)]);
                                ctx.builder.build_unconditional_branch(post_foreach_cmp).unwrap();

                                Ok(())
                            },
                            |_, _| Ok(()),
                        )?;

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )?;

                let bb = ctx.builder.get_insert_block().unwrap();
                let is_len_eq = ctx.builder.build_int_compare(
                    IntPredicate::EQ,
                    lhs_len,
                    rhs_len,
                    "",
                ).unwrap();
                cmp_phi.add_incoming(&[(&is_len_eq, bb)]);
                ctx.builder.build_unconditional_branch(post_foreach_cmp).unwrap();

                ctx.builder.position_at_end(post_foreach_cmp);
                let cmp_phi = cmp_phi.as_basic_value().into_int_value();

                // Invert the final value if __ne__
                if *op == Cmpop::NotEq {
                    ctx.builder.build_not(cmp_phi, "").unwrap()
                } else {
                    cmp_phi
                }
            } else if [left_ty, right_ty]
                .iter()
                .any(|ty| ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id()))
            {
                let llvm_usize = generator.get_size_type(ctx.ctx);

                let gen_list_cmpop = |generator: &mut G,
                                      ctx: &mut CodeGenContext<'ctx, '_>|
                 -> Result<IntValue<'ctx>, String> {
                    let is_list1 =
                        left_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id());
                    let is_list2 =
                        right_ty.obj_id(&ctx.unifier).is_some_and(|id| id == PrimDef::List.id());

                    let gen_bool_const = |ctx: &CodeGenContext<'ctx, '_>, val: bool| {
                        let llvm_i1 = ctx.ctx.bool_type();

                        match (op, val) {
                            (Cmpop::Eq, true) | (Cmpop::NotEq, false) => llvm_i1.const_all_ones(),
                            (Cmpop::Eq, false) | (Cmpop::NotEq, true) => llvm_i1.const_zero(),
                            (_, _) => unreachable!(),
                        }
                    };

                    if !(is_list1 && is_list2) {
                        return Ok(generator.bool_to_i8(ctx, gen_bool_const(ctx, false)));
                    }

                    let left_elem_ty = if let TypeEnum::TObj { params, .. } =
                        &*ctx.unifier.get_ty_immutable(left_ty)
                    {
                        *params.iter().next().unwrap().1
                    } else {
                        unreachable!()
                    };
                    let right_elem_ty = if let TypeEnum::TObj { params, .. } =
                        &*ctx.unifier.get_ty_immutable(right_ty)
                    {
                        *params.iter().next().unwrap().1
                    } else {
                        unreachable!()
                    };

                    if !ctx.unifier.unioned(left_elem_ty, right_elem_ty) {
                        return Ok(generator.bool_to_i8(ctx, gen_bool_const(ctx, false)));
                    }

                    if ![Cmpop::Eq, Cmpop::NotEq].contains(op) {
                        todo!("Only __eq__ and __ne__ is implemented for lists")
                    }

                    let left_val =
                        ListValue::from_ptr_val(lhs.into_pointer_value(), llvm_usize, None);
                    let right_val =
                        ListValue::from_ptr_val(rhs.into_pointer_value(), llvm_usize, None);

                    Ok(gen_if_else_expr_callback(
                        generator,
                        ctx,
                        |_, ctx| {
                            Ok(ctx
                                .builder
                                .build_int_compare(
                                    IntPredicate::EQ,
                                    left_val.load_size(ctx, None),
                                    right_val.load_size(ctx, None),
                                    "",
                                )
                                .unwrap())
                        },
                        |generator, ctx| {
                            let acc_addr = generator
                                .gen_var_alloc(ctx, ctx.ctx.bool_type().into(), None)
                                .unwrap();
                            ctx.builder
                                .build_store(acc_addr, ctx.ctx.bool_type().const_all_ones())
                                .unwrap();

                            gen_for_callback_incrementing(
                                generator,
                                ctx,
                                None,
                                llvm_usize.const_zero(),
                                (left_val.load_size(ctx, None), false),
                                |generator, ctx, hooks, i| {
                                    let left = unsafe {
                                        left_val.data().get_unchecked(ctx, generator, &i, None)
                                    };
                                    let right = unsafe {
                                        right_val.data().get_unchecked(ctx, generator, &i, None)
                                    };

                                    let res = gen_cmpop_expr_with_values(
                                        generator,
                                        ctx,
                                        (Some(left_elem_ty), left),
                                        &[Cmpop::Eq],
                                        &[(Some(right_elem_ty), right)],
                                    )?
                                    .unwrap()
                                    .to_basic_value_enum(ctx, generator, ctx.primitives.bool)
                                    .unwrap()
                                    .into_int_value();

                                    gen_if_callback(
                                        generator,
                                        ctx,
                                        |_, ctx| {
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
                                        |_, ctx| {
                                            ctx.builder
                                                .build_store(
                                                    acc_addr,
                                                    ctx.ctx.bool_type().const_zero(),
                                                )
                                                .unwrap();
                                            ctx.builder
                                                .build_unconditional_branch(hooks.exit_bb)
                                                .unwrap();

                                            Ok(())
                                        },
                                        |_, _| Ok(()),
                                    )
                                    .unwrap();

                                    Ok(())
                                },
                                llvm_usize.const_int(1, false),
                            )?;

                            let acc = ctx
                                .builder
                                .build_load(acc_addr, "")
                                .map(BasicValueEnum::into_int_value)
                                .unwrap();
                            let acc = if *op == Cmpop::NotEq {
                                gen_unaryop_expr_with_values(
                                    generator,
                                    ctx,
                                    Unaryop::Not,
                                    (&Some(ctx.primitives.bool), acc.into()),
                                )?
                                .unwrap()
                                .to_basic_value_enum(ctx, generator, ctx.primitives.bool)?
                                .into_int_value()
                            } else {
                                acc
                            };

                            Ok(Some(generator.bool_to_i8(ctx, acc)))
                        },
                        |generator, ctx| {
                            Ok(Some(generator.bool_to_i8(ctx, gen_bool_const(ctx, false))))
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

                let llvm_i1 = ctx.ctx.bool_type();
                let llvm_i32 = ctx.ctx.i32_type();

                // Assume `true` by default
                let cmp_addr = generator.gen_var_alloc(ctx, llvm_i1.into(), None).unwrap();
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
                        let plhs = generator.gen_var_alloc(ctx, lhs.get_type(), None).unwrap();
                        ctx.builder.build_store(plhs, *lhs).unwrap();

                        ctx.build_in_bounds_gep_and_load(
                            plhs,
                            &[llvm_i32.const_zero(), llvm_i32.const_int(i as u64, false)],
                            None,
                        )
                    };
                    let right_ty = right_tys[i];
                    let right_elem = {
                        let prhs = generator.gen_var_alloc(ctx, rhs.get_type(), None).unwrap();
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
                                .transpose()
                                .unwrap()
                                .and_then(|v| {
                                    v.to_basic_value_enum(ctx, generator, ctx.primitives.bool)
                                })
                                .map(BasicValueEnum::into_int_value)?;

                            Ok(ctx.builder.build_not(cmp, "").unwrap())
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
                    ctx.builder.build_not(cmp_phi, "").unwrap()
                } else {
                    cmp_phi
                }
            } else if [left_ty, right_ty].iter().any(|ty| matches!(&*ctx.unifier.get_ty_immutable(*ty), TypeEnum::TVar { .. })) {
                if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                    ctx.make_assert(
                        generator,
                        ctx.ctx.bool_type().const_all_ones(),
                        "0:AssertionError",
                        "nac3core::codegen::expr::gen_cmpop_expr_with_values: Unexpected comparison between two typevar values",
                        [None, None, None],
                        ctx.current_loc,
                    );
                }

                ctx.ctx.bool_type().get_poison()
            } else {
                return Err(format!("'{}' not supported between instances of '{}' and '{}'",
                                   op.op_info().symbol,
                                   ctx.unifier.stringify(left_ty),
                                   ctx.unifier.stringify(right_ty)))
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
                if let Some(res) = resolver.get_symbol_value(*id, ctx) {
                    res
                } else {
                    // Allow "raise Exception" short form
                    let def_id = resolver.get_identifier_def(*id).map_err(|e| {
                        format!("{} (at {})", e.iter().next().unwrap(), expr.location)
                    })?;
                    let def = ctx.top_level.definitions.read();
                    if let TopLevelDef::Class { constructor, .. } = *def[def_id.0].read() {
                        let TypeEnum::TFunc(signature) =
                            ctx.unifier.get_ty(constructor.unwrap()).as_ref().clone()
                        else {
                            return Err(format!(
                                "Failed to resolve symbol {} (at {})",
                                id, expr.location
                            ));
                        };
                        return Ok(generator
                            .gen_call(ctx, None, (&signature, def_id), Vec::default())?
                            .map(Into::into));
                    }
                    return Err(format!("Failed to resolve symbol {} (at {})", id, expr.location));
                }
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
                let ty = if let TypeEnum::TObj { obj_id, params, .. } =
                    &*ctx.unifier.get_ty(expr.custom.unwrap())
                {
                    assert_eq!(*obj_id, PrimDef::List.id());

                    *params.iter().next().unwrap().1
                } else {
                    unreachable!()
                };

                if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(ty) {
                    None
                } else {
                    Some(ctx.get_llvm_type(generator, ty))
                }
            } else {
                Some(elements[0].get_type())
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

            // Change Class attribute access requests to accessing constants from Class Definition
            if let Some(c) = value.custom {
                if let TypeEnum::TFunc(_) = &*ctx.unifier.get_ty(c) {
                    let defs = ctx.top_level.definitions.read();
                    let result = defs.iter().find_map(|def| {
                        if let Some(rear_guard) = def.try_read() {
                            if let TopLevelDef::Class {
                                constructor: Some(constructor),
                                attributes,
                                ..
                            } = &*rear_guard
                            {
                                if *constructor == c {
                                    return attributes.iter().find_map(|f| {
                                        if f.0 == *attr {
                                            // All other checks performed by this point
                                            return Some(f.2.clone());
                                        }
                                        None
                                    });
                                }
                            }
                        }
                        None
                    });
                    match result {
                        Some(val) => {
                            let mut modified_expr = expr.clone();
                            modified_expr.node = ExprKind::Constant { value: val, kind: None };

                            return generator.gen_expr(ctx, &modified_expr);
                        }
                        None => unreachable!("Function Type should not have attributes"),
                    }
                } else if let TypeEnum::TObj { obj_id, fields, params } = &*ctx.unifier.get_ty(c) {
                    if fields.is_empty() && params.is_empty() {
                        let defs = ctx.top_level.definitions.read();
                        let def = defs[obj_id.0].read();
                        match if let TopLevelDef::Class { attributes, .. } = &*def {
                            attributes.iter().find_map(|f| {
                                if f.0 == *attr {
                                    return Some(f.2.clone());
                                }
                                None
                            })
                        } else {
                            None
                        } {
                            Some(val) => {
                                let mut modified_expr = expr.clone();
                                modified_expr.node = ExprKind::Constant { value: val, kind: None };

                                return generator.gen_expr(ctx, &modified_expr);
                            }
                            None => unreachable!(),
                        }
                    }
                }
            }

            match generator.gen_expr(ctx, value)? {
                Some(ValueEnum::Static(v)) => v.get_field(*attr, ctx).map_or_else(
                    || {
                        let v = v.to_basic_value_enum(ctx, generator, value.custom.unwrap())?;
                        let (index, _) = ctx.get_attr_index(value.custom.unwrap(), *attr);
                        Ok(ValueEnum::Dynamic(ctx.build_gep_and_load(
                            v.into_pointer_value(),
                            &[zero, int32.const_int(index as u64, false)],
                            None,
                        ))) as Result<_, String>
                    },
                    Ok,
                )?,
                Some(ValueEnum::Dynamic(v)) => {
                    let (index, attr_value) = ctx.get_attr_index(value.custom.unwrap(), *attr);
                    if let Some(val) = attr_value {
                        // Change to Constant Construct
                        let mut modified_expr = expr.clone();
                        modified_expr.node = ExprKind::Constant { value: val, kind: None };

                        return generator.gen_expr(ctx, &modified_expr);
                    }
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
            return gen_binop_expr(generator, ctx, left, Binop::normal(*op), right, expr.location);
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

                    // Handle Class Method calls
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
                                            err_msg.into(),
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
                TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                    let ty = params.iter().next().unwrap().1;

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
                        let res_array_ret =
                            allocate_list(generator, ctx, Some(ty), length, Some("ret"));
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
                TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                    let Some(ndarray) = generator.gen_expr(ctx, value)? else {
                        return Ok(None);
                    };

                    let ndarray_ty = value.custom.unwrap();
                    let ndarray = ndarray.to_basic_value_enum(ctx, generator, ndarray_ty)?;

                    let ndarray = NDArrayObject::from_object(
                        generator,
                        ctx,
                        AnyObject { ty: ndarray_ty, value: ndarray },
                    );

                    let indices = gen_ndarray_subscript_ndindices(generator, ctx, slice)?;
                    let result = ndarray
                        .index(generator, ctx, &indices)
                        .split_unsized(generator, ctx)
                        .to_basic_value_enum();
                    return Ok(Some(ValueEnum::Dynamic(result)));
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

/// Generate LLVM IR for an [`ExprKind::Slice`]
#[allow(clippy::type_complexity)]
pub fn gen_slice<'ctx, G: CodeGenerator>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    lower: &Option<Box<Expr<Option<Type>>>>,
    upper: &Option<Box<Expr<Option<Type>>>>,
    step: &Option<Box<Expr<Option<Type>>>>,
) -> Result<
    (
        Option<Instance<'ctx, Int<Int32>>>,
        Option<Instance<'ctx, Int<Int32>>>,
        Option<Instance<'ctx, Int<Int32>>>,
    ),
    String,
> {
    let mut help = |value_expr: &Option<Box<Expr<Option<Type>>>>| -> Result<_, String> {
        Ok(match value_expr {
            None => None,
            Some(value_expr) => {
                let value_expr = generator
                    .gen_expr(ctx, value_expr)?
                    .unwrap()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.int32)?;

                let value_expr = Int(Int32).check_value(generator, ctx.ctx, value_expr).unwrap();

                Some(value_expr)
            }
        })
    };

    let lower = help(lower)?;
    let upper = help(upper)?;
    let step = help(step)?;

    Ok((lower, upper, step))
}
