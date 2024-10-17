use std::convert::TryInto;

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use nac3parser::ast::{Constant, ExprKind, Location};

use super::{numpy::unpack_ndarray_var_tys, *};
use crate::{
    symbol_resolver::SymbolValue,
    typecheck::typedef::{into_var_map, iter_type_vars, Mapping, TypeVarId, VarMap},
};

/// All primitive types and functions in nac3core.
#[derive(Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum PrimDef {
    // Classes
    Int32,
    Int64,
    Float,
    Bool,
    None,
    Range,
    Str,
    Exception,
    UInt32,
    UInt64,
    Option,
    List,
    NDArray,

    // Option methods
    FunOptionIsSome,
    FunOptionIsNone,
    FunOptionUnwrap,

    // Option-related functions
    FunSome,

    // NDArray methods
    FunNDArrayCopy,
    FunNDArrayFill,

    // Range methods
    FunRangeInit,

    // NumPy factory functions
    FunNpNDArray,
    FunNpEmpty,
    FunNpZeros,
    FunNpOnes,
    FunNpFull,
    FunNpArray,
    FunNpEye,
    FunNpIdentity,

    // Miscellaneous NumPy & SciPy functions
    FunNpRound,
    FunNpFloor,
    FunNpCeil,
    FunNpMin,
    FunNpMinimum,
    FunNpArgmin,
    FunNpMax,
    FunNpMaximum,
    FunNpArgmax,
    FunNpIsNan,
    FunNpIsInf,
    FunNpSin,
    FunNpCos,
    FunNpExp,
    FunNpExp2,
    FunNpLog,
    FunNpLog10,
    FunNpLog2,
    FunNpFabs,
    FunNpSqrt,
    FunNpRint,
    FunNpTan,
    FunNpArcsin,
    FunNpArccos,
    FunNpArctan,
    FunNpSinh,
    FunNpCosh,
    FunNpTanh,
    FunNpArcsinh,
    FunNpArccosh,
    FunNpArctanh,
    FunNpExpm1,
    FunNpCbrt,
    FunSpSpecErf,
    FunSpSpecErfc,
    FunSpSpecGamma,
    FunSpSpecGammaln,
    FunSpSpecJ0,
    FunSpSpecJ1,
    FunNpArctan2,
    FunNpCopysign,
    FunNpFmax,
    FunNpFmin,
    FunNpLdExp,
    FunNpHypot,
    FunNpNextAfter,
    FunNpTranspose,
    FunNpReshape,

    // Linalg functions
    FunNpDot,
    FunNpLinalgCholesky,
    FunNpLinalgQr,
    FunNpLinalgSvd,
    FunNpLinalgInv,
    FunNpLinalgPinv,
    FunNpLinalgMatrixPower,
    FunNpLinalgDet,
    FunSpLinalgLu,
    FunSpLinalgSchur,
    FunSpLinalgHessenberg,

    // Miscellaneous Python & NAC3 functions
    FunInt32,
    FunInt64,
    FunUInt32,
    FunUInt64,
    FunFloat,
    FunRound,
    FunRound64,
    FunStr,
    FunBool,
    FunFloor,
    FunFloor64,
    FunCeil,
    FunCeil64,
    FunLen,
    FunMin,
    FunMax,
    FunAbs,
}

/// Associated details of a [`PrimDef`]
pub enum PrimDefDetails {
    PrimFunction { name: &'static str, simple_name: &'static str },
    PrimClass { name: &'static str, get_ty_fn: fn(&PrimitiveStore) -> Type },
}

impl PrimDef {
    /// Get the assigned [`DefinitionId`] of this [`PrimDef`].
    ///
    /// The assigned definition ID is defined by the position this [`PrimDef`] enum unit variant is defined at,
    /// with the first `PrimDef`'s definition id being `0`.
    #[must_use]
    pub fn id(&self) -> DefinitionId {
        DefinitionId(*self as usize)
    }

    /// Check if a definition ID is that of a [`PrimDef`].
    #[must_use]
    pub fn contains_id(id: DefinitionId) -> bool {
        Self::iter().any(|prim| prim.id() == id)
    }

    /// Get the definition "simple name" of this [`PrimDef`].
    ///
    /// If the [`PrimDef`] is a function, this corresponds to [`TopLevelDef::Function::simple_name`].
    ///
    /// If the [`PrimDef`] is a class, this returns [`None`].
    #[must_use]
    pub fn simple_name(&self) -> &'static str {
        match self.details() {
            PrimDefDetails::PrimFunction { simple_name, .. } => simple_name,
            PrimDefDetails::PrimClass { .. } => {
                panic!("PrimDef {self:?} has no simple_name as it is not a function.")
            }
        }
    }

    /// Get the definition "name" of this [`PrimDef`].
    ///
    /// If the [`PrimDef`] is a function, this corresponds to [`TopLevelDef::Function::name`].
    ///
    /// If the [`PrimDef`] is a class, this corresponds to [`TopLevelDef::Class::name`].
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.details() {
            PrimDefDetails::PrimFunction { name, .. } | PrimDefDetails::PrimClass { name, .. } => {
                name
            }
        }
    }

    /// Get the associated details of this [`PrimDef`]
    #[must_use]
    pub fn details(self) -> PrimDefDetails {
        fn class(name: &'static str, get_ty_fn: fn(&PrimitiveStore) -> Type) -> PrimDefDetails {
            PrimDefDetails::PrimClass { name, get_ty_fn }
        }

        fn fun(name: &'static str, simple_name: Option<&'static str>) -> PrimDefDetails {
            PrimDefDetails::PrimFunction { simple_name: simple_name.unwrap_or(name), name }
        }

        match self {
            // Classes
            PrimDef::Int32 => class("int32", |primitives| primitives.int32),
            PrimDef::Int64 => class("int64", |primitives| primitives.int64),
            PrimDef::Float => class("float", |primitives| primitives.float),
            PrimDef::Bool => class("bool", |primitives| primitives.bool),
            PrimDef::None => class("none", |primitives| primitives.none),
            PrimDef::Range => class("range", |primitives| primitives.range),
            PrimDef::Str => class("str", |primitives| primitives.str),
            PrimDef::Exception => class("Exception", |primitives| primitives.exception),
            PrimDef::UInt32 => class("uint32", |primitives| primitives.uint32),
            PrimDef::UInt64 => class("uint64", |primitives| primitives.uint64),
            PrimDef::Option => class("Option", |primitives| primitives.option),
            PrimDef::List => class("list", |primitives| primitives.list),
            PrimDef::NDArray => class("ndarray", |primitives| primitives.ndarray),

            // Option methods
            PrimDef::FunOptionIsSome => fun("Option.is_some", Some("is_some")),
            PrimDef::FunOptionIsNone => fun("Option.is_none", Some("is_none")),
            PrimDef::FunOptionUnwrap => fun("Option.unwrap", Some("unwrap")),

            // Option-related functions
            PrimDef::FunSome => fun("Some", None),

            // NDArray methods
            PrimDef::FunNDArrayCopy => fun("ndarray.copy", Some("copy")),
            PrimDef::FunNDArrayFill => fun("ndarray.fill", Some("fill")),

            // Range methods
            PrimDef::FunRangeInit => fun("range.__init__", Some("__init__")),

            // NumPy factory functions
            PrimDef::FunNpNDArray => fun("np_ndarray", None),
            PrimDef::FunNpEmpty => fun("np_empty", None),
            PrimDef::FunNpZeros => fun("np_zeros", None),
            PrimDef::FunNpOnes => fun("np_ones", None),
            PrimDef::FunNpFull => fun("np_full", None),
            PrimDef::FunNpArray => fun("np_array", None),
            PrimDef::FunNpEye => fun("np_eye", None),
            PrimDef::FunNpIdentity => fun("np_identity", None),

            // Miscellaneous NumPy & SciPy functions
            PrimDef::FunNpRound => fun("np_round", None),
            PrimDef::FunNpFloor => fun("np_floor", None),
            PrimDef::FunNpCeil => fun("np_ceil", None),
            PrimDef::FunNpMin => fun("np_min", None),
            PrimDef::FunNpMinimum => fun("np_minimum", None),
            PrimDef::FunNpArgmin => fun("np_argmin", None),
            PrimDef::FunNpMax => fun("np_max", None),
            PrimDef::FunNpMaximum => fun("np_maximum", None),
            PrimDef::FunNpArgmax => fun("np_argmax", None),
            PrimDef::FunNpIsNan => fun("np_isnan", None),
            PrimDef::FunNpIsInf => fun("np_isinf", None),
            PrimDef::FunNpSin => fun("np_sin", None),
            PrimDef::FunNpCos => fun("np_cos", None),
            PrimDef::FunNpExp => fun("np_exp", None),
            PrimDef::FunNpExp2 => fun("np_exp2", None),
            PrimDef::FunNpLog => fun("np_log", None),
            PrimDef::FunNpLog10 => fun("np_log10", None),
            PrimDef::FunNpLog2 => fun("np_log2", None),
            PrimDef::FunNpFabs => fun("np_fabs", None),
            PrimDef::FunNpSqrt => fun("np_sqrt", None),
            PrimDef::FunNpRint => fun("np_rint", None),
            PrimDef::FunNpTan => fun("np_tan", None),
            PrimDef::FunNpArcsin => fun("np_arcsin", None),
            PrimDef::FunNpArccos => fun("np_arccos", None),
            PrimDef::FunNpArctan => fun("np_arctan", None),
            PrimDef::FunNpSinh => fun("np_sinh", None),
            PrimDef::FunNpCosh => fun("np_cosh", None),
            PrimDef::FunNpTanh => fun("np_tanh", None),
            PrimDef::FunNpArcsinh => fun("np_arcsinh", None),
            PrimDef::FunNpArccosh => fun("np_arccosh", None),
            PrimDef::FunNpArctanh => fun("np_arctanh", None),
            PrimDef::FunNpExpm1 => fun("np_expm1", None),
            PrimDef::FunNpCbrt => fun("np_cbrt", None),
            PrimDef::FunSpSpecErf => fun("sp_spec_erf", None),
            PrimDef::FunSpSpecErfc => fun("sp_spec_erfc", None),
            PrimDef::FunSpSpecGamma => fun("sp_spec_gamma", None),
            PrimDef::FunSpSpecGammaln => fun("sp_spec_gammaln", None),
            PrimDef::FunSpSpecJ0 => fun("sp_spec_j0", None),
            PrimDef::FunSpSpecJ1 => fun("sp_spec_j1", None),
            PrimDef::FunNpArctan2 => fun("np_arctan2", None),
            PrimDef::FunNpCopysign => fun("np_copysign", None),
            PrimDef::FunNpFmax => fun("np_fmax", None),
            PrimDef::FunNpFmin => fun("np_fmin", None),
            PrimDef::FunNpLdExp => fun("np_ldexp", None),
            PrimDef::FunNpHypot => fun("np_hypot", None),
            PrimDef::FunNpNextAfter => fun("np_nextafter", None),
            PrimDef::FunNpTranspose => fun("np_transpose", None),
            PrimDef::FunNpReshape => fun("np_reshape", None),

            // Linalg functions
            PrimDef::FunNpDot => fun("np_dot", None),
            PrimDef::FunNpLinalgCholesky => fun("np_linalg_cholesky", None),
            PrimDef::FunNpLinalgQr => fun("np_linalg_qr", None),
            PrimDef::FunNpLinalgSvd => fun("np_linalg_svd", None),
            PrimDef::FunNpLinalgInv => fun("np_linalg_inv", None),
            PrimDef::FunNpLinalgPinv => fun("np_linalg_pinv", None),
            PrimDef::FunNpLinalgMatrixPower => fun("np_linalg_matrix_power", None),
            PrimDef::FunNpLinalgDet => fun("np_linalg_det", None),
            PrimDef::FunSpLinalgLu => fun("sp_linalg_lu", None),
            PrimDef::FunSpLinalgSchur => fun("sp_linalg_schur", None),
            PrimDef::FunSpLinalgHessenberg => fun("sp_linalg_hessenberg", None),

            // Miscellaneous Python & NAC3 functions
            PrimDef::FunInt32 => fun("int32", None),
            PrimDef::FunInt64 => fun("int64", None),
            PrimDef::FunUInt32 => fun("uint32", None),
            PrimDef::FunUInt64 => fun("uint64", None),
            PrimDef::FunFloat => fun("float", None),
            PrimDef::FunRound => fun("round", None),
            PrimDef::FunRound64 => fun("round64", None),
            PrimDef::FunStr => fun("str", None),
            PrimDef::FunBool => fun("bool", None),
            PrimDef::FunFloor => fun("floor", None),
            PrimDef::FunFloor64 => fun("floor64", None),
            PrimDef::FunCeil => fun("ceil", None),
            PrimDef::FunCeil64 => fun("ceil64", None),
            PrimDef::FunLen => fun("len", None),
            PrimDef::FunMin => fun("min", None),
            PrimDef::FunMax => fun("max", None),
            PrimDef::FunAbs => fun("abs", None),
        }
    }
}

/// Asserts that a [`PrimDef`] is in an allowlist.
///
/// Like `debug_assert!`, this statements of this function are only
/// enabled if `cfg!(debug_assertions)` is true.
pub fn debug_assert_prim_is_allowed(prim: PrimDef, allowlist: &[PrimDef]) {
    if cfg!(debug_assertions) {
        let allowed = allowlist.iter().any(|p| *p == prim);
        assert!(
            allowed,
            "Disallowed primitive definition. Got {prim:?}, but expects it to be in {allowlist:?}"
        );
    }
}

/// Construct the fields of class `Exception`
/// See [`TypeEnum::TObj::fields`] and [`TopLevelDef::Class::fields`]
#[must_use]
pub fn make_exception_fields(int32: Type, int64: Type, str: Type) -> Vec<(StrRef, Type, bool)> {
    vec![
        ("__name__".into(), int32, true),
        ("__file__".into(), str, true),
        ("__line__".into(), int32, true),
        ("__col__".into(), int32, true),
        ("__func__".into(), str, true),
        ("__message__".into(), str, true),
        ("__param0__".into(), int64, true),
        ("__param1__".into(), int64, true),
        ("__param2__".into(), int64, true),
    ]
}

impl TopLevelDef {
    pub fn to_string(&self, unifier: &mut Unifier) -> String {
        match self {
            TopLevelDef::Class { name, ancestors, fields, methods, type_vars, .. } => {
                let fields_str = fields
                    .iter()
                    .map(|(n, ty, _)| (n.to_string(), unifier.stringify(*ty)))
                    .collect_vec();

                let methods_str = methods
                    .iter()
                    .map(|(n, ty, id)| (n.to_string(), unifier.stringify(*ty), *id))
                    .collect_vec();
                format!(
                    "Class {{\nname: {:?},\nancestors: {:?},\nfields: {:?},\nmethods: {:?},\ntype_vars: {:?}\n}}",
                    name,
                    ancestors.iter().map(|ancestor| ancestor.stringify(unifier)).collect_vec(),
                    fields_str.iter().map(|(a, _)| a).collect_vec(),
                    methods_str.iter().map(|(a, b, _)| (a, b)).collect_vec(),
                    type_vars.iter().map(|id| unifier.stringify(*id)).collect_vec(),
                )
            }
            TopLevelDef::Function { name, signature, var_id, .. } => format!(
                "Function {{\nname: {:?},\nsig: {:?},\nvar_id: {:?}\n}}",
                name,
                unifier.stringify(*signature),
                {
                    // preserve the order for debug output and test
                    let mut r = var_id.clone();
                    r.sort_unstable();
                    r
                }
            ),
            TopLevelDef::Variable { name, ty, .. } => {
                format!("Variable {{ name: {name:?}, ty: {:?} }}", unifier.stringify(*ty),)
            }
        }
    }
}

impl TopLevelComposer {
    #[must_use]
    pub fn make_primitives(size_t: u32) -> (PrimitiveStore, Unifier) {
        let mut unifier = Unifier::new();
        let int32 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Int32.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let int64 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Int64.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let float = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Float.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let bool = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Bool.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let none = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::None.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let range = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Range.id(),
            fields: [
                ("start".into(), (int32, true)),
                ("stop".into(), (int32, true)),
                ("step".into(), (int32, true)),
            ]
            .into_iter()
            .collect(),
            params: VarMap::new(),
        });
        let str = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Str.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let exception = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Exception.id(),
            fields: make_exception_fields(int32, int64, str)
                .into_iter()
                .map(|(name, ty, mutable)| (name, (ty, mutable)))
                .collect(),
            params: VarMap::new(),
        });
        let uint32 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::UInt32.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });
        let uint64 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::UInt64.id(),
            fields: HashMap::new(),
            params: VarMap::new(),
        });

        let option_type_var = unifier.get_fresh_var(Some("option_type_var".into()), None);
        let is_some_type_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: bool,
            vars: into_var_map([option_type_var]),
        }));
        let unwrap_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: option_type_var.ty,
            vars: into_var_map([option_type_var]),
        }));
        let option = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::Option.id(),
            fields: vec![
                (PrimDef::FunOptionIsSome.simple_name().into(), (is_some_type_fun_ty, true)),
                (PrimDef::FunOptionIsNone.simple_name().into(), (is_some_type_fun_ty, true)),
                (PrimDef::FunOptionUnwrap.simple_name().into(), (unwrap_fun_ty, true)),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            params: into_var_map([option_type_var]),
        });

        let size_t_ty = match size_t {
            32 => uint32,
            64 => uint64,
            _ => unreachable!(),
        };

        let list_elem_tvar = unifier.get_fresh_var(Some("list_elem".into()), None);
        let list = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::List.id(),
            fields: Mapping::new(),
            params: into_var_map([list_elem_tvar]),
        });

        let ndarray_dtype_tvar = unifier.get_fresh_var(Some("ndarray_dtype".into()), None);
        let ndarray_ndims_tvar =
            unifier.get_fresh_const_generic_var(size_t_ty, Some("ndarray_ndims".into()), None);
        let ndarray_copy_fun_ret_ty = unifier.get_fresh_var(None, None);
        let ndarray_copy_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: ndarray_copy_fun_ret_ty.ty,
            vars: into_var_map([ndarray_dtype_tvar, ndarray_ndims_tvar]),
        }));
        let ndarray_fill_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![FuncArg {
                name: "value".into(),
                ty: ndarray_dtype_tvar.ty,
                default_value: None,
                is_vararg: false,
            }],
            ret: none,
            vars: into_var_map([ndarray_dtype_tvar, ndarray_ndims_tvar]),
        }));
        let ndarray = unifier.add_ty(TypeEnum::TObj {
            obj_id: PrimDef::NDArray.id(),
            fields: Mapping::from([
                (PrimDef::FunNDArrayCopy.simple_name().into(), (ndarray_copy_fun_ty, true)),
                (PrimDef::FunNDArrayFill.simple_name().into(), (ndarray_fill_fun_ty, true)),
            ]),
            params: into_var_map([ndarray_dtype_tvar, ndarray_ndims_tvar]),
        });

        unifier.unify(ndarray_copy_fun_ret_ty.ty, ndarray).unwrap();

        let primitives = PrimitiveStore {
            int32,
            int64,
            uint32,
            uint64,
            float,
            bool,
            none,
            range,
            str,
            exception,
            option,
            list,
            ndarray,
            size_t,
        };
        unifier.put_primitive_store(&primitives);
        crate::typecheck::magic_methods::set_primitives_magic_methods(&primitives, &mut unifier);
        (primitives, unifier)
    }

    /// already include the `definition_id` of itself inside the ancestors vector
    /// when first registering, the `type_vars`, fields, methods, ancestors are invalid
    #[must_use]
    pub fn make_top_level_class_def(
        obj_id: DefinitionId,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        name: StrRef,
        constructor: Option<Type>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Class {
            name,
            object_id: obj_id,
            type_vars: Vec::default(),
            fields: Vec::default(),
            attributes: Vec::default(),
            methods: Vec::default(),
            ancestors: Vec::default(),
            constructor,
            resolver,
            loc,
        }
    }

    /// when first registering, the type is a invalid value
    #[must_use]
    pub fn make_top_level_function_def(
        name: String,
        simple_name: StrRef,
        ty: Type,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Function {
            name,
            simple_name,
            signature: ty,
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver,
            codegen_callback: None,
            loc,
        }
    }

    #[must_use]
    pub fn make_top_level_variable_def(
        name: String,
        simple_name: StrRef,
        ty: Type,
        ty_decl: Option<Expr>,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Variable { name, simple_name, ty, ty_decl, resolver, loc }
    }

    #[must_use]
    pub fn make_class_method_name(mut class_name: String, method_name: &str) -> String {
        class_name.push('.');
        class_name.push_str(method_name);
        class_name
    }

    pub fn get_class_method_def_info(
        class_methods_def: &[(StrRef, Type, DefinitionId)],
        method_name: StrRef,
    ) -> Result<(Type, DefinitionId), HashSet<String>> {
        for (name, ty, def_id) in class_methods_def {
            if name == &method_name {
                return Ok((*ty, *def_id));
            }
        }
        Err(HashSet::from([format!("no method {method_name} in the current class")]))
    }

    /// get all base class def id of a class, excluding itself. \
    /// this function should called only after the direct parent is set
    /// and before all the ancestors are set
    /// and when we allow single inheritance \
    /// the order of the returned list is from the child to the deepest ancestor
    pub fn get_all_ancestors_helper(
        child: &TypeAnnotation,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
    ) -> Result<Vec<TypeAnnotation>, HashSet<String>> {
        let mut result: Vec<TypeAnnotation> = Vec::new();
        let mut parent = Self::get_parent(child, temp_def_list);
        while let Some(p) = parent {
            parent = Self::get_parent(&p, temp_def_list);
            let p_id = if let TypeAnnotation::CustomClass { id, .. } = &p {
                *id
            } else {
                unreachable!("must be class kind annotation")
            };
            // check cycle
            let no_cycle = result.iter().all(|x| {
                let TypeAnnotation::CustomClass { id, .. } = x else {
                    unreachable!("must be class kind annotation")
                };

                id.0 != p_id.0
            });
            if no_cycle {
                result.push(p);
            } else {
                return Err(HashSet::from(["cyclic inheritance detected".into()]));
            }
        }
        Ok(result)
    }

    /// should only be called when finding all ancestors, so panic when wrong
    fn get_parent(
        child: &TypeAnnotation,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
    ) -> Option<TypeAnnotation> {
        let child_id = if let TypeAnnotation::CustomClass { id, .. } = child {
            *id
        } else {
            unreachable!("should be class type annotation")
        };
        let child_def = temp_def_list.get(child_id.0).unwrap();
        let child_def = child_def.read();
        let TopLevelDef::Class { ancestors, .. } = &*child_def else {
            unreachable!("child must be top level class def")
        };

        if ancestors.is_empty() {
            None
        } else {
            Some(ancestors[0].clone())
        }
    }

    /// get the `var_id` of a given `TVar` type
    pub fn get_var_id(var_ty: Type, unifier: &mut Unifier) -> Result<TypeVarId, HashSet<String>> {
        if let TypeEnum::TVar { id, .. } = unifier.get_ty(var_ty).as_ref() {
            Ok(*id)
        } else {
            Err(HashSet::from(["not type var".to_string()]))
        }
    }

    pub fn check_overload_function_type(
        this: Type,
        other: Type,
        unifier: &mut Unifier,
        type_var_to_concrete_def: &HashMap<Type, TypeAnnotation>,
    ) -> bool {
        let this = unifier.get_ty(this);
        let this = this.as_ref();
        let other = unifier.get_ty(other);
        let other = other.as_ref();
        let (
            TypeEnum::TFunc(FunSignature { args: this_args, ret: this_ret, .. }),
            TypeEnum::TFunc(FunSignature { args: other_args, ret: other_ret, .. }),
        ) = (this, other)
        else {
            unreachable!("this function must be called with function type")
        };

        // check args
        let args_ok =
            this_args
                .iter()
                .map(|FuncArg { name, ty, .. }| (name, type_var_to_concrete_def.get(ty).unwrap()))
                .zip(other_args.iter().map(|FuncArg { name, ty, .. }| {
                    (name, type_var_to_concrete_def.get(ty).unwrap())
                }))
                .all(|(this, other)| {
                    if this.0 == &"self".into() && this.0 == other.0 {
                        true
                    } else {
                        this.0 == other.0
                            && check_overload_type_annotation_compatible(this.1, other.1, unifier)
                    }
                });

        // check rets
        let ret_ok = check_overload_type_annotation_compatible(
            type_var_to_concrete_def.get(this_ret).unwrap(),
            type_var_to_concrete_def.get(other_ret).unwrap(),
            unifier,
        );

        // return
        args_ok && ret_ok
    }

    pub fn check_overload_field_type(
        this: Type,
        other: Type,
        unifier: &mut Unifier,
        type_var_to_concrete_def: &HashMap<Type, TypeAnnotation>,
    ) -> bool {
        check_overload_type_annotation_compatible(
            type_var_to_concrete_def.get(&this).unwrap(),
            type_var_to_concrete_def.get(&other).unwrap(),
            unifier,
        )
    }

    /// This function returns the fields that have been initialized in the `__init__` function of a class
    /// The function takes as input:
    ///  * `class_id`: The `object_id` of the class whose function is being evaluated (check `TopLevelDef::Class`)
    ///  * `definition_ast_list`: A list of ast definitions and statements defined in `TopLevelComposer`
    ///  * `stmts`: The body of function being parsed. Each statment is analyzed to check varaible initialization statements
    pub fn get_all_assigned_field(
        class_id: usize,
        definition_ast_list: &Vec<DefAst>,
        stmts: &[Stmt<()>],
    ) -> Result<HashSet<StrRef>, HashSet<String>> {
        let mut result = HashSet::new();
        for s in stmts {
            match &s.node {
                ast::StmtKind::AnnAssign { target, .. }
                    if {
                        if let ast::ExprKind::Attribute { value, .. } = &target.node {
                            if let ast::ExprKind::Name { id, .. } = &value.node {
                                id == &"self".into()
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } =>
                {
                    return Err(HashSet::from([format!(
                        "redundant type annotation for class fields at {}",
                        s.location
                    )]))
                }
                ast::StmtKind::Assign { targets, .. } => {
                    for t in targets {
                        if let ast::ExprKind::Attribute { value, attr, .. } = &t.node {
                            if let ast::ExprKind::Name { id, .. } = &value.node {
                                if id == &"self".into() {
                                    result.insert(*attr);
                                }
                            }
                        }
                    }
                }
                // TODO: do not check for For and While?
                ast::StmtKind::For { body, orelse, .. }
                | ast::StmtKind::While { body, orelse, .. } => {
                    result.extend(Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        body.as_slice(),
                    )?);
                    result.extend(Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        orelse.as_slice(),
                    )?);
                }
                ast::StmtKind::If { body, orelse, .. } => {
                    let inited_for_sure = Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        body.as_slice(),
                    )?
                    .intersection(&Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        orelse.as_slice(),
                    )?)
                    .copied()
                    .collect::<HashSet<_>>();
                    result.extend(inited_for_sure);
                }
                ast::StmtKind::Try { body, orelse, finalbody, .. } => {
                    let inited_for_sure = Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        body.as_slice(),
                    )?
                    .intersection(&Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        orelse.as_slice(),
                    )?)
                    .copied()
                    .collect::<HashSet<_>>();
                    result.extend(inited_for_sure);
                    result.extend(Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        finalbody.as_slice(),
                    )?);
                }
                ast::StmtKind::With { body, .. } => {
                    result.extend(Self::get_all_assigned_field(
                        class_id,
                        definition_ast_list,
                        body.as_slice(),
                    )?);
                }
                // Variables Initialized in function calls
                ast::StmtKind::Expr { value, .. } => {
                    let ExprKind::Call { func, .. } = &value.node else {
                        continue;
                    };
                    let ExprKind::Attribute { value, attr, .. } = &func.node else {
                        continue;
                    };
                    let ExprKind::Name { id, .. } = &value.node else {
                        continue;
                    };
                    // Need to consider the two cases:
                    //  Case 1) Call to class function i.e. id = `self`
                    //  Case 2) Call to class ancestor function i.e. id = ancestor_name
                    // We leave checking whether function in case 2 belonged to class ancestor or not to type checker
                    //
                    // According to current handling of `self`, function definition are fixed and do not change regardless
                    // of which object is passed as `self` i.e. virtual polymorphism is not supported
                    // Therefore, we change class id for case 2 to reflect behavior of our compiler

                    let class_name = if *id == "self".into() {
                        let ast::StmtKind::ClassDef { name, .. } =
                            &definition_ast_list[class_id].1.as_ref().unwrap().node
                        else {
                            unreachable!()
                        };
                        name
                    } else {
                        id
                    };

                    let parent_method = definition_ast_list.iter().find_map(|def| {
                        let (
                            class_def,
                            Some(ast::Located {
                                node: ast::StmtKind::ClassDef { name, body, .. },
                                ..
                            }),
                        ) = &def
                        else {
                            return None;
                        };
                        let TopLevelDef::Class { object_id: class_id, .. } = &*class_def.read()
                        else {
                            unreachable!()
                        };

                        if name == class_name {
                            body.iter().find_map(|m| {
                                let ast::StmtKind::FunctionDef { name, body, .. } = &m.node else {
                                    return None;
                                };
                                if *name == *attr {
                                    return Some((body.clone(), class_id.0));
                                }
                                None
                            })
                        } else {
                            None
                        }
                    });

                    // If method body is none then method does not exist
                    if let Some((method_body, class_id)) = parent_method {
                        result.extend(Self::get_all_assigned_field(
                            class_id,
                            definition_ast_list,
                            method_body.as_slice(),
                        )?);
                    } else {
                        return Err(HashSet::from([format!(
                            "{}.{} not found in class {class_name} at {}",
                            *id, *attr, value.location
                        )]));
                    }
                }
                ast::StmtKind::Pass { .. }
                | ast::StmtKind::Assert { .. }
                | ast::StmtKind::AnnAssign { .. } => {}

                _ => {
                    unimplemented!()
                }
            }
        }
        Ok(result)
    }

    pub fn parse_parameter_default_value(
        default: &ast::Expr,
        resolver: &(dyn SymbolResolver + Send + Sync),
    ) -> Result<SymbolValue, HashSet<String>> {
        parse_parameter_default_value(default, resolver)
    }

    pub fn check_default_param_type(
        val: &SymbolValue,
        ty: &TypeAnnotation,
        primitive: &PrimitiveStore,
        unifier: &mut Unifier,
    ) -> Result<(), String> {
        fn is_compatible(
            found: &TypeAnnotation,
            expect: &TypeAnnotation,
            unifier: &mut Unifier,
            primitive: &PrimitiveStore,
        ) -> bool {
            match (found, expect) {
                (TypeAnnotation::Primitive(f), TypeAnnotation::Primitive(e)) => {
                    unifier.unioned(*f, *e)
                }
                (
                    TypeAnnotation::CustomClass { id: f_id, params: f_param },
                    TypeAnnotation::CustomClass { id: e_id, params: e_param },
                ) => {
                    *f_id == *e_id
                        && *f_id == primitive.option.obj_id(unifier).unwrap()
                        && (f_param.is_empty()
                            || (f_param.len() == 1
                                && e_param.len() == 1
                                && is_compatible(&f_param[0], &e_param[0], unifier, primitive)))
                }
                (TypeAnnotation::Tuple(f), TypeAnnotation::Tuple(e)) => {
                    f.len() == e.len()
                        && f.iter()
                            .zip(e.iter())
                            .all(|(f, e)| is_compatible(f, e, unifier, primitive))
                }
                _ => false,
            }
        }

        let found = val.get_type_annotation(primitive, unifier);
        if is_compatible(&found, ty, unifier, primitive) {
            Ok(())
        } else {
            Err(format!(
                "incompatible default parameter type, expect {}, found {}",
                ty.stringify(unifier),
                found.stringify(unifier),
            ))
        }
    }
}

pub fn parse_parameter_default_value(
    default: &ast::Expr,
    resolver: &(dyn SymbolResolver + Send + Sync),
) -> Result<SymbolValue, HashSet<String>> {
    fn handle_constant(val: &Constant, loc: &Location) -> Result<SymbolValue, HashSet<String>> {
        match val {
            Constant::Int(v) => {
                if let Ok(v) = (*v).try_into() {
                    Ok(SymbolValue::I32(v))
                } else {
                    Err(HashSet::from([format!("integer value out of range at {loc}")]))
                }
            }
            Constant::Float(v) => Ok(SymbolValue::Double(*v)),
            Constant::Bool(v) => Ok(SymbolValue::Bool(*v)),
            Constant::Tuple(tuple) => Ok(SymbolValue::Tuple(
                tuple.iter().map(|x| handle_constant(x, loc)).collect::<Result<Vec<_>, _>>()?,
            )),
            Constant::None => Err(HashSet::from([format!(
                "`None` is not supported, use `none` for option type instead ({loc})"
            )])),
            _ => unimplemented!("this constant is not supported at {}", loc),
        }
    }
    match &default.node {
        ast::ExprKind::Constant { value, .. } => handle_constant(value, &default.location),
        ast::ExprKind::Call { func, args, .. } if args.len() == 1 => match &func.node {
            ast::ExprKind::Name { id, .. } if *id == "int64".into() => match &args[0].node {
                ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                    let v: Result<i64, _> = (*v).try_into();
                    match v {
                        Ok(v) => Ok(SymbolValue::I64(v)),
                        _ => Err(HashSet::from([format!(
                            "default param value out of range at {}",
                            default.location
                        )])),
                    }
                }
                _ => Err(HashSet::from([format!(
                    "only allow constant integer here at {}",
                    default.location
                )])),
            },
            ast::ExprKind::Name { id, .. } if *id == "uint32".into() => match &args[0].node {
                ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                    let v: Result<u32, _> = (*v).try_into();
                    match v {
                        Ok(v) => Ok(SymbolValue::U32(v)),
                        _ => Err(HashSet::from([format!(
                            "default param value out of range at {}",
                            default.location
                        )])),
                    }
                }
                _ => Err(HashSet::from([format!(
                    "only allow constant integer here at {}",
                    default.location
                )])),
            },
            ast::ExprKind::Name { id, .. } if *id == "uint64".into() => match &args[0].node {
                ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                    let v: Result<u64, _> = (*v).try_into();
                    match v {
                        Ok(v) => Ok(SymbolValue::U64(v)),
                        _ => Err(HashSet::from([format!(
                            "default param value out of range at {}",
                            default.location
                        )])),
                    }
                }
                _ => Err(HashSet::from([format!(
                    "only allow constant integer here at {}",
                    default.location
                )])),
            },
            ast::ExprKind::Name { id, .. } if *id == "Some".into() => Ok(SymbolValue::OptionSome(
                Box::new(parse_parameter_default_value(&args[0], resolver)?),
            )),
            _ => Err(HashSet::from([format!(
                "unsupported default parameter at {}",
                default.location
            )])),
        },
        ast::ExprKind::Tuple { elts, .. } => Ok(SymbolValue::Tuple(
            elts.iter()
                .map(|x| parse_parameter_default_value(x, resolver))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        ast::ExprKind::Name { id, .. } if id == &"none".into() => Ok(SymbolValue::OptionNone),
        ast::ExprKind::Name { id, .. } => {
            resolver.get_default_param_value(default).ok_or_else(|| {
                HashSet::from([format!(
                    "`{}` cannot be used as a default parameter at {} \
                        (not primitive type, option or tuple / not defined?)",
                    id, default.location
                )])
            })
        }
        _ => Err(HashSet::from([format!(
            "unsupported default parameter (not primitive type, option or tuple) at {}",
            default.location
        )])),
    }
}

/// Obtains the element type of an array-like type.
pub fn arraylike_flatten_element_type(unifier: &mut Unifier, ty: Type) -> Type {
    match &*unifier.get_ty(ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            unpack_ndarray_var_tys(unifier, ty).0
        }

        TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
            arraylike_flatten_element_type(unifier, iter_type_vars(params).next().unwrap().ty)
        }
        _ => ty,
    }
}

/// Obtains the number of dimensions of an array-like type.
pub fn arraylike_get_ndims(unifier: &mut Unifier, ty: Type) -> u64 {
    match &*unifier.get_ty(ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            let ndims = unpack_ndarray_var_tys(unifier, ty).1;
            let TypeEnum::TLiteral { values, .. } = &*unifier.get_ty_immutable(ndims) else {
                panic!("Expected TLiteral for ndarray.ndims, got {}", unifier.stringify(ndims))
            };

            if values.len() > 1 {
                todo!("Getting num of dimensions for ndarray with more than one ndim bound is unimplemented")
            }

            u64::try_from(values[0].clone()).unwrap()
        }

        TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
            arraylike_get_ndims(unifier, iter_type_vars(params).next().unwrap().ty) + 1
        }
        _ => 0,
    }
}
