use indexmap::IndexMap;
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::iter::zip;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{borrow::Cow, collections::HashSet};

use nac3parser::ast::{Location, StrRef};

use super::type_error::{TypeError, TypeErrorKind};
use super::unification_table::{UnificationKey, UnificationTable};
use crate::symbol_resolver::SymbolValue;
use crate::toplevel::{DefinitionId, TopLevelContext, TopLevelDef};
use crate::typecheck::type_inferencer::PrimitiveStore;

#[cfg(test)]
mod test;

/// Handle for a type, implemented as a key in the unification table.
pub type Type = UnificationKey;

/// Macro for generating functions related to type traits, e.g. whether the type is integral.
macro_rules! primitive_type_trait_fn {
    ($id:ident, $( $matches:ident ),*) => {
        #[must_use]
        pub fn $id(self, unifier: &mut Unifier, store: &PrimitiveStore) -> bool {
            [$(store.$matches,)*].into_iter().any(|ty| unifier.unioned(self, ty))
        }
    };
}

impl Type {
    /// Wrapper function for cleaner code so that we don't need to write this long pattern matching
    /// just to get the field `obj_id`.
    #[must_use]
    pub fn obj_id(self, unifier: &Unifier) -> Option<DefinitionId> {
        if let TypeEnum::TObj { obj_id, .. } = &*unifier.get_ty_immutable(self) {
            Some(*obj_id)
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_primitive(self, unifier: &mut Unifier, store: &PrimitiveStore) -> bool {
        store.into_iter().any(|ty| unifier.unioned(self, ty))
    }

    primitive_type_trait_fn!(is_integral, bool, int32, int64, uint32, uint64);
    primitive_type_trait_fn!(is_floating_point, float);
    primitive_type_trait_fn!(is_arithmetic, int32, int64, uint32, uint64, float);
    primitive_type_trait_fn!(is_signed, int32, uint32, float);
    primitive_type_trait_fn!(is_unsigned, uint32, uint64);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CallId(pub(super) usize);

pub type Mapping<K, V = Type> = HashMap<K, V>;
pub type IndexMapping<K, V = Type> = IndexMap<K, V>;

/// ID of a Python type variable. Specific to `nac3core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeVarId(u32);

impl From<TypeVarId> for u32 {
    fn from(value: TypeVarId) -> Self {
        value.0
    }
}

impl fmt::Display for TypeVarId {
    // NOTE: Must output the string of the ID value. Certain unit tests rely on string comparisons.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// A Python type variable. Used by `nac3core` during type inference.
#[derive(Debug, Clone, Copy)]
pub struct TypeVar {
    /// `nac3core`'s internal [`TypeVarId`] of this type variable.
    pub id: TypeVarId,

    /// The assigned [`Type`] of this Python type variable.
    pub ty: Type,
}

impl From<(TypeVarId, Type)> for TypeVar {
    fn from((id, ty): (TypeVarId, Type)) -> Self {
        TypeVar { id, ty }
    }
}

impl From<(&TypeVarId, &Type)> for TypeVar {
    fn from((id, ty): (&TypeVarId, &Type)) -> Self {
        TypeVar { id: *id, ty: *ty }
    }
}

impl From<TypeVar> for (TypeVarId, Type) {
    fn from(value: TypeVar) -> Self {
        (value.id, value.ty)
    }
}

/// The mapping between [`TypeVarId`] and [unifier type][`Type`].
pub type VarMap = IndexMapping<TypeVarId>;

/// Build a [`VarMap`] from an iterator of [`TypeVar`]
///
/// The resulting [`VarMap`] will have the same order as the input iterator.
pub fn into_var_map<I>(vars: I) -> VarMap
where
    I: IntoIterator<Item = TypeVar>,
{
    vars.into_iter().map(|var| (var.id, var.ty)).collect()
}

/// A trait representing a possibly generic object type.
pub trait GenericObjectType
where
    Self: Sized,
{
    fn try_create(ty: Type, unifier: &mut Unifier) -> Option<Self>;

    /// Creates an instance from a [`Type`].
    #[must_use]
    fn create(ty: Type, unifier: &mut Unifier) -> Self {
        Self::try_create(ty, unifier).unwrap()
    }

    /// Returns the [`Type`] underlying this instance.
    #[must_use]
    fn get_type(&self) -> Type;

    /// Similar to [`Type::obj_id`], except that the [`DefinitionId`] is not wrapped within an
    /// [`Option`].
    #[must_use]
    fn obj_id(&self, unifier: &Unifier) -> DefinitionId {
        self.get_type().obj_id(unifier).unwrap()
    }

    /// Returns a copy of the [`VarMap`] of this object type.
    #[must_use]
    fn var_map(&self, unifier: &mut Unifier) -> VarMap {
        let TypeEnum::TObj { params, .. } = &*unifier.get_ty(self.get_type()) else {
            unreachable!()
        };

        params.clone()
    }

    /// Creates an iterator over the [`VarMap`] of this object type, applying `iter_fn` on the
    /// created [`Iterator`].
    #[must_use]
    fn iter_var_map<R, IterFn: FnOnce(&mut dyn Iterator<Item = TypeVar>, &mut Unifier) -> R>(
        &self,
        unifier: &mut Unifier,
        iter_fn: IterFn,
    ) -> R {
        let TypeEnum::TObj { params, .. } = &*unifier.get_ty(self.get_type()) else {
            unreachable!()
        };

        let res = iter_fn(&mut params.iter().map(TypeVar::from), unifier);
        res
    }

    /// Returns the [`TypeVar`] instance at the given index.
    #[must_use]
    fn get_var_at(&self, unifier: &mut Unifier, i: usize) -> Option<TypeVar> {
        self.iter_var_map(unifier, |iter, _| iter.nth(i))
    }
}

impl<T: GenericObjectType> From<T> for Type {
    fn from(value: T) -> Self {
        value.get_type()
    }
}

/// An adapter that converts [`Type`] into
pub struct GenericTypeAdapter(Type);

impl GenericObjectType for GenericTypeAdapter {
    fn try_create(ty: Type, unifier: &mut Unifier) -> Option<Self> {
        if let TypeEnum::TObj { .. } = &*unifier.get_ty_immutable(ty) {
            Some(GenericTypeAdapter(ty))
        } else {
            None
        }
    }

    fn get_type(&self) -> Type {
        self.0
    }
}

#[derive(Clone)]
pub struct Call {
    pub posargs: Vec<Type>,
    pub kwargs: HashMap<StrRef, Type>,
    pub ret: Type,
    pub fun: RefCell<Option<Type>>,
    pub loc: Option<Location>,
}

#[derive(Debug, Clone)]
pub struct FuncArg {
    pub name: StrRef,
    pub ty: Type,
    pub default_value: Option<SymbolValue>,
}

impl FuncArg {
    #[must_use]
    pub fn is_required(&self) -> bool {
        self.default_value.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct FunSignature {
    pub args: Vec<FuncArg>,
    pub ret: Type,
    pub vars: VarMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKey {
    Str(StrRef),
    Int(i32),
}

impl From<&RecordKey> for StrRef {
    fn from(r: &RecordKey) -> Self {
        match r {
            RecordKey::Str(s) => *s,
            RecordKey::Int(i) => StrRef::from(i.to_string()),
        }
    }
}

impl From<StrRef> for RecordKey {
    fn from(s: StrRef) -> Self {
        RecordKey::Str(s)
    }
}

impl From<&str> for RecordKey {
    fn from(s: &str) -> Self {
        RecordKey::Str(s.into())
    }
}

impl From<i32> for RecordKey {
    fn from(i: i32) -> Self {
        RecordKey::Int(i)
    }
}

impl Display for RecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordKey::Str(s) => write!(f, "{s}"),
            RecordKey::Int(i) => write!(f, "{i}"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RecordField {
    ty: Type,
    mutable: bool,
    loc: Option<Location>,
}

impl RecordField {
    #[must_use]
    pub fn new(ty: Type, mutable: bool, loc: Option<Location>) -> RecordField {
        RecordField { ty, mutable, loc }
    }
}

/// Category of variable and value types.
#[derive(Debug, Clone)]
pub enum TypeEnum {
    TRigidVar {
        id: TypeVarId,
        name: Option<StrRef>,
        loc: Option<Location>,
    },

    /// A type variable.
    TVar {
        id: TypeVarId,
        // empty indicates this is not a struct/tuple/list
        fields: Option<Mapping<RecordKey, RecordField>>,
        // empty indicates no restriction
        range: Vec<Type>,
        name: Option<StrRef>,
        loc: Option<Location>,
        /// Whether this type variable refers to a const-generic variable.
        is_const_generic: bool,
    },

    /// A literal generic type matching `typing.Literal`.
    TLiteral {
        /// The value of the constant.
        values: Vec<SymbolValue>,
        loc: Option<Location>,
    },

    /// A tuple type.
    TTuple {
        /// The types of elements present in this tuple.
        ty: Vec<Type>,
    },

    /// A list type.
    TList {
        /// The type of elements present in this list.
        ty: Type,
    },

    /// An object type.
    TObj {
        /// The [`DefinitionId`] of this object type.
        obj_id: DefinitionId,

        /// The fields present in this object type.
        ///
        /// The key of the [Mapping] is the identifier of the field, while the value is a tuple
        /// containing the [Type] of the field, and a `bool` indicating whether the field is a
        /// variable (as opposed to a function).
        fields: Mapping<StrRef, (Type, bool)>,

        /// Mapping between the ID of type variables and the [Type] representing the type variables
        /// of this object type.
        params: VarMap,
    },
    TVirtual {
        ty: Type,
    },
    TCall(Vec<CallId>),

    /// A function type.
    TFunc(FunSignature),
}

impl TypeEnum {
    #[must_use]
    pub fn get_type_name(&self) -> &'static str {
        match self {
            TypeEnum::TRigidVar { .. } => "TRigidVar",
            TypeEnum::TVar { .. } => "TVar",
            TypeEnum::TLiteral { .. } => "TConstant",
            TypeEnum::TTuple { .. } => "TTuple",
            TypeEnum::TList { .. } => "TList",
            TypeEnum::TObj { .. } => "TObj",
            TypeEnum::TVirtual { .. } => "TVirtual",
            TypeEnum::TCall { .. } => "TCall",
            TypeEnum::TFunc { .. } => "TFunc",
        }
    }
}

pub type SharedUnifier = Arc<Mutex<(UnificationTable<TypeEnum>, u32, Vec<Call>)>>;

#[derive(Clone)]
pub struct Unifier {
    pub(crate) top_level: Option<Arc<TopLevelContext>>,
    pub(crate) unification_table: UnificationTable<Rc<TypeEnum>>,
    pub(crate) calls: Vec<Rc<Call>>,
    var_id_counter: u32,
    unify_cache: HashSet<(Type, Type)>,
    snapshot: Option<(usize, u32)>,
    primitive_store: Option<PrimitiveStore>,
}

impl Default for Unifier {
    fn default() -> Self {
        Unifier::new()
    }
}

impl Unifier {
    /// Get an empty unifier
    #[must_use]
    pub fn new() -> Unifier {
        Unifier {
            unification_table: UnificationTable::new(),
            var_id_counter: 0,
            calls: Vec::new(),
            unify_cache: HashSet::new(),
            top_level: None,
            snapshot: None,
            primitive_store: None,
        }
    }

    /// Sets the [`PrimitiveStore`] instance within this `Unifier`.
    ///
    /// This function can only be invoked once. Any subsequent invocations will result in an
    /// assertion error.
    pub fn put_primitive_store(&mut self, primitives: &PrimitiveStore) {
        assert!(self.primitive_store.is_none());
        self.primitive_store.replace(*primitives);
    }

    /// Returns the [`UnificationTable`] associated with this `Unifier`.
    ///
    /// # Safety
    ///
    /// The use of this function is discouraged under most circumstances. Only use this function if
    /// in-place manipulation of type variables and/or type fields is necessary, otherwise prefer to
    /// [add a new type][`Unifier::add_ty`] and [unify the type][`Unifier::unify`] with an existing
    /// type.
    pub unsafe fn get_unification_table(&mut self) -> &mut UnificationTable<Rc<TypeEnum>> {
        &mut self.unification_table
    }

    /// Determine if the two types are the same
    pub fn unioned(&mut self, a: Type, b: Type) -> bool {
        self.unification_table.unioned(a, b)
    }

    pub fn from_shared_unifier(unifier: &SharedUnifier) -> Unifier {
        let lock = unifier.lock().unwrap();
        Unifier {
            unification_table: UnificationTable::from_send(&lock.0),
            var_id_counter: lock.1,
            calls: lock.2.iter().map(|v| Rc::new(v.clone())).collect_vec(),
            top_level: None,
            unify_cache: HashSet::new(),
            snapshot: None,
            primitive_store: None,
        }
    }

    #[must_use]
    pub fn get_shared_unifier(&self) -> SharedUnifier {
        Arc::new(Mutex::new((
            self.unification_table.get_send(),
            self.var_id_counter,
            self.calls.iter().map(|v| v.as_ref().clone()).collect_vec(),
        )))
    }

    /// Register a type to the unifier.
    /// Returns a key in the `unification_table`.
    pub fn add_ty(&mut self, a: TypeEnum) -> Type {
        self.unification_table.new_key(Rc::new(a))
    }

    pub fn add_record(&mut self, fields: Mapping<RecordKey, RecordField>) -> Type {
        let id = self.generate_var_id();
        self.add_ty(TypeEnum::TVar {
            id,
            range: vec![],
            fields: Some(fields),
            name: None,
            loc: None,
            is_const_generic: false,
        })
    }

    pub fn add_call(&mut self, call: Call) -> CallId {
        let id = CallId(self.calls.len());
        self.calls.push(Rc::new(call));
        id
    }

    pub fn get_call_signature(&mut self, id: CallId) -> Option<FunSignature> {
        let fun = self.calls.get(id.0).unwrap().fun.borrow().unwrap();
        if let TypeEnum::TFunc(sign) = &*self.get_ty(fun) {
            Some(sign.clone())
        } else {
            None
        }
    }

    #[must_use]
    pub fn get_call_signature_immutable(&self, id: CallId) -> Option<FunSignature> {
        let fun = self.calls.get(id.0).unwrap().fun.borrow().unwrap();
        if let TypeEnum::TFunc(sign) = &*self.get_ty_immutable(fun) {
            Some(sign.clone())
        } else {
            None
        }
    }

    pub fn get_representative(&mut self, ty: Type) -> Type {
        self.unification_table.get_representative(ty)
    }

    /// Get the `TypeEnum` of a type.
    pub fn get_ty(&mut self, a: Type) -> Rc<TypeEnum> {
        self.unification_table.probe_value(a).clone()
    }

    #[must_use]
    pub fn get_ty_immutable(&self, a: Type) -> Rc<TypeEnum> {
        self.unification_table.probe_value_immutable(a).clone()
    }

    pub fn get_fresh_rigid_var(&mut self, name: Option<StrRef>, loc: Option<Location>) -> TypeVar {
        let id = self.generate_var_id();
        let ty = self.add_ty(TypeEnum::TRigidVar { id, name, loc });
        TypeVar { id, ty }
    }

    pub fn get_dummy_var(&mut self) -> TypeVar {
        self.get_fresh_var_with_range(&[], None, None)
    }

    /// Returns a fresh [type variable][TypeEnum::TVar] with no associated range.
    ///
    /// This type variable can be instantiated by any type.
    pub fn get_fresh_var(&mut self, name: Option<StrRef>, loc: Option<Location>) -> TypeVar {
        self.get_fresh_var_with_range(&[], name, loc)
    }

    /// Returns a fresh [type variable][TypeEnum::TVar] with the range specified by `range`.
    ///
    /// This type variable can be instantiated by any type present in `range`.
    pub fn get_fresh_var_with_range(
        &mut self,
        range: &[Type],
        name: Option<StrRef>,
        loc: Option<Location>,
    ) -> TypeVar {
        let range = range.to_vec();

        let id = self.generate_var_id();
        let ty = self.add_ty(TypeEnum::TVar {
            id,
            range,
            fields: None,
            name,
            loc,
            is_const_generic: false,
        });
        TypeVar { id, ty }
    }

    /// Returns a fresh type representing a constant generic variable with the given underlying type `ty`.
    pub fn get_fresh_const_generic_var(
        &mut self,
        ty: Type,
        name: Option<StrRef>,
        loc: Option<Location>,
    ) -> TypeVar {
        let id = self.generate_var_id();
        let ty = self.add_ty(TypeEnum::TVar {
            id,
            range: vec![ty],
            fields: None,
            name,
            loc,
            is_const_generic: true,
        });
        TypeVar { id, ty }
    }

    /// Returns a fresh type representing a [literal][TypeEnum::TConstant] with the given `values`.
    pub fn get_fresh_literal(&mut self, values: Vec<SymbolValue>, loc: Option<Location>) -> Type {
        let ty_enum = TypeEnum::TLiteral { values: values.into_iter().dedup().collect(), loc };
        self.add_ty(ty_enum)
    }

    /// Unification would not unify rigid variables with other types, but we want to do this for
    /// function instantiations, so we make it explicit.
    pub fn replace_rigid_var(&mut self, rigid: Type, b: Type) {
        assert!(matches!(&*self.get_ty(rigid), TypeEnum::TRigidVar { .. }));
        self.set_a_to_b(rigid, b);
    }

    pub fn get_instantiations(&mut self, ty: Type) -> Option<Vec<Type>> {
        match &*self.get_ty(ty) {
            TypeEnum::TVar { range, .. } => {
                if range.is_empty() {
                    None
                } else {
                    Some(
                        range
                            .iter()
                            .flat_map(|ty| {
                                self.get_instantiations(*ty).unwrap_or_else(|| vec![*ty])
                            })
                            .collect_vec(),
                    )
                }
            }
            TypeEnum::TList { ty } => self
                .get_instantiations(*ty)
                .map(|ty| ty.iter().map(|&ty| self.add_ty(TypeEnum::TList { ty })).collect_vec()),
            TypeEnum::TVirtual { ty } => self.get_instantiations(*ty).map(|ty| {
                ty.iter().map(|&ty| self.add_ty(TypeEnum::TVirtual { ty })).collect_vec()
            }),
            TypeEnum::TTuple { ty } => {
                let tuples = ty
                    .iter()
                    .map(|ty| self.get_instantiations(*ty).unwrap_or_else(|| vec![*ty]))
                    .multi_cartesian_product()
                    .collect_vec();
                if tuples.len() == 1 {
                    None
                } else {
                    Some(
                        tuples.into_iter().map(|ty| self.add_ty(TypeEnum::TTuple { ty })).collect(),
                    )
                }
            }
            TypeEnum::TObj { params, .. } => {
                let (keys, params): (Vec<TypeVarId>, Vec<Type>) = params.iter().unzip();
                let params = params
                    .into_iter()
                    .map(|ty| self.get_instantiations(ty).unwrap_or_else(|| vec![ty]))
                    .multi_cartesian_product()
                    .collect_vec();
                if params.len() <= 1 {
                    None
                } else {
                    Some(
                        params
                            .into_iter()
                            .map(|params| {
                                self.subst(
                                    ty,
                                    &zip(keys.iter().copied(), params.iter().copied()).collect(),
                                )
                                .unwrap_or(ty)
                            })
                            .collect(),
                    )
                }
            }
            _ => None,
        }
    }

    pub fn is_concrete(&mut self, a: Type, allowed_typevars: &[Type]) -> bool {
        use TypeEnum::*;
        match &*self.get_ty(a) {
            TRigidVar { .. }
            | TLiteral { .. }
            // functions are instantiated for each call sites, so the function type can contain
            // type variables.
            | TFunc { .. } => true,

            TVar { .. } => allowed_typevars.iter().any(|b| self.unification_table.unioned(a, *b)),
            TCall { .. } => false,
            TList { ty }
            | TVirtual { ty } => self.is_concrete(*ty, allowed_typevars),

            TTuple { ty } => ty.iter().all(|ty| self.is_concrete(*ty, allowed_typevars)),
            TObj { params: vars, .. } => {
                vars.values().all(|ty| self.is_concrete(*ty, allowed_typevars))
            }
        }
    }

    fn restore_snapshot(&mut self) {
        if let Some(snapshot) = self.snapshot.take() {
            self.unification_table.restore_snapshot(snapshot);
        }
    }

    fn discard_snapshot(&mut self, snapshot: (usize, u32)) {
        if self.snapshot == Some(snapshot) {
            self.unification_table.discard_snapshot(snapshot);
            self.snapshot = None;
        }
    }

    pub fn unify_call(
        &mut self,
        call: &Call,
        b: Type,
        signature: &FunSignature,
    ) -> Result<(), TypeError> {
        /*
            NOTE: scenarios to consider:

            ```python
            def func1(x: int32, y: int32, z: int32 = 5): pass

            # Normal scenarios
            func1(23, 45) # OK, z has default
            func1(23, 45, 67) # OK, z's default is overwritten
            func1(x = 23, y = 45) # OK, user is using kwargs to set positional args
            func1(y = 45, x = 23) # OK, kwargs order doesn't matter

            # Error scenarios
            func1() # ERROR: Missing arguments: x, y
            func1(23) # ERROR: Missing arguments: y
            func1(z = 23) # ERROR: Missing arguments: x, y
            func1(x = 23) # ERROR: Missing arguments: y
            func1(23, 45, x = 5) # ERROR: Got multiple values for x
            func1(23, 45, x = 5, y = 6) # ERROR: Got multiple values for x (y too but Python does not report it)
            func1(23, 45, 67, z = 89) # ERROR: Got multiple values for z
            func1(23, 45, 67, 89) # ERROR: Function only takes from 2 to 3 positional arguments but 4 were given.
            func1(23, 45, 67, w = 3) # ERROR: Got an unexpected keyword argument 'w'

            # Error scenarios that do not need to be handled here.
            func1(23, 45, z = 67, z = 89) # ERROR: Keyword argument repeated: z, the parser panics on this.
            ```
        */

        struct ParamInfo<'a> {
            /// Has this parameter been supplied with an argument already?
            has_been_supplied: bool,
            /// The corresponding [`FuncArg`] instance of this parameter (for fast table lookups)
            param: &'a FuncArg,
        }

        let snapshot = self.unification_table.get_snapshot();
        if self.snapshot.is_none() {
            self.snapshot = Some(snapshot);
        }

        // Get details about the function signature/parameters.
        let num_params = signature.args.len();

        // Force the type vars in `b` and `signature' to be up-to-date.
        let b = self.instantiate_fun(b, signature);
        let TypeEnum::TFunc(signature) = &*self.get_ty(b) else { unreachable!() };

        // Get details about the input arguments
        let Call { posargs, kwargs, ret, fun, loc } = call;
        let num_args = posargs.len() + kwargs.len();

        // Now we check the arguments against the parameters

        // Helper lambdas
        let mut type_check_arg = |param_name, expected_arg_ty, arg_ty| {
            let ok = self.unify_impl(expected_arg_ty, arg_ty, false).is_ok();
            if ok {
                Ok(())
            } else {
                // Typecheck failed, throw an error.
                self.restore_snapshot();
                Err(TypeError::new(
                    TypeErrorKind::IncorrectArgType {
                        name: param_name,
                        expected: expected_arg_ty,
                        got: arg_ty,
                    },
                    *loc,
                ))
            }
        };

        // Check for "too many arguments"
        if num_params < posargs.len() {
            let expected_min_count =
                signature.args.iter().filter(|param| param.is_required()).count();
            let expected_max_count = num_params;

            self.restore_snapshot();
            return Err(TypeError::new(
                TypeErrorKind::TooManyArguments {
                    expected_min_count,
                    expected_max_count,
                    got_count: num_args,
                },
                *loc,
            ));
        }

        // NOTE: order of `param_info_by_name` is leveraged, so use an IndexMap
        let mut param_info_by_name: IndexMap<StrRef, ParamInfo> = signature
            .args
            .iter()
            .map(|arg| (arg.name, ParamInfo { has_been_supplied: false, param: arg }))
            .collect();

        // Now consume all positional arguments and typecheck them.
        for (&arg_ty, param) in zip(posargs, signature.args.iter()) {
            // We will also use this opportunity to mark the corresponding `param_info` as having been supplied.
            let param_info = param_info_by_name.get_mut(&param.name).unwrap();
            param_info.has_been_supplied = true;

            // Typecheck
            type_check_arg(param.name, param.ty, arg_ty)?;
        }

        // Now consume all keyword arguments and typecheck them.
        for (&param_name, &arg_ty) in kwargs {
            // We will also use this opportunity to check if this keyword argument is "legal".

            let Some(param_info) = param_info_by_name.get_mut(&param_name) else {
                self.restore_snapshot();
                return Err(TypeError::new(TypeErrorKind::UnknownArgName(param_name), *loc));
            };

            if param_info.has_been_supplied {
                // NOTE: Duplicate keyword argument (i.e., `hello(1, 2, 3, arg = 4, arg = 5)`)
                // is IMPOSSIBLE as the parser would have already failed.
                // We only have to care about "got multiple values for XYZ"

                self.restore_snapshot();
                return Err(TypeError::new(
                    TypeErrorKind::GotMultipleValues { name: param_name },
                    *loc,
                ));
            }

            param_info.has_been_supplied = true;

            // Typecheck
            type_check_arg(param_name, param_info.param.ty, arg_ty)?;
        }

        // After checking posargs and kwargs, check if there are any
        // unsupplied required parameters, and throw an error if they exist.
        let missing_arg_names = param_info_by_name
            .values()
            .filter(|param_info| param_info.param.is_required() && !param_info.has_been_supplied)
            .map(|param_info| param_info.param.name)
            .collect_vec();
        if !missing_arg_names.is_empty() {
            self.restore_snapshot();
            return Err(TypeError::new(TypeErrorKind::MissingArgs { missing_arg_names }, *loc));
        }

        // Finally, check the Call's return type
        self.unify_impl(*ret, signature.ret, false).map_err(|mut err| {
            self.restore_snapshot();
            if err.loc.is_none() {
                err.loc = *loc;
            }
            err
        })?;

        *fun.borrow_mut() = Some(b);

        self.discard_snapshot(snapshot);
        Ok(())
    }

    pub fn unify(&mut self, a: Type, b: Type) -> Result<(), TypeError> {
        let snapshot = self.unification_table.get_snapshot();
        if self.snapshot.is_none() {
            self.snapshot = Some(snapshot);
        }
        self.unify_cache.clear();
        if self.unification_table.unioned(a, b) {
            self.discard_snapshot(snapshot);
            Ok(())
        } else {
            let result = self.unify_impl(a, b, false);
            if result.is_err() {
                self.restore_snapshot();
            }
            self.discard_snapshot(snapshot);
            result
        }
    }

    fn unify_impl(&mut self, a: Type, b: Type, swapped: bool) -> Result<(), TypeError> {
        use TypeEnum::*;

        if !swapped {
            let rep_a = self.unification_table.get_representative(a);
            let rep_b = self.unification_table.get_representative(b);
            if rep_a == rep_b || self.unify_cache.contains(&(rep_a, rep_b)) {
                return Ok(());
            }
            self.unify_cache.insert((rep_a, rep_b));
        }

        let (ty_a, ty_b) = {
            (
                self.unification_table.probe_value(a).clone(),
                self.unification_table.probe_value(b).clone(),
            )
        };
        match (&*ty_a, &*ty_b) {
            (
                TVar {
                    fields: fields1, id, name: name1, loc: loc1, is_const_generic: false, ..
                },
                TVar {
                    fields: fields2,
                    id: id2,
                    name: name2,
                    loc: loc2,
                    is_const_generic: false,
                    ..
                },
            ) => {
                let new_fields = match (fields1, fields2) {
                    (None, None) => None,
                    (None, Some(fields)) => Some(fields.clone()),
                    (_, None) => {
                        return self.unify_impl(b, a, true);
                    }
                    (Some(fields1), Some(fields2)) => {
                        let mut new_fields: Mapping<_, _> = fields2.clone();
                        for (key, val1) in fields1 {
                            if let Some(val2) = fields2.get(key) {
                                self.unify_impl(val1.ty, val2.ty, false).map_err(|_| {
                                    TypeError::new(
                                        TypeErrorKind::FieldUnificationError {
                                            field: *key,
                                            types: (val1.ty, val2.ty),
                                            loc: (*loc1, *loc2),
                                        },
                                        None,
                                    )
                                })?;
                                new_fields.insert(
                                    *key,
                                    RecordField::new(
                                        val1.ty,
                                        val1.mutable || val2.mutable,
                                        val1.loc.or(val2.loc),
                                    ),
                                );
                            } else {
                                new_fields.insert(*key, *val1);
                            }
                        }
                        Some(new_fields)
                    }
                };
                let intersection = self
                    .get_intersection(a, b)
                    .map_err(|()| TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None))?
                    .unwrap();
                let range = if let TVar { range, .. } = &*self.get_ty(intersection) {
                    range.clone()
                } else {
                    unreachable!()
                };
                self.unification_table.unify(a, b);
                self.unification_table.set_value(
                    a,
                    Rc::new(TVar {
                        id: name1.map_or(*id2, |_| *id),
                        fields: new_fields,
                        range,
                        name: name1.or(*name2),
                        loc: loc1.or(*loc2),
                        is_const_generic: false,
                    }),
                );
            }
            (TVar { fields: None, range, is_const_generic: false, .. }, _) => {
                // We check for the range of the type variable to see if unification is allowed.
                // Note that although b may be compatible with a, we may have to constrain type
                // variables in b to make sure that instantiations of b would always be compatible
                // with a.
                // The return value x of check_var_compatibility would be a new type that is
                // guaranteed to be compatible with a under all possible instantiations. So we
                // unify x with b to recursively apply the constrains, and then set a to x.
                let x = self
                    .check_var_compatibility(b, range)
                    .map_err(|_| {
                        TypeError::new(TypeErrorKind::IncompatibleRange(b, range.clone()), None)
                    })?
                    .unwrap_or(b);
                self.unify_impl(x, b, false)?;
                self.set_a_to_b(a, x);
            }
            (TVar { fields: Some(fields), range, is_const_generic: false, .. }, TTuple { ty }) => {
                let len = i32::try_from(ty.len()).unwrap();
                for (k, v) in fields {
                    match *k {
                        RecordKey::Int(i) => {
                            if v.mutable {
                                return Err(TypeError::new(
                                    TypeErrorKind::MutationError(*k, b),
                                    v.loc,
                                ));
                            }
                            let ind = if i < 0 { len + i } else { i };
                            if ind >= len || ind < 0 {
                                return Err(TypeError::new(
                                    TypeErrorKind::TupleIndexOutOfBounds { index: i, len },
                                    v.loc,
                                ));
                            }
                            self.unify_impl(v.ty, ty[ind as usize], false)
                                .map_err(|e| e.at(v.loc))?;
                        }
                        RecordKey::Str(_) => {
                            return Err(TypeError::new(TypeErrorKind::NoSuchField(*k, b), v.loc))
                        }
                    }
                }
                let x = self.check_var_compatibility(b, range)?.unwrap_or(b);
                self.unify_impl(x, b, false)?;
                self.set_a_to_b(a, x);
            }
            (TVar { fields: Some(fields), range, is_const_generic: false, .. }, TList { ty }) => {
                for (k, v) in fields {
                    match *k {
                        RecordKey::Int(_) => {
                            self.unify_impl(v.ty, *ty, false).map_err(|e| e.at(v.loc))?;
                        }
                        RecordKey::Str(_) => {
                            return Err(TypeError::new(TypeErrorKind::NoSuchField(*k, b), v.loc))
                        }
                    }
                }
                let x = self.check_var_compatibility(b, range)?.unwrap_or(b);
                self.unify_impl(x, b, false)?;
                self.set_a_to_b(a, x);
            }

            (
                TVar { id: id1, range: ty1, is_const_generic: true, .. },
                TVar { id: id2, range: ty2, .. },
            ) => {
                let ty1 = ty1[0];
                let ty2 = ty2[0];

                if id1 != id2 {
                    self.unify_impl(ty1, ty2, false)?;
                }

                self.set_a_to_b(a, b);
            }

            (TVar { range: tys, is_const_generic: true, .. }, TLiteral { values, .. }) => {
                assert_eq!(tys.len(), 1);
                assert_eq!(values.len(), 1);

                let primitives =
                    &self.primitive_store.expect("Expected PrimitiveStore to be present");

                let ty = tys[0];
                let value = &values[0];
                let value_ty = value.get_type(primitives, self);

                // If the types don't match, try to implicitly promote integers
                if !self.unioned(ty, value_ty) {
                    let Ok(num_val) = i128::try_from(value.clone()) else {
                        return Self::incompatible_types(a, b);
                    };

                    let can_convert = if self.unioned(ty, primitives.int32) {
                        i32::try_from(num_val).is_ok()
                    } else if self.unioned(ty, primitives.int64) {
                        i64::try_from(num_val).is_ok()
                    } else if self.unioned(ty, primitives.uint32) {
                        u32::try_from(num_val).is_ok()
                    } else if self.unioned(ty, primitives.uint64) {
                        u64::try_from(num_val).is_ok()
                    } else {
                        false
                    };

                    if !can_convert {
                        return Self::incompatible_types(a, b);
                    }
                }

                self.set_a_to_b(a, b);
            }

            (TLiteral { values: val1, .. }, TLiteral { values: val2, .. }) => {
                for (v1, v2) in zip(val1, val2) {
                    if v1 != v2 {
                        // Try performing integer promotion on literals
                        let v1i = i128::try_from(v1.clone()).ok();
                        let v2i = i128::try_from(v2.clone()).ok();

                        if v1i != v2i {
                            return Self::incompatible_types(a, b);
                        }
                    }
                }

                self.set_a_to_b(a, b);
            }

            (TTuple { ty: ty1 }, TTuple { ty: ty2 }) => {
                if ty1.len() != ty2.len() {
                    return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                }
                for (x, y) in ty1.iter().zip(ty2.iter()) {
                    if self.unify_impl(*x, *y, false).is_err() {
                        return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                    }
                }
                self.set_a_to_b(a, b);
            }
            (TList { ty: ty1 }, TList { ty: ty2 }) => {
                if self.unify_impl(*ty1, *ty2, false).is_err() {
                    return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                }
                self.set_a_to_b(a, b);
            }
            (TVar { fields: Some(map), range, .. }, TObj { fields, .. }) => {
                for (k, field) in map {
                    match *k {
                        RecordKey::Str(s) => {
                            let (ty, mutable) = fields.get(&s).copied().ok_or_else(|| {
                                TypeError::new(TypeErrorKind::NoSuchField(*k, b), field.loc)
                            })?;
                            // typevar represents the usage of the variable
                            // it is OK to have immutable usage for mutable fields
                            // but cannot have mutable usage for immutable fields
                            if field.mutable && !mutable {
                                return Err(TypeError::new(
                                    TypeErrorKind::MutationError(*k, b),
                                    field.loc,
                                ));
                            }
                            self.unify_impl(field.ty, ty, false).map_err(|v| v.at(field.loc))?;
                        }
                        RecordKey::Int(_) => {
                            return Err(TypeError::new(
                                TypeErrorKind::NoSuchField(*k, b),
                                field.loc,
                            ))
                        }
                    }
                }
                let x = self.check_var_compatibility(b, range)?.unwrap_or(b);
                self.unify_impl(x, b, false)?;
                self.set_a_to_b(a, x);
            }
            (TVar { fields: Some(map), range, .. }, TVirtual { ty }) => {
                let ty = self.get_ty(*ty);
                if let TObj { fields, .. } = ty.as_ref() {
                    for (k, field) in map {
                        match *k {
                            RecordKey::Str(s) => {
                                let (ty, _) = fields.get(&s).copied().ok_or_else(|| {
                                    TypeError::new(TypeErrorKind::NoSuchField(*k, b), field.loc)
                                })?;
                                if !matches!(self.get_ty(ty).as_ref(), TFunc { .. }) {
                                    return Err(TypeError::new(
                                        TypeErrorKind::NoSuchField(*k, b),
                                        field.loc,
                                    ));
                                }
                                if field.mutable {
                                    return Err(TypeError::new(
                                        TypeErrorKind::MutationError(*k, b),
                                        field.loc,
                                    ));
                                }
                                self.unify_impl(field.ty, ty, false)
                                    .map_err(|v| v.at(field.loc))?;
                            }
                            RecordKey::Int(_) => {
                                return Err(TypeError::new(
                                    TypeErrorKind::NoSuchField(*k, b),
                                    field.loc,
                                ))
                            }
                        }
                    }
                } else {
                    // require annotation...
                    return Err(TypeError::new(TypeErrorKind::RequiresTypeAnn, None));
                }
                let x = self.check_var_compatibility(b, range)?.unwrap_or(b);
                self.unify_impl(x, b, false)?;
                self.set_a_to_b(a, x);
            }
            (
                TObj { obj_id: id1, params: params1, .. },
                TObj { obj_id: id2, params: params2, .. },
            ) => {
                if id1 != id2 {
                    Self::incompatible_types(a, b)?;
                }

                // Sort the type arguments by its UnificationKey first, since `HashMap::iter` visits
                // all K-V pairs "in arbitrary order"
                let (tv1, tv2) = (
                    params1.iter().map(|(_, v)| v).collect_vec(),
                    params2.iter().map(|(_, v)| v).collect_vec(),
                );
                for (x, y) in zip(tv1, tv2) {
                    if self.unify_impl(*x, *y, false).is_err() {
                        return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                    };
                }
                self.set_a_to_b(a, b);
            }
            (TVirtual { ty: ty1 }, TVirtual { ty: ty2 }) => {
                if self.unify_impl(*ty1, *ty2, false).is_err() {
                    return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                };
                self.set_a_to_b(a, b);
            }
            (TCall(calls1), TCall(calls2)) => {
                // we do not unify individual calls, instead we defer until the unification wtih a
                // function definition.
                let calls = calls1.iter().chain(calls2.iter()).copied().collect();
                self.set_a_to_b(a, b);
                self.unification_table.set_value(b, Rc::new(TCall(calls)));
            }
            (TCall(calls), TFunc(signature)) => {
                // we unify every calls to the function signature.
                for c in calls {
                    let call = self.calls[c.0].clone();
                    self.unify_call(&call, b, signature)?;
                }
                self.set_a_to_b(a, b);
            }
            (TFunc(sign1), TFunc(sign2)) => {
                if !sign1.vars.is_empty() || !sign2.vars.is_empty() {
                    return Err(TypeError::new(TypeErrorKind::PolymorphicFunctionPointer, None));
                }
                if sign1.args.len() != sign2.args.len() {
                    return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                }
                for (x, y) in sign1.args.iter().zip(sign2.args.iter()) {
                    if x.name != y.name || x.default_value != y.default_value {
                        return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                    }
                    if self.unify_impl(x.ty, y.ty, false).is_err() {
                        return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                    };
                }
                if self.unify_impl(sign1.ret, sign2.ret, false).is_err() {
                    return Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None));
                };
                self.set_a_to_b(a, b);
            }
            (TVar { fields: Some(fields), .. }, _) => {
                let (k, v) = fields.iter().next().unwrap();
                return Err(TypeError::new(TypeErrorKind::NoSuchField(*k, b), v.loc));
            }
            _ => {
                if swapped {
                    return Self::incompatible_types(a, b);
                }

                self.unify_impl(b, a, true)?;
            }
        }
        Ok(())
    }

    pub fn stringify(&self, ty: Type) -> String {
        self.stringify_with_notes(ty, &mut None)
    }

    pub fn stringify_with_notes(
        &self,
        ty: Type,
        notes: &mut Option<HashMap<TypeVarId, String>>,
    ) -> String {
        let top_level = self.top_level.clone();
        self.internal_stringify(
            ty,
            &mut |id| {
                top_level.as_ref().map_or_else(
                    || format!("{id}"),
                    |top_level| {
                        let top_level_def = &top_level.definitions.read()[id];
                        let TopLevelDef::Class { name, .. } = &*top_level_def.read() else {
                            unreachable!("expected class definition")
                        };

                        name.to_string()
                    },
                )
            },
            &mut |id| format!("typevar{id}"),
            notes,
        )
    }

    /// Get string representation of the type
    pub fn internal_stringify<F, G>(
        &self,
        ty: Type,
        obj_to_name: &mut F,
        var_to_name: &mut G,
        notes: &mut Option<HashMap<TypeVarId, String>>,
    ) -> String
    where
        F: FnMut(usize) -> String,
        G: FnMut(TypeVarId) -> String,
    {
        let ty = self.unification_table.probe_value_immutable(ty).clone();
        match ty.as_ref() {
            TypeEnum::TRigidVar { id, name, .. } => {
                name.map(|v| v.to_string()).unwrap_or_else(|| var_to_name(*id))
            }
            TypeEnum::TVar { id, name, fields, range, .. } => {
                let n = if let Some(fields) = fields {
                    let mut fields = fields.iter().map(|(k, f)| {
                        format!(
                            "{}={}",
                            k,
                            self.internal_stringify(f.ty, obj_to_name, var_to_name, notes)
                        )
                    });
                    let fields = fields.join(", ");
                    format!(
                        "{}[{}]",
                        name.map(|v| v.to_string()).unwrap_or_else(|| var_to_name(*id)),
                        fields
                    )
                } else {
                    name.map(|v| v.to_string()).unwrap_or_else(|| var_to_name(*id))
                };
                if !range.is_empty() && notes.is_some() && !notes.as_ref().unwrap().contains_key(id)
                {
                    // just in case if there is any cyclic dependency
                    notes.as_mut().unwrap().insert(*id, String::new());
                    let body = format!(
                        "{} ∈ {{{}}}",
                        n,
                        range
                            .iter()
                            .map(|v| self.internal_stringify(*v, obj_to_name, var_to_name, notes))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    notes.as_mut().unwrap().insert(*id, body);
                };
                n
            }
            TypeEnum::TLiteral { values, .. } => {
                format!("const({})", values.iter().map(|v| format!("{v:?}")).join(", "))
            }
            TypeEnum::TTuple { ty } => {
                let mut fields =
                    ty.iter().map(|v| self.internal_stringify(*v, obj_to_name, var_to_name, notes));
                format!("tuple[{}]", fields.join(", "))
            }
            TypeEnum::TList { ty } => {
                format!("list[{}]", self.internal_stringify(*ty, obj_to_name, var_to_name, notes))
            }
            TypeEnum::TVirtual { ty } => {
                format!(
                    "virtual[{}]",
                    self.internal_stringify(*ty, obj_to_name, var_to_name, notes)
                )
            }
            TypeEnum::TObj { obj_id, params, .. } => {
                let name = obj_to_name(obj_id.0);
                if params.is_empty() {
                    name
                } else {
                    let mut params = params
                        .iter()
                        .map(|(_, v)| self.internal_stringify(*v, obj_to_name, var_to_name, notes));
                    format!("{}[{}]", name, params.join(", "))
                }
            }
            TypeEnum::TCall { .. } => "call".to_owned(),
            TypeEnum::TFunc(signature) => {
                let params = signature
                    .args
                    .iter()
                    .map(|arg| {
                        if let Some(dv) = &arg.default_value {
                            format!(
                                "{}:{}={}",
                                arg.name,
                                self.internal_stringify(arg.ty, obj_to_name, var_to_name, notes),
                                dv
                            )
                        } else {
                            format!(
                                "{}:{}",
                                arg.name,
                                self.internal_stringify(arg.ty, obj_to_name, var_to_name, notes)
                            )
                        }
                    })
                    .join(", ");
                let ret = self.internal_stringify(signature.ret, obj_to_name, var_to_name, notes);
                format!("fn[[{params}], {ret}]")
            }
        }
    }

    /// Unifies `a` and `b` together, and set the value to the value of `b`.
    fn set_a_to_b(&mut self, a: Type, b: Type) {
        let table = &mut self.unification_table;
        let ty_b = table.probe_value(b).clone();
        table.unify(a, b);
        table.set_value(a, ty_b);
    }

    fn incompatible_types(a: Type, b: Type) -> Result<(), TypeError> {
        Err(TypeError::new(TypeErrorKind::IncompatibleTypes(a, b), None))
    }

    /// Instantiate a function if it hasn't been instantiated.
    /// Returns Some(T) where T is the instantiated type.
    /// Returns None if the function is already instantiated.
    fn instantiate_fun(&mut self, ty: Type, fun: &FunSignature) -> Type {
        let mut instantiated = true;
        let mut vars = Vec::new();
        for (k, v) in &fun.vars {
            if let TypeEnum::TVar { id, name, loc, range, .. } =
                self.unification_table.probe_value(*v).as_ref()
            {
                // for class methods that contain type vars not in class declaration,
                // as long as there exits one uninstantiated type var, the function is not instantiated,
                // and need to do substitution on those type vars
                if k == id {
                    instantiated = false;
                    vars.push((*k, range.clone(), *name, *loc));
                }
            }
        }
        if instantiated {
            ty
        } else {
            let mapping = vars
                .into_iter()
                .map(|(k, range, name, loc)| {
                    (k, self.get_fresh_var_with_range(range.as_ref(), name, loc).ty)
                })
                .collect();
            self.subst(ty, &mapping).unwrap_or(ty)
        }
    }

    /// Substitute type variables within a type into other types.
    /// If this returns Some(T), T would be the substituted type.
    /// If this returns None, the result type would be the original type
    /// (no substitution has to be done).
    pub fn subst(&mut self, a: Type, mapping: &VarMap) -> Option<Type> {
        self.subst_impl(a, mapping, &mut HashMap::new())
    }

    fn subst_impl(
        &mut self,
        a: Type,
        mapping: &VarMap,
        cache: &mut HashMap<Type, Option<Type>>,
    ) -> Option<Type> {
        let cached = cache.get_mut(&a);
        if let Some(cached) = cached {
            if cached.is_none() {
                *cached = Some(self.get_fresh_var(None, None).ty);
            }
            return *cached;
        }

        let ty = self.unification_table.probe_value(a).clone();
        // this function would only be called when we instantiate functions.
        // function type signature should ONLY contain concrete types and type
        // variables, i.e. things like TRecord, TCall should not occur, and we
        // should be safe to not implement the substitution for those variants.
        match &*ty {
            TypeEnum::TRigidVar { .. } | TypeEnum::TLiteral { .. } => None,
            TypeEnum::TVar { id, .. } => mapping.get(id).copied(),
            TypeEnum::TTuple { ty } => {
                let mut new_ty = Cow::from(ty);
                for (i, t) in ty.iter().enumerate() {
                    if let Some(t1) = self.subst_impl(*t, mapping, cache) {
                        new_ty.to_mut()[i] = t1;
                    }
                }
                if matches!(new_ty, Cow::Owned(_)) {
                    Some(self.add_ty(TypeEnum::TTuple { ty: new_ty.into_owned() }))
                } else {
                    None
                }
            }
            TypeEnum::TList { ty } => {
                self.subst_impl(*ty, mapping, cache).map(|t| self.add_ty(TypeEnum::TList { ty: t }))
            }
            TypeEnum::TVirtual { ty } => self
                .subst_impl(*ty, mapping, cache)
                .map(|t| self.add_ty(TypeEnum::TVirtual { ty: t })),
            TypeEnum::TObj { obj_id, fields, params } => {
                // Type variables in field types must be present in the type parameter.
                // If the mapping does not contain any type variables in the
                // parameter list, we don't need to substitute the fields.
                // This is also used to prevent infinite substitution...
                let need_subst = params.values().any(|v| {
                    let ty = self.unification_table.probe_value(*v);
                    if let TypeEnum::TVar { id, .. } = ty.as_ref() {
                        mapping.contains_key(id)
                    } else {
                        false
                    }
                });
                if need_subst {
                    cache.insert(a, None);
                    let obj_id = *obj_id;
                    let params =
                        self.subst_map(params, mapping, cache).unwrap_or_else(|| params.clone());
                    let fields =
                        self.subst_map2(fields, mapping, cache).unwrap_or_else(|| fields.clone());
                    let new_ty = self.add_ty(TypeEnum::TObj { obj_id, params, fields });
                    if let Some(var) = cache.get(&a).unwrap() {
                        self.unify_impl(new_ty, *var, false).unwrap();
                    }
                    Some(new_ty)
                } else {
                    None
                }
            }
            TypeEnum::TFunc(FunSignature { args, ret, vars: params }) => {
                let new_params = self.subst_map(params, mapping, cache);
                let new_ret = self.subst_impl(*ret, mapping, cache);
                let mut new_args = Cow::from(args);
                for (i, t) in args.iter().enumerate() {
                    if let Some(t1) = self.subst_impl(t.ty, mapping, cache) {
                        let mut t = t.clone();
                        t.ty = t1;
                        new_args.to_mut()[i] = t;
                    }
                }
                if new_params.is_some() || new_ret.is_some() || matches!(new_args, Cow::Owned(..)) {
                    let params = new_params.unwrap_or_else(|| params.clone());
                    let ret = new_ret.unwrap_or(*ret);
                    let args = new_args.into_owned();
                    Some(self.add_ty(TypeEnum::TFunc(FunSignature { args, ret, vars: params })))
                } else {
                    None
                }
            }
            TypeEnum::TCall(_) => {
                unreachable!("{} not expected", ty.get_type_name())
            }
        }
    }

    fn subst_map<K>(
        &mut self,
        map: &IndexMapping<K>,
        mapping: &VarMap,
        cache: &mut HashMap<Type, Option<Type>>,
    ) -> Option<IndexMapping<K>>
    where
        K: std::hash::Hash + Eq + Clone,
    {
        let mut map2 = None;
        for (k, v) in map {
            if let Some(v1) = self.subst_impl(*v, mapping, cache) {
                if map2.is_none() {
                    map2 = Some(map.clone());
                }
                *map2.as_mut().unwrap().get_mut(k).unwrap() = v1;
            }
        }
        map2
    }

    fn subst_map2<K>(
        &mut self,
        map: &Mapping<K, (Type, bool)>,
        mapping: &VarMap,
        cache: &mut HashMap<Type, Option<Type>>,
    ) -> Option<Mapping<K, (Type, bool)>>
    where
        K: std::hash::Hash + Eq + Clone,
    {
        let mut map2 = None;
        for (k, (v, mutability)) in map {
            if let Some(v1) = self.subst_impl(*v, mapping, cache) {
                if map2.is_none() {
                    map2 = Some(map.clone());
                }
                *map2.as_mut().unwrap().get_mut(k).unwrap() = (v1, *mutability);
            }
        }
        map2
    }

    fn get_intersection(&mut self, a: Type, b: Type) -> Result<Option<Type>, ()> {
        use TypeEnum::*;
        let x = self.get_ty(a);
        let y = self.get_ty(b);
        match (x.as_ref(), y.as_ref()) {
            (
                TVar { range: range1, name, loc, .. },
                TVar { fields, range: range2, name: name2, loc: loc2, .. },
            ) => {
                // new range is the intersection of them
                // empty range indicates no constraint
                if range1.is_empty() {
                    Ok(Some(b))
                } else if range2.is_empty() {
                    Ok(Some(a))
                } else {
                    let range = range2
                        .iter()
                        .cartesian_product(range1.iter())
                        .filter_map(|(v1, v2)| {
                            self.get_intersection(*v1, *v2).map(|v| v.unwrap_or(*v1)).ok()
                        })
                        .collect_vec();
                    if range.is_empty() {
                        Err(())
                    } else {
                        let id = self.generate_var_id();
                        let ty = TVar {
                            id,
                            fields: fields.clone(),
                            range,
                            name: name2.or(*name),
                            loc: loc2.or(*loc),
                            is_const_generic: false,
                        };
                        Ok(Some(self.unification_table.new_key(ty.into())))
                    }
                }
            }
            (_, TVar { range, .. }) => {
                // range should be restricted to the left hand side
                if range.is_empty() {
                    Ok(Some(a))
                } else {
                    for v in range {
                        let result = self.get_intersection(a, *v);
                        if let Ok(result) = result {
                            return Ok(result.or(Some(a)));
                        }
                    }
                    Err(())
                }
            }
            (TVar { range, .. }, _) => self.check_var_compatibility(b, range).or(Err(())),
            (TTuple { ty: ty1 }, TTuple { ty: ty2 }) if ty1.len() == ty2.len() => {
                let ty: Vec<_> = zip(ty1.iter(), ty2.iter())
                    .map(|(a, b)| self.get_intersection(*a, *b))
                    .try_collect()?;
                if ty.iter().any(Option::is_some) {
                    Ok(Some(self.add_ty(TTuple {
                        ty: zip(ty, ty1.iter()).map(|(a, b)| a.unwrap_or(*b)).collect(),
                    })))
                } else {
                    Ok(None)
                }
            }
            (TList { ty: ty1 }, TList { ty: ty2 }) => {
                Ok(self.get_intersection(*ty1, *ty2)?.map(|ty| self.add_ty(TList { ty })))
            }
            (TVirtual { ty: ty1 }, TVirtual { ty: ty2 }) => {
                Ok(self.get_intersection(*ty1, *ty2)?.map(|ty| self.add_ty(TVirtual { ty })))
            }
            (TObj { obj_id: id1, .. }, TObj { obj_id: id2, .. }) if id1 == id2 => Ok(None),
            // don't deal with function shape for now
            _ => Err(()),
        }
    }

    fn check_var_compatibility(
        &mut self,
        b: Type,
        range: &[Type],
    ) -> Result<Option<Type>, TypeError> {
        if range.is_empty() {
            return Ok(None);
        }
        for t in range {
            let result = self.get_intersection(*t, b);
            if let Ok(result) = result {
                return Ok(result);
            }
        }
        Err(TypeError::new(TypeErrorKind::IncompatibleRange(b, range.to_vec()), None))
    }

    /// Generate a new [`TypeVarId`] from [`Unifier::var_id_counter`]
    fn generate_var_id(&mut self) -> TypeVarId {
        self.var_id_counter += 1;
        TypeVarId(self.var_id_counter)
    }
}
