use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    sync::Arc,
};

use itertools::Itertools;
use parking_lot::RwLock;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use nac3parser::ast::{self, Constant, ExprKind, Located, Location, Stmt, StrRef};

use super::{
    DefinitionId, FunAttribute, TopLevelDef, check_overload_type_annotation_compatible,
    composer::{BuiltinRegistry, DefAst, TopLevelComposer},
    make_self_type_annotation,
    numpy::unpack_ndarray_var_tys,
    parse_ast_to_type_annotation_kinds,
    type_annotation::TypeAnnotation,
};
use crate::{
    symbol_resolver::{SymbolResolver, SymbolValue},
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{
            AttrKind, FunSignature, FuncArg, Mapping, Type, TypeEnum, TypeVarId, Unifier, VarMap,
            into_var_map, iter_type_vars,
        },
    },
};

/// All primitive types and functions in nac3core.
#[derive(Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum PrimDef {
    // Core primitives
    Float,
    Bool,
    Str,
    List,
    NDArray,
    Tuple,
    Exception,

    // Core functions
    Range,
    FunRound,
    FunRound64,
    FunFloor,
    FunFloor64,
    FunCeil,
    FunCeil64,
    FunLen,
    FunSome,
    FunMin,
    FunMax,
    FunAbs,
    StaticMethod,

    // Type qualifiers
    Kernel,
    KernelInvariant,
    ConstGeneric,
    None,
    Virtual,
    Option,

    // Decorators
    Compile,
    ExternFn,
    KernelDecorator,
    Portable,
    Rpc,

    // Typing constructs
    Generic,
    Literal,

    // NumPy type aliases
    Int32,
    Int64,
    UInt32,
    UInt64,
    Float64,

    // NumPy factory functions
    FunNpNDArray,
    FunNpEmpty,
    FunNpZeros,
    FunNpOnes,
    FunNpFull,
    FunNpArray,
    FunNpEye,
    FunNpIdentity,

    // NumPy ndarray property getters
    FunNpSize,
    FunNpShape,
    FunNpStrides,

    // NumPy ndarray view functions
    FunNpBroadcastTo,
    FunNpTranspose,
    FunNpReshape,

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
    FunNpAny,
    FunNpAll,

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
    FunStr,
    FunBool,

    // Option methods
    FunOptionIsSome,
    FunOptionIsNone,
    FunOptionUnwrap,

    // NDArray methods
    FunNDArrayCopy,
    FunNDArrayFill,

    // Range methods
    FunRangeInit,
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
    pub const fn id(&self) -> DefinitionId {
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
            // Core primitives
            Self::Float => class("float", |primitives| primitives.float),
            Self::Bool => class("bool", |primitives| primitives.bool),
            Self::Str => class("str", |primitives| primitives.str),
            Self::List => class("list", |primitives| primitives.list),
            Self::NDArray => class("ndarray", |primitives| primitives.ndarray),
            Self::Tuple => class("tuple", |_| unimplemented!()),
            Self::Exception => class("Exception", |primitives| primitives.exception),

            // Core functions
            Self::Range => class("range", |primitives| primitives.range),
            Self::FunRound => fun("round", None),
            Self::FunRound64 => fun("round64", None),
            Self::FunFloor => fun("floor", None),
            Self::FunFloor64 => fun("floor64", None),
            Self::FunCeil => fun("ceil", None),
            Self::FunCeil64 => fun("ceil64", None),
            Self::FunLen => fun("len", None),
            Self::FunSome => fun("Some", None),
            Self::FunMin => fun("min", None),
            Self::FunMax => fun("max", None),
            Self::FunAbs => fun("abs", None),
            Self::StaticMethod => class("staticmethod", |_| unimplemented!()),

            // Type qualifiers
            Self::Kernel => class("Kernel", |_| unimplemented!()),
            Self::KernelInvariant => class("KernelInvariant", |_| unimplemented!()),
            Self::ConstGeneric => class("ConstGeneric", |_| unimplemented!()),
            Self::None => class("none", |primitives| primitives.none),
            Self::Virtual => class("virtual", |_| unimplemented!()),
            Self::Option => class("Option", |primitives| primitives.option),

            // Decorators
            Self::Compile => class("compile", |_| unimplemented!()),
            Self::ExternFn => class("extern", |_| unimplemented!()),
            Self::KernelDecorator => class("kernel", |_| unimplemented!()),
            Self::Portable => class("portable", |_| unimplemented!()),
            Self::Rpc => class("rpc", |_| unimplemented!()),

            // Typing constructs
            Self::Generic => class("Generic", |_| unimplemented!()),
            Self::Literal => class("Literal", |_| unimplemented!()),

            // NumPy type aliases
            Self::Int32 => class("int32", |primitives| primitives.int32),
            Self::Int64 => class("int64", |primitives| primitives.int64),
            Self::UInt32 => class("uint32", |primitives| primitives.uint32),
            Self::UInt64 => class("uint64", |primitives| primitives.uint64),
            Self::Float64 => class("float64", |primitives| primitives.float),

            // NumPy factory functions
            Self::FunNpNDArray => fun("np_ndarray", None),
            Self::FunNpEmpty => fun("np_empty", None),
            Self::FunNpZeros => fun("np_zeros", None),
            Self::FunNpOnes => fun("np_ones", None),
            Self::FunNpFull => fun("np_full", None),
            Self::FunNpArray => fun("np_array", None),
            Self::FunNpEye => fun("np_eye", None),
            Self::FunNpIdentity => fun("np_identity", None),

            // NumPy NDArray property getters,
            Self::FunNpSize => fun("np_size", None),
            Self::FunNpShape => fun("np_shape", None),
            Self::FunNpStrides => fun("np_strides", None),

            // NumPy NDArray view functions
            Self::FunNpBroadcastTo => fun("np_broadcast_to", None),
            Self::FunNpTranspose => fun("np_transpose", None),
            Self::FunNpReshape => fun("np_reshape", None),

            // Miscellaneous NumPy & SciPy functions
            Self::FunNpRound => fun("np_round", None),
            Self::FunNpFloor => fun("np_floor", None),
            Self::FunNpCeil => fun("np_ceil", None),
            Self::FunNpMin => fun("np_min", None),
            Self::FunNpMinimum => fun("np_minimum", None),
            Self::FunNpArgmin => fun("np_argmin", None),
            Self::FunNpMax => fun("np_max", None),
            Self::FunNpMaximum => fun("np_maximum", None),
            Self::FunNpArgmax => fun("np_argmax", None),
            Self::FunNpIsNan => fun("np_isnan", None),
            Self::FunNpIsInf => fun("np_isinf", None),
            Self::FunNpSin => fun("np_sin", None),
            Self::FunNpCos => fun("np_cos", None),
            Self::FunNpExp => fun("np_exp", None),
            Self::FunNpExp2 => fun("np_exp2", None),
            Self::FunNpLog => fun("np_log", None),
            Self::FunNpLog10 => fun("np_log10", None),
            Self::FunNpLog2 => fun("np_log2", None),
            Self::FunNpFabs => fun("np_fabs", None),
            Self::FunNpSqrt => fun("np_sqrt", None),
            Self::FunNpRint => fun("np_rint", None),
            Self::FunNpTan => fun("np_tan", None),
            Self::FunNpArcsin => fun("np_arcsin", None),
            Self::FunNpArccos => fun("np_arccos", None),
            Self::FunNpArctan => fun("np_arctan", None),
            Self::FunNpSinh => fun("np_sinh", None),
            Self::FunNpCosh => fun("np_cosh", None),
            Self::FunNpTanh => fun("np_tanh", None),
            Self::FunNpArcsinh => fun("np_arcsinh", None),
            Self::FunNpArccosh => fun("np_arccosh", None),
            Self::FunNpArctanh => fun("np_arctanh", None),
            Self::FunNpExpm1 => fun("np_expm1", None),
            Self::FunNpCbrt => fun("np_cbrt", None),
            Self::FunSpSpecErf => fun("sp_spec_erf", None),
            Self::FunSpSpecErfc => fun("sp_spec_erfc", None),
            Self::FunSpSpecGamma => fun("sp_spec_gamma", None),
            Self::FunSpSpecGammaln => fun("sp_spec_gammaln", None),
            Self::FunSpSpecJ0 => fun("sp_spec_j0", None),
            Self::FunSpSpecJ1 => fun("sp_spec_j1", None),
            Self::FunNpArctan2 => fun("np_arctan2", None),
            Self::FunNpCopysign => fun("np_copysign", None),
            Self::FunNpFmax => fun("np_fmax", None),
            Self::FunNpFmin => fun("np_fmin", None),
            Self::FunNpLdExp => fun("np_ldexp", None),
            Self::FunNpHypot => fun("np_hypot", None),
            Self::FunNpNextAfter => fun("np_nextafter", None),
            Self::FunNpAny => fun("np_any", None),
            Self::FunNpAll => fun("np_all", None),

            // Linalg functions
            Self::FunNpDot => fun("np_dot", None),
            Self::FunNpLinalgCholesky => fun("np_linalg_cholesky", None),
            Self::FunNpLinalgQr => fun("np_linalg_qr", None),
            Self::FunNpLinalgSvd => fun("np_linalg_svd", None),
            Self::FunNpLinalgInv => fun("np_linalg_inv", None),
            Self::FunNpLinalgPinv => fun("np_linalg_pinv", None),
            Self::FunNpLinalgMatrixPower => fun("np_linalg_matrix_power", None),
            Self::FunNpLinalgDet => fun("np_linalg_det", None),
            Self::FunSpLinalgLu => fun("sp_linalg_lu", None),
            Self::FunSpLinalgSchur => fun("sp_linalg_schur", None),
            Self::FunSpLinalgHessenberg => fun("sp_linalg_hessenberg", None),

            // Miscellaneous Python & NAC3 functions
            Self::FunInt32 => fun("int32", None),
            Self::FunInt64 => fun("int64", None),
            Self::FunUInt32 => fun("uint32", None),
            Self::FunUInt64 => fun("uint64", None),
            Self::FunFloat => fun("float", None),
            Self::FunStr => fun("str", None),
            Self::FunBool => fun("bool", None),

            // Option methods
            Self::FunOptionIsSome => fun("Option.is_some", Some("is_some")),
            Self::FunOptionIsNone => fun("Option.is_none", Some("is_none")),
            Self::FunOptionUnwrap => fun("Option.unwrap", Some("unwrap")),

            // NDArray methods
            Self::FunNDArrayCopy => fun("ndarray.copy", Some("copy")),
            Self::FunNDArrayFill => fun("ndarray.fill", Some("fill")),

            // Range methods
            Self::FunRangeInit => fun("range.__init__", Some("__init__")),
        }
    }
}

/// Asserts that a [`PrimDef`] is in an allowlist.
///
/// Like `debug_assert!`, this statements of this function are only
/// enabled if `cfg!(debug_assertions)` is true.
pub fn debug_assert_prim_is_allowed(prim: PrimDef, allowlist: &[PrimDef]) {
    debug_assert!(
        allowlist.contains(&prim),
        "Disallowed primitive definition. Got {prim:?}, but expects it to be in {allowlist:?}"
    );
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
            Self::Module { name, functions, .. } => {
                format!(
                    "Module {{\nname: {:?},\nmethods: {:?}\n}}",
                    name,
                    functions.iter().map(|(n, _)| n.to_string()).collect_vec()
                )
            }
            Self::Class { name, ancestors, fields, methods, attributes, type_vars, .. } => {
                let fields_str = fields
                    .iter()
                    .map(|(n, ty, _)| (n.to_string(), unifier.stringify(*ty)))
                    .collect_vec();

                let attributes_str = attributes
                    .iter()
                    .map(|(n, ty, _)| (n.to_string(), unifier.stringify(*ty)))
                    .collect_vec();

                let methods_str = methods
                    .iter()
                    .map(|(n, ty, id)| (n.to_string(), unifier.stringify(*ty), *id))
                    .collect_vec();
                format!(
                    "Class {{\nname: {:?},\nancestors: {:?},\nfields: {:?},\nattributes: {:?},\nmethods: {:?},\ntype_vars: {:?}\n}}",
                    name,
                    ancestors.iter().map(|ancestor| ancestor.stringify(unifier)).collect_vec(),
                    fields_str.iter().map(|(a, _)| a).collect_vec(),
                    attributes_str.iter().map(|(a, _)| a).collect_vec(),
                    methods_str.iter().map(|(a, b, _)| (a, b)).collect_vec(),
                    type_vars.iter().map(|id| unifier.stringify(*id)).collect_vec(),
                )
            }
            Self::Function { name, signature, var_id, .. } => format!(
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
                ("start".into(), (int32, AttrKind::Field { mutable: false })),
                ("stop".into(), (int32, AttrKind::Field { mutable: false })),
                ("step".into(), (int32, AttrKind::Field { mutable: false })),
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
                .map(|(name, ty, mutable)| (name, (ty, AttrKind::Field { mutable })))
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
                (
                    PrimDef::FunOptionIsSome.simple_name().into(),
                    (is_some_type_fun_ty, AttrKind::Method),
                ),
                (
                    PrimDef::FunOptionIsNone.simple_name().into(),
                    (is_some_type_fun_ty, AttrKind::Method),
                ),
                (PrimDef::FunOptionUnwrap.simple_name().into(), (unwrap_fun_ty, AttrKind::Method)),
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
                (
                    PrimDef::FunNDArrayCopy.simple_name().into(),
                    (ndarray_copy_fun_ty, AttrKind::Method),
                ),
                (
                    PrimDef::FunNDArrayFill.simple_name().into(),
                    (ndarray_fill_fun_ty, AttrKind::Method),
                ),
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
            simple_name: {
                let name = name.to_string();
                name.rsplit_once('.').map(|(_, nme)| nme.to_string()).unwrap_or(name)
            },
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
        attributes: Vec<FunAttribute>,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Function {
            name,
            simple_name,
            signature: ty,
            var_id: Vec::default(),
            attributes,
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver,
            codegen_callback: None,
            loc,
        }
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
                    )]));
                }
                ast::StmtKind::Assign { targets, .. } => {
                    for t in targets {
                        if let ast::ExprKind::Attribute { value, attr, .. } = &t.node
                            && let ast::ExprKind::Name { id, .. } = &value.node
                            && id == &"self".into()
                        {
                            result.insert(*attr);
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
        builtin_registry: &Arc<dyn BuiltinRegistry>,
    ) -> Result<SymbolValue, HashSet<String>> {
        parse_parameter_default_value(default, resolver, builtin_registry)
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

    /// Parses the class type variables and direct parents
    /// we only allow single inheritance
    pub fn analyze_class_bases(
        class_def: &Arc<RwLock<TopLevelDef>>,
        class_ast: &Option<Stmt>,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        primitives_store: &PrimitiveStore,
        builtin_registry: &Arc<dyn BuiltinRegistry>,
    ) -> Result<(), HashSet<String>> {
        let TopLevelDef::Class {
            object_id: class_def_id,
            ancestors: class_ancestors,
            type_vars: class_type_vars,
            resolver,
            ..
        } = &mut *class_def.write()
        else {
            unreachable!()
        };
        let Some(ast::Located {
            node: ast::StmtKind::ClassDef { bases: class_bases_ast, .. }, ..
        }) = class_ast
        else {
            unreachable!()
        };
        let class_resolver = resolver.as_ref().unwrap().as_ref();

        let mut is_generic = false;
        let mut has_base = false;
        // Check class bases for typevars
        for b in class_bases_ast {
            match &b.node {
                // analyze typevars bounded to the class,
                // only support things like `class A(Generic[T, V])`,
                // things like `class A(Generic[T, V, ImportedModule.T])` is not supported
                // i.e. only simple names are allowed in the subscript
                // should update the TopLevelDef::Class.typevars and the TypeEnum::TObj.params
                ast::ExprKind::Subscript { slice, .. }
                    if builtin_registry
                        .has_generic_ann(b)
                        .map_err(|err| HashSet::from([err.to_string()]))? =>
                {
                    if is_generic {
                        return Err(HashSet::from([format!(
                            "only single Generic[...] is allowed (at {})",
                            b.location
                        )]));
                    }
                    is_generic = true;

                    let type_var_list = if let ast::ExprKind::Tuple { elts, .. } = &slice.node {
                        // if `class A(Generic[T, V, G])`
                        elts.iter().collect_vec()
                    } else {
                        // `class A(Generic[T])`
                        vec![&**slice]
                    };

                    let type_vars = type_var_list
                        .into_iter()
                        .map(|e| {
                            class_resolver.parse_type_annotation(
                                temp_def_list,
                                unifier,
                                primitives_store,
                                e,
                                builtin_registry,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    class_type_vars.extend(type_vars);
                }
                ast::ExprKind::Name { .. } | ast::ExprKind::Subscript { .. } => {
                    if has_base {
                        return Err(HashSet::from([format!(
                            "a class definition can only have at most one base class declaration and one generic declaration (at {})",
                            b.location
                        )]));
                    }
                    has_base = true;
                    // the function parse_ast_to make sure that no type var occurred in
                    // bast_ty if it is a CustomClassKind
                    let base_ty = parse_ast_to_type_annotation_kinds(
                        class_resolver,
                        temp_def_list,
                        builtin_registry,
                        unifier,
                        primitives_store,
                        b,
                        vec![(*class_def_id, class_type_vars.clone())]
                            .into_iter()
                            .collect::<HashMap<_, _>>(),
                    )?;
                    if let TypeAnnotation::CustomClass { .. } = &base_ty {
                        class_ancestors.push(base_ty);
                    } else {
                        return Err(HashSet::from([format!(
                            "class base declaration can only be custom class (at {})",
                            b.location
                        )]));
                    }
                }
                _ => {
                    return Err(HashSet::from([format!(
                        "unsupported statement in class defintion (at {})",
                        b.location
                    )]));
                }
            }
        }

        Ok(())
    }

    /// gets all ancestors of a class
    pub fn analyze_class_ancestors(
        class_def: &Arc<RwLock<TopLevelDef>>,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
    ) {
        // Check if class has a direct parent
        let TopLevelDef::Class { ancestors, type_vars, object_id, .. } = &mut *class_def.write()
        else {
            unreachable!()
        };
        let mut anc_set = HashMap::new();

        if let Some(ancestor) = ancestors.first() {
            let TypeAnnotation::CustomClass { id, .. } = ancestor else { unreachable!() };
            let TopLevelDef::Class { ancestors: parent_ancestors, .. } =
                &*temp_def_list[id.0].read()
            else {
                unreachable!()
            };
            for anc in parent_ancestors.iter().skip(1) {
                let TypeAnnotation::CustomClass { id, .. } = anc else { unreachable!() };
                anc_set.insert(id, anc.clone());
            }
            ancestors.extend(anc_set.into_values());
        }
        // push `self` as first ancestor of class
        ancestors.insert(0, make_self_type_annotation(type_vars.as_slice(), *object_id));
    }
}

pub fn parse_parameter_default_value(
    default: &ast::Expr,
    resolver: &(dyn SymbolResolver + Send + Sync),
    builtin_registry: &Arc<dyn BuiltinRegistry>,
) -> Result<SymbolValue, HashSet<String>> {
    fn handle_constant(val: &Constant, loc: &Location) -> Result<SymbolValue, HashSet<String>> {
        match val {
            Constant::Int(v) => (*v).try_into().map_or_else(
                |_| Err(HashSet::from([format!("integer value out of range at {loc}")])),
                |v| Ok(SymbolValue::I32(v)),
            ),
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
        ast::ExprKind::Call { func, args, .. } if args.len() == 1 => {
            let builtin = builtin_registry.match_builtin(func);
            match builtin {
                Some(PrimDef::Int64) => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        (*v).try_into().map_or_else(
                            |_| {
                                Err(HashSet::from([format!(
                                    "default param value out of range at {}",
                                    default.location
                                )]))
                            },
                            |v| Ok(SymbolValue::I64(v)),
                        )
                    }
                    _ => Err(HashSet::from([format!(
                        "only allow constant integer here at {}",
                        default.location
                    )])),
                },
                Some(PrimDef::UInt32) => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        (*v).try_into().map_or_else(
                            |_| {
                                Err(HashSet::from([format!(
                                    "default param value out of range at {}",
                                    default.location
                                )]))
                            },
                            |v| Ok(SymbolValue::U32(v)),
                        )
                    }
                    _ => Err(HashSet::from([format!(
                        "only allow constant integer here at {}",
                        default.location
                    )])),
                },
                Some(PrimDef::UInt64) => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        (*v).try_into().map_or_else(
                            |_| {
                                Err(HashSet::from([format!(
                                    "default param value out of range at {}",
                                    default.location
                                )]))
                            },
                            |v| Ok(SymbolValue::U64(v)),
                        )
                    }
                    _ => Err(HashSet::from([format!(
                        "only allow constant integer here at {}",
                        default.location
                    )])),
                },
                Some(PrimDef::FunSome) => Ok(SymbolValue::OptionSome(Box::new(
                    parse_parameter_default_value(&args[0], resolver, builtin_registry)?,
                ))),
                _ => Err(HashSet::from([format!(
                    "unsupported default parameter at {}",
                    default.location
                )])),
            }
        }
        ast::ExprKind::Tuple { elts, .. } => Ok(SymbolValue::Tuple(
            elts.iter()
                .map(|x| parse_parameter_default_value(x, resolver, builtin_registry))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        ast::ExprKind::Name { .. }
            if builtin_registry.match_builtin(default) == Some(PrimDef::None) =>
        {
            Ok(SymbolValue::OptionNone)
        }
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

/// Retrieves flags from a decorator, if any.
/// This function takes in:
/// * `decorator`: A reference to a `Located<ExprKind>` representing the decorator expression.
#[must_use]
pub fn get_decorator_flags(decorator: &Located<ExprKind>) -> Vec<Constant> {
    let mut flags = vec![];
    if let ExprKind::Call { keywords, .. } = &decorator.node {
        for keyword in keywords {
            if keyword.node.arg != Some("flags".into()) {
                continue;
            }
            if let ExprKind::Set { elts } = &keyword.node.value.node {
                for elt in elts {
                    if let ExprKind::Constant { value, .. } = &elt.node {
                        flags.push(value.clone());
                    }
                }
            }
        }
    }
    flags
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
                todo!(
                    "Getting num of dimensions for ndarray with more than one ndim bound is unimplemented"
                )
            }

            u64::try_from(values[0].clone()).unwrap()
        }

        TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
            arraylike_get_ndims(unifier, iter_type_vars(params).next().unwrap().ty) + 1
        }
        _ => 0,
    }
}

/// Extract an ndarray's `ndims` [type][`Type`] in `u64`. Panic if not possible.
/// The `ndims` must only contain 1 value.
#[must_use]
pub fn extract_ndims(unifier: &Unifier, ndims_ty: Type) -> u64 {
    let ndims_ty_enum = unifier.get_ty_immutable(ndims_ty);
    let TypeEnum::TLiteral { values, .. } = &*ndims_ty_enum else {
        panic!("ndims_ty should be a TLiteral");
    };

    assert_eq!(values.len(), 1, "ndims_ty TLiteral should only contain 1 value");

    let ndims = values[0].clone();
    u64::try_from(ndims).unwrap()
}

/// Return an ndarray's `ndims` as a typechecker [`Type`] from its `u64` value.
pub fn create_ndims(unifier: &mut Unifier, ndims: u64) -> Type {
    unifier.get_fresh_literal(vec![SymbolValue::U64(ndims)], None)
}
