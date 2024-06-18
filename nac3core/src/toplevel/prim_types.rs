use crate::{
    toplevel::helper::PrimDef,
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{Type, TypeEnum, Unifier, VarMap},
    },
};
use itertools::Itertools;

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
    debug_assert_eq!(*obj_id, PrimDef::NDArray.id());

    if dtype.is_none() && ndims.is_none() {
        return ndarray;
    }

    let tvar_ids = params.iter().map(|(obj_id, _)| *obj_id).collect_vec();
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

#[derive(Clone, Copy, Debug)]
pub struct NDArrayParams {
    pub dtype: Type,
    pub ndims: Type,
}

/// Extract the [`Type`]s of `ndarray`.
#[must_use]
pub fn unpack_ndarray_params(
    unifier: &Unifier,
    store: &PrimitiveStore,
    ndarray: Type,
) -> NDArrayParams {
    let TypeEnum::TObj { obj_id, params, .. } = &*unifier.get_ty_immutable(ndarray) else {
        panic!("Expected `ndarray` to be TObj, but got {}", unifier.stringify(ndarray))
    };
    debug_assert_eq!(*obj_id, PrimDef::NDArray.id());
    debug_assert_eq!(params.len(), 2);
    let dtype = *params.get(&store.ndarray_dtype_tvar.id).unwrap();
    let ndims = *params.get(&store.ndarray_ndims_tvar.id).unwrap();
    NDArrayParams { dtype, ndims }
}
