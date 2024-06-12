use crate::symbol_resolver::SymbolValue;
use crate::toplevel::helper::PrimDef;
use crate::toplevel::numpy::{make_ndarray_ty, unpack_ndarray_var_tys};
use crate::typecheck::{
    type_inferencer::*,
    typedef::{FunSignature, FuncArg, Type, TypeEnum, Unifier, VarMap},
};
use itertools::Itertools;
use nac3parser::ast::StrRef;
use nac3parser::ast::{Cmpop, Operator, Unaryop};
use std::cmp::max;
use std::collections::HashMap;
use std::rc::Rc;
use strum::IntoEnumIterator;

#[must_use]
pub fn binop_name(op: &Operator) -> &'static str {
    match op {
        Operator::Add => "__add__",
        Operator::Sub => "__sub__",
        Operator::Div => "__truediv__",
        Operator::Mod => "__mod__",
        Operator::Mult => "__mul__",
        Operator::Pow => "__pow__",
        Operator::BitOr => "__or__",
        Operator::BitXor => "__xor__",
        Operator::BitAnd => "__and__",
        Operator::LShift => "__lshift__",
        Operator::RShift => "__rshift__",
        Operator::FloorDiv => "__floordiv__",
        Operator::MatMult => "__matmul__",
    }
}

#[must_use]
pub fn binop_assign_name(op: &Operator) -> &'static str {
    match op {
        Operator::Add => "__iadd__",
        Operator::Sub => "__isub__",
        Operator::Div => "__itruediv__",
        Operator::Mod => "__imod__",
        Operator::Mult => "__imul__",
        Operator::Pow => "__ipow__",
        Operator::BitOr => "__ior__",
        Operator::BitXor => "__ixor__",
        Operator::BitAnd => "__iand__",
        Operator::LShift => "__ilshift__",
        Operator::RShift => "__irshift__",
        Operator::FloorDiv => "__ifloordiv__",
        Operator::MatMult => "__imatmul__",
    }
}

#[must_use]
pub fn unaryop_name(op: &Unaryop) -> &'static str {
    match op {
        Unaryop::UAdd => "__pos__",
        Unaryop::USub => "__neg__",
        Unaryop::Not => "__not__",
        Unaryop::Invert => "__inv__",
    }
}

#[must_use]
pub fn comparison_name(op: &Cmpop) -> Option<&'static str> {
    match op {
        Cmpop::Lt => Some("__lt__"),
        Cmpop::LtE => Some("__le__"),
        Cmpop::Gt => Some("__gt__"),
        Cmpop::GtE => Some("__ge__"),
        Cmpop::Eq => Some("__eq__"),
        Cmpop::NotEq => Some("__ne__"),
        _ => None,
    }
}

pub(super) fn with_fields<F>(unifier: &mut Unifier, ty: Type, f: F)
where
    F: FnOnce(&mut Unifier, &mut HashMap<StrRef, (Type, bool)>),
{
    let (id, mut fields, params) =
        if let TypeEnum::TObj { obj_id, fields, params } = &*unifier.get_ty(ty) {
            (*obj_id, fields.clone(), params.clone())
        } else {
            unreachable!()
        };
    f(unifier, &mut fields);
    unsafe {
        let unification_table = unifier.get_unification_table();
        unification_table.set_value(ty, Rc::new(TypeEnum::TObj { obj_id: id, fields, params }));
    }
}

pub fn impl_binop(
    unifier: &mut Unifier,
    _store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
    ops: &[Operator],
) {
    with_fields(unifier, ty, |unifier, fields| {
        let (other_ty, other_var_id) = if other_ty.len() == 1 {
            (other_ty[0], None)
        } else {
            let (ty, var_id) = unifier.get_fresh_var_with_range(other_ty, Some("N".into()), None);
            (ty, Some(var_id))
        };

        let function_vars = if let Some(var_id) = other_var_id {
            vec![(var_id, other_ty)].into_iter().collect::<VarMap>()
        } else {
            VarMap::new()
        };

        let ret_ty = ret_ty.unwrap_or_else(|| unifier.get_fresh_var(None, None).0);

        for op in ops {
            fields.insert(binop_name(op).into(), {
                (
                    unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        ret: ret_ty,
                        vars: function_vars.clone(),
                        args: vec![FuncArg {
                            ty: other_ty,
                            default_value: None,
                            name: "other".into(),
                        }],
                    })),
                    false,
                )
            });

            fields.insert(binop_assign_name(op).into(), {
                (
                    unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        ret: ret_ty,
                        vars: function_vars.clone(),
                        args: vec![FuncArg {
                            ty: other_ty,
                            default_value: None,
                            name: "other".into(),
                        }],
                    })),
                    false,
                )
            });
        }
    });
}

pub fn impl_unaryop(unifier: &mut Unifier, ty: Type, ret_ty: Option<Type>, ops: &[Unaryop]) {
    with_fields(unifier, ty, |unifier, fields| {
        let ret_ty = ret_ty.unwrap_or_else(|| unifier.get_fresh_var(None, None).0);

        for op in ops {
            fields.insert(
                unaryop_name(op).into(),
                (
                    unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        ret: ret_ty,
                        vars: VarMap::new(),
                        args: vec![],
                    })),
                    false,
                ),
            );
        }
    });
}

pub fn impl_cmpop(
    unifier: &mut Unifier,
    _store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ops: &[Cmpop],
    ret_ty: Option<Type>,
) {
    with_fields(unifier, ty, |unifier, fields| {
        let (other_ty, other_var_id) = if other_ty.len() == 1 {
            (other_ty[0], None)
        } else {
            let (ty, var_id) = unifier.get_fresh_var_with_range(other_ty, Some("N".into()), None);
            (ty, Some(var_id))
        };

        let function_vars = if let Some(var_id) = other_var_id {
            vec![(var_id, other_ty)].into_iter().collect::<VarMap>()
        } else {
            VarMap::new()
        };

        let ret_ty = ret_ty.unwrap_or_else(|| unifier.get_fresh_var(None, None).0);

        for op in ops {
            fields.insert(
                comparison_name(op).unwrap().into(),
                (
                    unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        ret: ret_ty,
                        vars: function_vars.clone(),
                        args: vec![FuncArg {
                            ty: other_ty,
                            default_value: None,
                            name: "other".into(),
                        }],
                    })),
                    false,
                ),
            );
        }
    });
}

/// `Add`, `Sub`, `Mult`
pub fn impl_basic_arithmetic(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(
        unifier,
        store,
        ty,
        other_ty,
        ret_ty,
        &[Operator::Add, Operator::Sub, Operator::Mult],
    );
}

/// `Pow`
pub fn impl_pow(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(unifier, store, ty, other_ty, ret_ty, &[Operator::Pow]);
}

/// `BitOr`, `BitXor`, `BitAnd`
pub fn impl_bitwise_arithmetic(unifier: &mut Unifier, store: &PrimitiveStore, ty: Type) {
    impl_binop(
        unifier,
        store,
        ty,
        &[ty],
        Some(ty),
        &[Operator::BitAnd, Operator::BitOr, Operator::BitXor],
    );
}

/// `LShift`, `RShift`
pub fn impl_bitwise_shift(unifier: &mut Unifier, store: &PrimitiveStore, ty: Type) {
    impl_binop(
        unifier,
        store,
        ty,
        &[store.int32, store.uint32],
        Some(ty),
        &[Operator::LShift, Operator::RShift],
    );
}

/// `Div`
pub fn impl_div(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(unifier, store, ty, other_ty, ret_ty, &[Operator::Div]);
}

/// `FloorDiv`
pub fn impl_floordiv(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(unifier, store, ty, other_ty, ret_ty, &[Operator::FloorDiv]);
}

/// `Mod`
pub fn impl_mod(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(unifier, store, ty, other_ty, ret_ty, &[Operator::Mod]);
}

/// [`Operator::MatMult`]
pub fn impl_matmul(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_binop(unifier, store, ty, other_ty, ret_ty, &[Operator::MatMult]);
}

/// `UAdd`, `USub`
pub fn impl_sign(unifier: &mut Unifier, _store: &PrimitiveStore, ty: Type, ret_ty: Option<Type>) {
    impl_unaryop(unifier, ty, ret_ty, &[Unaryop::UAdd, Unaryop::USub]);
}

/// `Invert`
pub fn impl_invert(unifier: &mut Unifier, _store: &PrimitiveStore, ty: Type, ret_ty: Option<Type>) {
    impl_unaryop(unifier, ty, ret_ty, &[Unaryop::Invert]);
}

/// `Not`
pub fn impl_not(unifier: &mut Unifier, _store: &PrimitiveStore, ty: Type, ret_ty: Option<Type>) {
    impl_unaryop(unifier, ty, ret_ty, &[Unaryop::Not]);
}

/// `Lt`, `LtE`, `Gt`, `GtE`
pub fn impl_comparison(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_cmpop(
        unifier,
        store,
        ty,
        other_ty,
        &[Cmpop::Lt, Cmpop::Gt, Cmpop::LtE, Cmpop::GtE],
        ret_ty,
    );
}

/// `Eq`, `NotEq`
pub fn impl_eq(
    unifier: &mut Unifier,
    store: &PrimitiveStore,
    ty: Type,
    other_ty: &[Type],
    ret_ty: Option<Type>,
) {
    impl_cmpop(unifier, store, ty, other_ty, &[Cmpop::Eq, Cmpop::NotEq], ret_ty);
}

/// Returns the expected return type of binary operations with at least one `ndarray` operand.
pub fn typeof_ndarray_broadcast(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    left: Type,
    right: Type,
) -> Result<Type, String> {
    let is_left_ndarray = left.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());
    let is_right_ndarray = right.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());

    assert!(is_left_ndarray || is_right_ndarray);

    if is_left_ndarray && is_right_ndarray {
        // Perform broadcasting on two ndarray operands.

        let (left_ty_dtype, left_ty_ndims) = unpack_ndarray_var_tys(unifier, left);
        let (right_ty_dtype, right_ty_ndims) = unpack_ndarray_var_tys(unifier, right);

        assert!(unifier.unioned(left_ty_dtype, right_ty_dtype));

        let left_ty_ndims = match &*unifier.get_ty_immutable(left_ty_ndims) {
            TypeEnum::TLiteral { values, .. } => values.clone(),
            _ => unreachable!(),
        };
        let right_ty_ndims = match &*unifier.get_ty_immutable(right_ty_ndims) {
            TypeEnum::TLiteral { values, .. } => values.clone(),
            _ => unreachable!(),
        };

        let res_ndims = left_ty_ndims
            .into_iter()
            .cartesian_product(right_ty_ndims)
            .map(|(left, right)| {
                let left_val = u64::try_from(left).unwrap();
                let right_val = u64::try_from(right).unwrap();

                max(left_val, right_val)
            })
            .unique()
            .map(SymbolValue::U64)
            .collect_vec();
        let res_ndims = unifier.get_fresh_literal(res_ndims, None);

        Ok(make_ndarray_ty(unifier, primitives, Some(left_ty_dtype), Some(res_ndims)))
    } else {
        let (ndarray_ty, scalar_ty) = if is_left_ndarray { (left, right) } else { (right, left) };

        let (ndarray_ty_dtype, _) = unpack_ndarray_var_tys(unifier, ndarray_ty);

        if unifier.unioned(ndarray_ty_dtype, scalar_ty) {
            Ok(ndarray_ty)
        } else {
            let (expected_ty, actual_ty) = if is_left_ndarray {
                (ndarray_ty_dtype, scalar_ty)
            } else {
                (scalar_ty, ndarray_ty_dtype)
            };

            Err(format!(
                "Expected right-hand side operand to be {}, got {}",
                unifier.stringify(expected_ty),
                unifier.stringify(actual_ty),
            ))
        }
    }
}

/// Returns the return type given a binary operator and its primitive operands.
pub fn typeof_binop(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    op: &Operator,
    lhs: Type,
    rhs: Type,
) -> Result<Option<Type>, String> {
    let is_left_ndarray = lhs.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());
    let is_right_ndarray = rhs.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());

    Ok(Some(match op {
        Operator::Add | Operator::Sub | Operator::Mult | Operator::Mod | Operator::FloorDiv => {
            if is_left_ndarray || is_right_ndarray {
                typeof_ndarray_broadcast(unifier, primitives, lhs, rhs)?
            } else if unifier.unioned(lhs, rhs) {
                lhs
            } else {
                return Ok(None);
            }
        }

        Operator::MatMult => {
            let (_, lhs_ndims) = unpack_ndarray_var_tys(unifier, lhs);
            let lhs_ndims = match &*unifier.get_ty_immutable(lhs_ndims) {
                TypeEnum::TLiteral { values, .. } => {
                    assert_eq!(values.len(), 1);
                    u64::try_from(values[0].clone()).unwrap()
                }
                _ => unreachable!(),
            };
            let (_, rhs_ndims) = unpack_ndarray_var_tys(unifier, rhs);
            let rhs_ndims = match &*unifier.get_ty_immutable(rhs_ndims) {
                TypeEnum::TLiteral { values, .. } => {
                    assert_eq!(values.len(), 1);
                    u64::try_from(values[0].clone()).unwrap()
                }
                _ => unreachable!(),
            };

            match (lhs_ndims, rhs_ndims) {
                (2, 2) => typeof_ndarray_broadcast(unifier, primitives, lhs, rhs)?,
                (lhs, rhs) if lhs == 0 || rhs == 0 => {
                    return Err(format!(
                    "Input operand {} does not have enough dimensions (has {lhs}, requires {rhs})",
                    (rhs == 0) as u8
                ))
                }
                (lhs, rhs) => {
                    return Err(format!(
                        "ndarray.__matmul__ on {lhs}D and {rhs}D operands not supported"
                    ))
                }
            }
        }

        Operator::Div => {
            if is_left_ndarray || is_right_ndarray {
                typeof_ndarray_broadcast(unifier, primitives, lhs, rhs)?
            } else if unifier.unioned(lhs, rhs) {
                primitives.float
            } else {
                return Ok(None);
            }
        }

        Operator::Pow => {
            if is_left_ndarray || is_right_ndarray {
                typeof_ndarray_broadcast(unifier, primitives, lhs, rhs)?
            } else if [
                primitives.int32,
                primitives.int64,
                primitives.uint32,
                primitives.uint64,
                primitives.float,
            ]
            .into_iter()
            .any(|ty| unifier.unioned(lhs, ty))
            {
                lhs
            } else {
                return Ok(None);
            }
        }

        Operator::LShift | Operator::RShift => lhs,
        Operator::BitOr | Operator::BitXor | Operator::BitAnd => {
            if unifier.unioned(lhs, rhs) {
                lhs
            } else {
                return Ok(None);
            }
        }
    }))
}

pub fn typeof_unaryop(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    op: &Unaryop,
    operand: Type,
) -> Result<Option<Type>, String> {
    let operand_obj_id = operand.obj_id(unifier);

    if *op == Unaryop::Not
        && operand_obj_id.is_some_and(|id| id == primitives.ndarray.obj_id(unifier).unwrap())
    {
        return Err(
            "The truth value of an array with more than one element is ambiguous".to_string()
        );
    }

    Ok(match *op {
        Unaryop::Not => match operand_obj_id {
            Some(v) if v == PrimDef::NDArray.id() => Some(operand),
            Some(_) => Some(primitives.bool),
            _ => None,
        },

        Unaryop::Invert => {
            if operand_obj_id.is_some_and(|id| id == PrimDef::Bool.id()) {
                Some(primitives.int32)
            } else if operand_obj_id.is_some_and(|id| PrimDef::iter().any(|prim| id == prim.id())) {
                Some(operand)
            } else {
                None
            }
        }

        Unaryop::UAdd | Unaryop::USub => {
            if operand_obj_id.is_some_and(|id| id == PrimDef::NDArray.id()) {
                let (dtype, _) = unpack_ndarray_var_tys(unifier, operand);
                if dtype.obj_id(unifier).is_some_and(|id| id == PrimDef::Bool.id()) {
                    return Err(if *op == Unaryop::UAdd {
                        "The ufunc 'positive' cannot be applied to ndarray[bool, N]".to_string()
                    } else {
                        "The numpy boolean negative, the `-` operator, is not supported, use the `~` operator function instead.".to_string()
                    });
                }

                Some(operand)
            } else if operand_obj_id.is_some_and(|id| id == PrimDef::Bool.id()) {
                Some(primitives.int32)
            } else if operand_obj_id.is_some_and(|id| PrimDef::iter().any(|prim| id == prim.id())) {
                Some(operand)
            } else {
                None
            }
        }
    })
}

/// Returns the return type given a comparison operator and its primitive operands.
pub fn typeof_cmpop(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    _op: &Cmpop,
    lhs: Type,
    rhs: Type,
) -> Result<Option<Type>, String> {
    let is_left_ndarray = lhs.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());
    let is_right_ndarray = rhs.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id());

    Ok(Some(if is_left_ndarray || is_right_ndarray {
        let brd = typeof_ndarray_broadcast(unifier, primitives, lhs, rhs)?;
        let (_, ndims) = unpack_ndarray_var_tys(unifier, brd);

        make_ndarray_ty(unifier, primitives, Some(primitives.bool), Some(ndims))
    } else if unifier.unioned(lhs, rhs) {
        primitives.bool
    } else {
        return Ok(None);
    }))
}

pub fn set_primitives_magic_methods(store: &PrimitiveStore, unifier: &mut Unifier) {
    let PrimitiveStore {
        int32: int32_t,
        int64: int64_t,
        float: float_t,
        bool: bool_t,
        uint32: uint32_t,
        uint64: uint64_t,
        ndarray: ndarray_t,
        ..
    } = *store;
    let size_t = store.usize();

    /* int ======== */
    for t in [int32_t, int64_t, uint32_t, uint64_t] {
        let ndarray_int_t = make_ndarray_ty(unifier, store, Some(t), None);
        impl_basic_arithmetic(unifier, store, t, &[t, ndarray_int_t], None);
        impl_pow(unifier, store, t, &[t, ndarray_int_t], None);
        impl_bitwise_arithmetic(unifier, store, t);
        impl_bitwise_shift(unifier, store, t);
        impl_div(unifier, store, t, &[t, ndarray_int_t], None);
        impl_floordiv(unifier, store, t, &[t, ndarray_int_t], None);
        impl_mod(unifier, store, t, &[t, ndarray_int_t], None);
        impl_invert(unifier, store, t, Some(t));
        impl_not(unifier, store, t, Some(bool_t));
        impl_comparison(unifier, store, t, &[t, ndarray_int_t], None);
        impl_eq(unifier, store, t, &[t, ndarray_int_t], None);
    }
    for t in [int32_t, int64_t] {
        impl_sign(unifier, store, t, Some(t));
    }

    /* float ======== */
    let ndarray_float_t = make_ndarray_ty(unifier, store, Some(float_t), None);
    let ndarray_int32_t = make_ndarray_ty(unifier, store, Some(int32_t), None);
    impl_basic_arithmetic(unifier, store, float_t, &[float_t, ndarray_float_t], None);
    impl_pow(unifier, store, float_t, &[int32_t, float_t, ndarray_int32_t, ndarray_float_t], None);
    impl_div(unifier, store, float_t, &[float_t, ndarray_float_t], None);
    impl_floordiv(unifier, store, float_t, &[float_t, ndarray_float_t], None);
    impl_mod(unifier, store, float_t, &[float_t, ndarray_float_t], None);
    impl_sign(unifier, store, float_t, Some(float_t));
    impl_not(unifier, store, float_t, Some(bool_t));
    impl_comparison(unifier, store, float_t, &[float_t, ndarray_float_t], None);
    impl_eq(unifier, store, float_t, &[float_t, ndarray_float_t], None);

    /* bool ======== */
    let ndarray_bool_t = make_ndarray_ty(unifier, store, Some(bool_t), None);
    impl_invert(unifier, store, bool_t, Some(int32_t));
    impl_not(unifier, store, bool_t, Some(bool_t));
    impl_sign(unifier, store, bool_t, Some(int32_t));
    impl_eq(unifier, store, bool_t, &[bool_t, ndarray_bool_t], None);

    /* ndarray ===== */
    let ndarray_usized_ndims_tvar =
        unifier.get_fresh_const_generic_var(size_t, Some("ndarray_ndims".into()), None);
    let ndarray_unsized_t =
        make_ndarray_ty(unifier, store, None, Some(ndarray_usized_ndims_tvar.0));
    let (ndarray_dtype_t, _) = unpack_ndarray_var_tys(unifier, ndarray_t);
    let (ndarray_unsized_dtype_t, _) = unpack_ndarray_var_tys(unifier, ndarray_unsized_t);
    impl_basic_arithmetic(
        unifier,
        store,
        ndarray_t,
        &[ndarray_unsized_t, ndarray_unsized_dtype_t],
        None,
    );
    impl_pow(unifier, store, ndarray_t, &[ndarray_unsized_t, ndarray_unsized_dtype_t], None);
    impl_div(unifier, store, ndarray_t, &[ndarray_t, ndarray_dtype_t], None);
    impl_floordiv(unifier, store, ndarray_t, &[ndarray_unsized_t, ndarray_unsized_dtype_t], None);
    impl_mod(unifier, store, ndarray_t, &[ndarray_unsized_t, ndarray_unsized_dtype_t], None);
    impl_matmul(unifier, store, ndarray_t, &[ndarray_t], Some(ndarray_t));
    impl_sign(unifier, store, ndarray_t, Some(ndarray_t));
    impl_invert(unifier, store, ndarray_t, Some(ndarray_t));
    impl_eq(unifier, store, ndarray_t, &[ndarray_unsized_t, ndarray_unsized_dtype_t], None);
    impl_comparison(unifier, store, ndarray_t, &[ndarray_unsized_t, ndarray_unsized_dtype_t], None);
}
