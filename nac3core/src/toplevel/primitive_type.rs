use crate::toplevel::helper::PrimDef;
use crate::typecheck::type_inferencer::PrimitiveStore;
use crate::typecheck::typedef::{GenericObjectType, Type, TypeVar, Unifier, VarMap};

#[derive(Clone, Copy)]
pub struct OptionType(Type);

impl OptionType {
    pub fn from_primitive(
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
        type_ty: Option<Type>,
    ) -> Self {
        primitives.option.subst(unifier, type_ty)
    }

    pub fn type_tvar(&self, unifier: &mut Unifier) -> TypeVar {
        self.get_var_at(unifier, 0).unwrap()
    }

    #[must_use]
    pub fn subst(&self, unifier: &mut Unifier, type_ty: Option<Type>) -> Self {
        let new_vars = [(self.type_tvar(unifier).id, type_ty)]
            .into_iter()
            .filter_map(|(id, ty)| ty.map(|ty| (id, ty)))
            .collect::<VarMap>();

        let new_ty = unifier.subst(self.get_type(), &new_vars).unwrap_or(self.get_type());
        OptionType(new_ty)
    }
}

impl GenericObjectType for OptionType {
    fn try_create(ty: Type, unifier: &mut Unifier) -> Option<Self> {
        if ty.obj_id(unifier).is_some_and(|id| id == PrimDef::Option.id()) {
            Some(OptionType(ty))
        } else {
            None
        }
    }

    fn get_type(&self) -> Type {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct NDArrayType(Type);

impl NDArrayType {
    pub fn from_primitive(
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
        dtype: Option<Type>,
        ndims: Option<Type>,
    ) -> Self {
        primitives.ndarray.subst(unifier, dtype, ndims)
    }

    pub fn dtype_tvar(&self, unifier: &mut Unifier) -> TypeVar {
        self.get_var_at(unifier, 0).unwrap()
    }

    pub fn ndims_tvar(&self, unifier: &mut Unifier) -> TypeVar {
        self.get_var_at(unifier, 1).unwrap()
    }

    #[must_use]
    pub fn subst(
        &self,
        unifier: &mut Unifier,
        dtype_ty: Option<Type>,
        ndims_ty: Option<Type>,
    ) -> Self {
        let new_vars =
            [(self.dtype_tvar(unifier).id, dtype_ty), (self.ndims_tvar(unifier).id, ndims_ty)]
                .into_iter()
                .filter_map(|(id, ty)| ty.map(|ty| (id, ty)))
                .collect::<VarMap>();

        let new_ty = unifier.subst(self.get_type(), &new_vars).unwrap_or(self.get_type());
        NDArrayType(new_ty)
    }
}

impl GenericObjectType for NDArrayType {
    fn try_create(ty: Type, unifier: &mut Unifier) -> Option<Self> {
        if ty.obj_id(unifier).is_some_and(|id| id == PrimDef::NDArray.id()) {
            Some(NDArrayType(ty))
        } else {
            None
        }
    }

    fn get_type(&self) -> Type {
        self.0
    }
}
