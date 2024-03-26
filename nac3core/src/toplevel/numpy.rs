use itertools::Itertools;
use crate::{
    toplevel::helper::PRIMITIVE_DEF_IDS,
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{Type, TypeEnum, Unifier, VarMap},
    },
};

/// Creates a `ndarray` [`Type`] with the given type arguments.
/// 
/// * `dtype` - The element type of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
/// * `ndims` - The number of dimensions of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
pub fn make_ndarray_ty(
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    dtype: Option<Type>,
    ndims: Option<Type>,
) -> Type {
    subst_ndarray_tvars(unifier, primitives.ndarray, dtype, ndims)
}

/// Substitutes type variables in `ndarray`.
///
/// * `dtype` - The element type of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
/// * `ndims` - The number of dimensions of the `ndarray`, or [`None`] if the type variable is not
/// specialized.
pub fn subst_ndarray_tvars(
    unifier: &mut Unifier,
    ndarray: Type,
    dtype: Option<Type>,
    ndims: Option<Type>,
) -> Type {
    let TypeEnum::TObj { obj_id, params, .. } = &*unifier.get_ty_immutable(ndarray) else {
        panic!("Expected `ndarray` to be TObj, but got {}", unifier.stringify(ndarray))
    };
    debug_assert_eq!(*obj_id, PRIMITIVE_DEF_IDS.ndarray);

    if dtype.is_none() && ndims.is_none() {
        return ndarray
    }

    let tvar_ids = params.iter()
        .map(|(obj_id, _)| *obj_id)
        .collect_vec();
    debug_assert_eq!(tvar_ids.len(), 2);

    let mut tvar_subst = VarMap::new();
    if let Some(dtype) = dtype {
        tvar_subst.insert(tvar_ids[0], dtype);
    }
    if let Some(ndims) = ndims {
        tvar_subst.insert(tvar_ids[1], ndims);
    }

    unifier.subst(ndarray, &tvar_subst).unwrap_or(ndarray)
}

fn unpack_ndarray_tvars(
    unifier: &mut Unifier,
    ndarray: Type,
) -> Vec<(u32, Type)> {
    let TypeEnum::TObj { obj_id, params, .. } = &*unifier.get_ty_immutable(ndarray) else {
        panic!("Expected `ndarray` to be TObj, but got {}", unifier.stringify(ndarray))
    };
    debug_assert_eq!(*obj_id, PRIMITIVE_DEF_IDS.ndarray);
    debug_assert_eq!(params.len(), 2);

    params.iter()
        .sorted_by_key(|(obj_id, _)| *obj_id)
        .map(|(var_id, ty)| (*var_id, *ty))
        .collect_vec()
}

/// Unpacks the type variable IDs of `ndarray` into a tuple. The elements of the tuple corresponds 
/// to `dtype` (the element type) and `ndims` (the number of dimensions) of the `ndarray` 
/// respectively.
pub fn unpack_ndarray_var_ids(
    unifier: &mut Unifier,
    ndarray: Type,
) -> (u32, u32) {
    unpack_ndarray_tvars(unifier, ndarray)
        .into_iter()
        .map(|v| v.0)
        .collect_tuple()
        .unwrap()
}

/// Unpacks the type variables of `ndarray` into a tuple. The elements of the tuple corresponds to
/// `dtype` (the element type) and `ndims` (the number of dimensions) of the `ndarray` respectively.
pub fn unpack_ndarray_var_tys(
    unifier: &mut Unifier,
    ndarray: Type,
) -> (Type, Type) {
    unpack_ndarray_tvars(unifier, ndarray)
        .into_iter()
        .map(|v| v.1)
        .collect_tuple()
        .unwrap()
}
