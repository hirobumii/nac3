use std::{
    collections::{HashMap, HashSet},
    fmt,
    rc::Rc,
    sync::Arc,
};

use anyhow::anyhow;
use indexmap::IndexMap;
use itertools::Itertools as _;
use nac3parser::ast::{self, Expr, ExprKind, FileName, Located, StrRef, fold::Fold};
use parking_lot::RwLock;

use crate::{
    codegen::{expr::get_subst_key, stmt::exn_constructor},
    symbol_resolver::SymbolValue,
    toplevel::{
        DefinitionId, FunInstance, GenCall, Location, Stmt, SymbolResolver, TopLevelContext,
        TopLevelDef, builtins, get_type_from_type_annotation_kinds,
        get_type_var_contained_in_type_annotation, helper::PrimDef, make_self_type_annotation,
        parse_ast_to_type_annotation_kinds, type_annotation::TypeAnnotation,
    },
    typecheck::{
        type_inferencer::{CodeLocation, FunctionData, Inferencer, PrimitiveStore},
        typedef::{CallId, FunSignature, FuncArg, Type, TypeEnum, TypeVar, Unifier, VarMap},
    },
};

/// Default implementation of [`BuiltinRegistry`] using string-based matching.
///
/// This zero-sized struct provides the standard builtin matching behavior
/// for standalone mode. Use `DefaultBuiltinRegistry` when you need a simple
/// builtin registry without custom matching logic.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultBuiltinRegistry;

impl BuiltinRegistry for DefaultBuiltinRegistry {}

/// Trait for matching AST expressions against builtin identifiers.
pub trait BuiltinRegistry: Send + Sync {
    /// Match an AST expression against known builtin identifiers.
    ///
    /// Returns `Some(PrimDef)` if the expression matches a recognized builtin,
    /// otherwise returns `None`.
    ///
    /// # Arguments
    /// * `expr` - The AST expression to match
    fn match_builtin(&self, expr: &Located<ExprKind>) -> Option<PrimDef> {
        let get_name = |e: &ExprKind| match e {
            ExprKind::Name { id, .. } => Some(id.to_string()),
            ExprKind::Subscript { value, .. } => {
                if let ExprKind::Name { id, .. } = &value.node {
                    Some(id.to_string())
                } else {
                    None
                }
            }
            _ => None,
        };

        let name = get_name(&expr.node)?;

        Some(match name.as_str() {
            // Core primitives
            "float" => PrimDef::Float,
            "bool" => PrimDef::Bool,
            "str" => PrimDef::Str,
            "list" => PrimDef::List,
            "tuple" => PrimDef::Tuple,
            "Exception" => PrimDef::Exception,

            // Core functions
            "range" => PrimDef::Range,
            "enumerate" => PrimDef::Enumerate,
            "round" => PrimDef::FunRound,
            "round64" => PrimDef::FunRound64,
            "floor" => PrimDef::FunFloor,
            "floor64" => PrimDef::FunFloor64,
            "ceil" => PrimDef::FunCeil,
            "ceil64" => PrimDef::FunCeil64,
            "len" => PrimDef::FunLen,
            "min" => PrimDef::FunMin,
            "max" => PrimDef::FunMax,
            "abs" => PrimDef::FunAbs,
            "Some" => PrimDef::FunSome,
            "staticmethod" => PrimDef::StaticMethod,

            // Type qualifier
            "Auto" => PrimDef::Auto,
            "Kernel" => PrimDef::Kernel,
            "KernelInvariant" => PrimDef::KernelInvariant,
            "ConstGeneric" => PrimDef::ConstGeneric,
            "none" => PrimDef::None,
            "virtual" => PrimDef::Virtual,
            "Option" => PrimDef::Option,

            // Decorators
            "compile" => PrimDef::Compile,
            "extern" => PrimDef::ExternFn,
            "kernel" => PrimDef::KernelDecorator,
            "portable" => PrimDef::Portable,
            "rpc" => PrimDef::Rpc,

            // Typing
            "Generic" => PrimDef::Generic,
            "Literal" => PrimDef::Literal,

            // NumPy
            "int32" => PrimDef::Int32,
            "int64" => PrimDef::Int64,
            "uint32" => PrimDef::UInt32,
            "uint64" => PrimDef::UInt64,

            "np_ndarray" | "ndarray" => PrimDef::NDArray,
            "np_empty" => PrimDef::FunNpEmpty,
            "np_zeros" => PrimDef::FunNpZeros,
            "np_ones" => PrimDef::FunNpOnes,
            "np_full" => PrimDef::FunNpFull,
            "np_array" => PrimDef::FunNpArray,
            "np_eye" => PrimDef::FunNpEye,
            "np_identity" => PrimDef::FunNpIdentity,

            "np_size" => PrimDef::FunNpSize,
            "np_shape" => PrimDef::FunNpShape,
            "np_strides" => PrimDef::FunNpStrides,

            "np_broadcast_to" => PrimDef::FunNpBroadcastTo,
            "np_transpose" => PrimDef::FunNpTranspose,
            "np_reshape" => PrimDef::FunNpReshape,

            "np_round" => PrimDef::FunNpRound,
            "np_floor" => PrimDef::FunNpFloor,
            "np_ceil" => PrimDef::FunNpCeil,
            "np_min" => PrimDef::FunNpMin,
            "np_minimum" => PrimDef::FunNpMinimum,
            "np_max" => PrimDef::FunNpMax,
            "np_maximum" => PrimDef::FunNpMaximum,
            "np_argmin" => PrimDef::FunNpArgmin,
            "np_argmax" => PrimDef::FunNpArgmax,
            "np_isnan" => PrimDef::FunNpIsNan,
            "np_isinf" => PrimDef::FunNpIsInf,
            "np_sin" => PrimDef::FunNpSin,
            "np_cos" => PrimDef::FunNpCos,
            "np_exp" => PrimDef::FunNpExp,
            "np_exp2" => PrimDef::FunNpExp2,
            "np_log" => PrimDef::FunNpLog,
            "np_log10" => PrimDef::FunNpLog10,
            "np_log2" => PrimDef::FunNpLog2,
            "np_fabs" => PrimDef::FunNpFabs,
            "np_sqrt" => PrimDef::FunNpSqrt,
            "np_rint" => PrimDef::FunNpRint,
            "np_tan" => PrimDef::FunNpTan,
            "np_arcsin" => PrimDef::FunNpArcsin,
            "np_arccos" => PrimDef::FunNpArccos,
            "np_arctan" => PrimDef::FunNpArctan,
            "np_sinh" => PrimDef::FunNpSinh,
            "np_cosh" => PrimDef::FunNpCosh,
            "np_tanh" => PrimDef::FunNpTanh,
            "np_arcsinh" => PrimDef::FunNpArcsinh,
            "np_arccosh" => PrimDef::FunNpArccosh,
            "np_arctanh" => PrimDef::FunNpArctanh,
            "np_expm1" => PrimDef::FunNpExpm1,
            "np_cbrt" => PrimDef::FunNpCbrt,
            "sp_spec_erf" => PrimDef::FunSpSpecErf,
            "sp_spec_erfc" => PrimDef::FunSpSpecErfc,
            "sp_spec_gamma" => PrimDef::FunSpSpecGamma,
            "sp_spec_gammaln" => PrimDef::FunSpSpecGammaln,
            "sp_spec_j0" => PrimDef::FunSpSpecJ0,
            "sp_spec_j1" => PrimDef::FunSpSpecJ1,
            "np_arctan2" => PrimDef::FunNpArctan2,
            "np_copysign" => PrimDef::FunNpCopysign,
            "np_fmax" => PrimDef::FunNpFmax,
            "np_fmin" => PrimDef::FunNpFmin,
            "np_ldexp" => PrimDef::FunNpLdExp,
            "np_hypot" => PrimDef::FunNpHypot,
            "np_nextafter" => PrimDef::FunNpNextAfter,
            "np_any" => PrimDef::FunNpAny,
            "np_all" => PrimDef::FunNpAll,

            "np_dot" => PrimDef::FunNpDot,
            "np_linalg_cholesky" => PrimDef::FunNpLinalgCholesky,
            "np_linalg_qr" => PrimDef::FunNpLinalgQr,
            "np_linalg_svd" => PrimDef::FunNpLinalgSvd,
            "np_linalg_inv" => PrimDef::FunNpLinalgInv,
            "np_linalg_pinv" => PrimDef::FunNpLinalgPinv,
            "np_linalg_matrix_power" => PrimDef::FunNpLinalgMatrixPower,
            "np_linalg_det" => PrimDef::FunNpLinalgDet,
            "sp_linalg_lu" => PrimDef::FunSpLinalgLu,
            "sp_linalg_schur" => PrimDef::FunSpLinalgSchur,
            "sp_linalg_hessenberg" => PrimDef::FunSpLinalgHessenberg,

            _ => return None,
        })
    }

    /// Checks whether the type annotation expression `type_ann` indicates that a class contains
    /// generic types in its members, usually `Generic[T]`.
    ///
    /// The type annotation is resolved in the decorator's global module context.
    fn has_generic_ann(&self, type_ann: &Located<ExprKind>) -> Result<bool, BuiltinMatchError> {
        Ok(self.match_builtin(type_ann) == Some(PrimDef::Generic))
    }

    /// Checks whether the type annotation expression `type_ann` indicates that the variable is
    /// mutable, usually `Kernel[T]`.
    ///
    /// The type annotation is resolved in the decorator's global module context.
    ///
    /// Returns `Ok(None)` if this functionality is not supported.
    fn has_kernel_ann(&self, type_ann: &Located<ExprKind>) -> Result<bool, BuiltinMatchError> {
        Ok(self.match_builtin(type_ann) == Some(PrimDef::Kernel))
    }

    /// Checks whether the type annotation expression `type_ann` indicates that the variable is
    /// immutable, usually `KernelInvariant[T]`.
    ///
    /// The type annotation is resolved in the decorator's global module context.
    fn has_invariant_ann(&self, type_ann: &Located<ExprKind>) -> Result<bool, BuiltinMatchError> {
        Ok(self.match_builtin(type_ann) == Some(PrimDef::KernelInvariant))
    }

    /// Checks whether the `decorator` indicates that the function should be an `extern` function,
    /// usually `@extern`.
    ///
    /// An `extern` function is a function that is only declared in the compiled Python binary and
    /// whose implementation is defined elsewhere, such as compiler builtins or functions that are
    /// executed on the host interpreter.
    ///
    /// The decorator is resolved in the decorator's global module context.
    fn is_extern_decorator(
        &self,
        decorator: &Located<ExprKind>,
    ) -> Result<bool, BuiltinMatchError> {
        Ok(self.match_builtin(decorator) == Some(PrimDef::ExternFn)
            || self.match_builtin(decorator) == Some(PrimDef::Rpc))
    }

    /// Checks whether the `decorator` indicates that the function is a static method, usually the
    /// default python `@staticmethod` decorator. These are function that do no take `self` as an
    /// argument, and can be called without instantiating the class.
    ///
    /// The decorator is resolved in the decorator's global module context.
    fn is_static_method_decorator(
        &self,
        decorator: &Located<ExprKind>,
    ) -> Result<bool, BuiltinMatchError> {
        Ok(self.match_builtin(decorator) == Some(PrimDef::StaticMethod))
    }

    /// Returns true if kernel decorators are supported (ARTIQ mode).
    /// Returns false for standalone mode.
    fn supports_kernel_decorators(&self) -> bool {
        false
    }
}

/// Errors that can occur during builtin identifier matching
#[derive(Debug, Clone)]
pub enum BuiltinMatchError {
    ModuleNotFound { file: FileName },
    PythonError(String),
    ResolutionError(String),
}

impl fmt::Display for BuiltinMatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleNotFound { file } => {
                write!(f, "No module found for file {file:?}")
            }
            Self::PythonError(err) => {
                write!(f, "Python error: {err}")
            }
            Self::ResolutionError(err) => {
                write!(f, "Resolution error: {err}")
            }
        }
    }
}

impl std::error::Error for BuiltinMatchError {}

/// Converts a typed expression `Located<ExprKind<U>, U>` to an untyped expression `Located<ExprKind>`.
///
/// This function recursively erases type information from the expression tree while preserving
/// the structure, location information, and identifiers needed for builtin matching.
pub fn erase_expr_type<U>(expr: &Located<ExprKind<U>, U>) -> Located<ExprKind> {
    Located {
        location: expr.location,
        custom: (),
        node: match &expr.node {
            ExprKind::Name { id, ctx } => ExprKind::Name { id: *id, ctx: *ctx },
            ExprKind::Subscript { value, slice, ctx } => ExprKind::Subscript {
                value: Box::new(erase_expr_type(value)),
                slice: Box::new(erase_expr_type(slice)),
                ctx: *ctx,
            },
            ExprKind::Attribute { value, attr, ctx } => ExprKind::Attribute {
                value: Box::new(erase_expr_type(value)),
                attr: *attr,
                ctx: *ctx,
            },
            ExprKind::Call { func, args, keywords } => ExprKind::Call {
                func: Box::new(erase_expr_type(func)),
                args: args.iter().map(erase_expr_type).collect(),
                keywords: keywords
                    .iter()
                    .map(|kw| ast::Located {
                        location: kw.location,
                        custom: (),
                        node: ast::KeywordData {
                            arg: kw.node.arg,
                            value: Box::new(erase_expr_type(&kw.node.value)),
                        },
                    })
                    .collect(),
            },
            ExprKind::Tuple { elts, ctx } => {
                ExprKind::Tuple { elts: elts.iter().map(erase_expr_type).collect(), ctx: *ctx }
            }
            ExprKind::Constant { value, kind } => {
                ExprKind::Constant { value: value.clone(), kind: kind.clone() }
            }
            _ => ExprKind::Constant { value: ast::Constant::None, kind: None },
        },
    }
}

/// Converts an untyped expression `Located<ExprKind>` to a typed expression `Located<ExprKind<Option<Type>>, Option<Type>>`.
pub fn promote_expr_type(
    expr: &Located<ExprKind>,
) -> Located<ExprKind<Option<Type>>, Option<Type>> {
    Located {
        location: expr.location,
        custom: None,
        node: match &expr.node {
            ExprKind::Name { id, ctx } => ExprKind::Name { id: *id, ctx: *ctx },
            ExprKind::Subscript { value, slice, ctx } => ExprKind::Subscript {
                value: Box::new(promote_expr_type(value)),
                slice: Box::new(promote_expr_type(slice)),
                ctx: *ctx,
            },
            ExprKind::Attribute { value, attr, ctx } => ExprKind::Attribute {
                value: Box::new(promote_expr_type(value)),
                attr: *attr,
                ctx: *ctx,
            },
            ExprKind::Call { func, args, keywords } => ExprKind::Call {
                func: Box::new(promote_expr_type(func)),
                args: args.iter().map(promote_expr_type).collect(),
                keywords: keywords
                    .iter()
                    .map(|kw| ast::Located {
                        location: kw.location,
                        custom: None,
                        node: ast::KeywordData {
                            arg: kw.node.arg,
                            value: Box::new(promote_expr_type(&kw.node.value)),
                        },
                    })
                    .collect(),
            },
            ExprKind::Tuple { elts, ctx } => {
                ExprKind::Tuple { elts: elts.iter().map(promote_expr_type).collect(), ctx: *ctx }
            }
            ExprKind::Constant { value, kind } => {
                ExprKind::Constant { value: value.clone(), kind: kind.clone() }
            }
            _ => ExprKind::Constant { value: ast::Constant::None, kind: None },
        },
    }
}

pub type DefAst = (Arc<RwLock<TopLevelDef>>, Option<Stmt<()>>);
pub struct TopLevelComposer {
    // list of top level definitions, same as top level context
    pub definition_ast_list: Vec<DefAst>,
    // start as a primitive unifier, will add more top_level defs inside
    pub unifier: Unifier,
    // primitive store
    pub primitives_ty: PrimitiveStore,
    // to prevent duplicate definition
    pub defined_names: HashSet<String>,
    // get the class def id of a class method
    pub method_class: HashMap<DefinitionId, DefinitionId>,
    // number of built-in function and classes in the definition list, later skip
    pub builtin_num: usize,
    /// Registry for builtin identifiers.
    pub builtin_registry: Arc<dyn BuiltinRegistry>,
    /// The size of a native word on the target platform.
    pub size_t: u32,
}

/// The specification for a builtin function, consisting of the function name, the function
/// signature, and a [code generation callback][`GenCall`].
pub type BuiltinFuncSpec = (StrRef, FunSignature, Arc<GenCall>);

/// A function that creates a [`BuiltinFuncSpec`] using the provided [`PrimitiveStore`] and
/// [`Unifier`].
pub type BuiltinFuncCreator = dyn Fn(&PrimitiveStore, &mut Unifier) -> BuiltinFuncSpec;

impl TopLevelComposer {
    /// return a composer and things to make a "primitive" symbol resolver, so that the symbol
    /// resolver can later figure out primitive tye definitions when passed a primitive type name
    ///
    /// `lateinit_builtins` are specifically for the ARTIQ module. Since the [`Unifier`] instance
    /// used to create builtin functions do not persist until method compilation, any types
    /// created (e.g. [`TypeEnum::TVar`]) also do not persist. Those functions should be instead put
    /// in `lateinit_builtins`, where they will be instantiated with the [`Unifier`] instance used
    /// for method compilation.
    #[must_use]
    pub fn new(
        builtins: Vec<BuiltinFuncSpec>,
        lateinit_builtins: Vec<Box<BuiltinFuncCreator>>,
        builtin_registry: Arc<dyn BuiltinRegistry>,
        size_t: u32,
    ) -> (Self, HashMap<StrRef, DefinitionId>, HashMap<StrRef, Type>) {
        let (primitives_ty, mut unifier) = Self::make_primitives(size_t);
        let mut definition_ast_list = builtins::get_builtins(&mut unifier, &primitives_ty);
        let defined_names = HashSet::default();
        let method_class = HashMap::default();

        let mut builtin_id = HashMap::default();
        let mut builtin_ty = HashMap::default();

        // Materialize lateinit_builtins, now that the unifier is ready
        let lateinit_builtins = lateinit_builtins
            .into_iter()
            .map(|builtin| builtin(&primitives_ty, &mut unifier))
            .collect_vec();

        for (name, sig, codegen_callback) in builtins.into_iter().chain(lateinit_builtins) {
            let fun_sig = unifier.add_ty(TypeEnum::TFunc(sig));
            builtin_ty.insert(name, fun_sig);
            builtin_id.insert(name, DefinitionId(definition_ast_list.len()));
            definition_ast_list.push((
                Arc::new(RwLock::new(TopLevelDef::Function {
                    name: name.into(),
                    simple_name: name,
                    signature: fun_sig,
                    instance_to_stmt: HashMap::default(),
                    instance_to_symbol: HashMap::default(),
                    var_id: Vec::default(),
                    attributes: Vec::default(),
                    resolver: None,
                    codegen_callback: Some(codegen_callback),
                    loc: None,
                })),
                None,
            ));
        }

        (
            Self {
                builtin_num: definition_ast_list.len(),
                definition_ast_list,
                primitives_ty,
                unifier,
                defined_names,
                method_class,
                builtin_registry,
                size_t,
            },
            builtin_id,
            builtin_ty,
        )
    }

    #[must_use]
    pub fn make_top_level_context(&self) -> TopLevelContext {
        TopLevelContext {
            definitions: RwLock::new(
                self.definition_ast_list.iter().map(|(x, ..)| x.clone()).collect_vec(),
            )
            .into(),
            // NOTE: only one for now
            unifiers: Arc::new(RwLock::new(vec![(
                self.unifier.get_shared_unifier(),
                self.primitives_ty,
            )])),
            personality_symbol: Some("__nac3_personality".into()),
            builtin_registry: self.builtin_registry.clone(),
        }
    }

    #[must_use]
    pub fn extract_def_list(&self) -> Vec<Arc<RwLock<TopLevelDef>>> {
        self.definition_ast_list.iter().map(|(def, ..)| def.clone()).collect_vec()
    }

    /// register top level modules
    pub fn register_top_level_module(
        &mut self,
        module_name: &str,
        name_to_pyid: &Rc<HashMap<StrRef, u64>>,
        resolver: Arc<dyn SymbolResolver + Send + Sync>,
        location: Option<Location>,
    ) -> Result<DefinitionId, String> {
        let mut classes: HashMap<StrRef, DefinitionId> = HashMap::new();
        let mut methods: HashMap<StrRef, DefinitionId> = HashMap::new();

        for (name, _) in name_to_pyid.iter() {
            if let Ok(def_id) = resolver.get_identifier_def(*name) {
                // Avoid repeated attribute instances resulting from multiple imports of same module
                if self.defined_names.contains(&format!("{module_name}.{name}")) {
                    match &*self.definition_ast_list[def_id.0].0.read() {
                        TopLevelDef::Class { .. } => {
                            classes.insert(*name, def_id);
                        }
                        TopLevelDef::Function { .. } => {
                            methods.insert(*name, def_id);
                        }
                        TopLevelDef::Module { .. } => {
                            unreachable!("modules cannot be nested inside another module")
                        }
                    }
                }
            }
        }
        let module_def = TopLevelDef::Module {
            name: module_name.to_string().into(),
            simple_name: module_name
                .rsplit_once('.')
                .map_or(module_name, |(_, nme)| nme)
                .to_string(),
            module_id: DefinitionId(self.definition_ast_list.len()),
            classes: classes.into_iter().collect(),
            functions: methods.into_iter().collect(),
            resolver: Some(resolver),
            loc: location,
        };

        self.definition_ast_list.push((Arc::new(RwLock::new(module_def)), None));
        Ok(DefinitionId(self.definition_ast_list.len() - 1))
    }

    /// register, just remember the names of top level classes/function
    /// and check duplicate class/method/function definition
    pub fn register_top_level(
        &mut self,
        ast: Stmt<()>,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        mod_path: &str,
        allow_no_constructor: bool,
    ) -> Result<(StrRef, DefinitionId, Option<Type>), String> {
        type MethodInfo = (
            // the simple method name without class name
            StrRef,
            // in this top level def, method name is prefixed with the class name
            Arc<RwLock<TopLevelDef>>,
            DefinitionId,
            Type,
            Stmt<()>,
        );

        let defined_names = &mut self.defined_names;
        match &ast.node {
            ast::StmtKind::ClassDef { name: class_name, bases, body, .. } => {
                let fully_qualified_class_name = if mod_path.is_empty() {
                    *class_name
                } else {
                    format!("{}.{}", &mod_path, class_name).into()
                };
                if !defined_names.insert(fully_qualified_class_name.into()) {
                    return Err(format!(
                        "duplicate definition of class `{}` (at {})",
                        class_name, ast.location
                    ));
                }

                let class_name = *class_name;
                let class_def_id = self.definition_ast_list.len();

                // since later when registering class method, ast will still be used,
                // here push None temporarily, later will move the ast inside
                let constructor_ty = self.unifier.get_dummy_var().ty;
                let mut class_def_ast = (
                    Arc::new(RwLock::new(Self::make_top_level_class_def(
                        DefinitionId(class_def_id),
                        resolver.clone(),
                        fully_qualified_class_name,
                        Some(constructor_ty),
                        Some(ast.location),
                    ))),
                    None,
                );

                // parse class def body and register class methods into the def list.
                // module's symbol resolver would not know the name of the class methods,
                // thus cannot return their definition_id
                let mut class_method_name_def_ids: Vec<MethodInfo> = Vec::new();
                // we do not push anything to the def list, so we keep track of the index
                // and then push in the correct order after the for loop
                let mut class_method_index_offset = 0;
                let init_id = "__init__".into();
                // TODO: Fix this hack. We will generate constructor for classes that inherit
                // from Exception class (directly or indirectly), but this code cannot handle
                // subclass of other exception classes.
                let mut contains_constructor = bases.iter().any(|base| {
                    self.builtin_registry.match_builtin(base) == Some(PrimDef::Exception)
                });
                for b in body {
                    if let ast::StmtKind::FunctionDef {
                        name: method_name, decorator_list, ..
                    } = &b.node
                    {
                        if method_name == &init_id {
                            contains_constructor = true;
                        }
                        let global_class_method_name = Self::make_class_method_name(
                            fully_qualified_class_name.into(),
                            &method_name.to_string(),
                        );
                        if !defined_names.insert(global_class_method_name.clone()) {
                            return Err(format!(
                                "class method `{}` defined twice (at {})",
                                global_class_method_name, b.location
                            ));
                        }
                        let method_def_id = self.definition_ast_list.len() + {
                            // plus 1 here since we already have the class def
                            class_method_index_offset += 1;
                            class_method_index_offset
                        };

                        // dummy method define here
                        let dummy_method_type = self.unifier.get_dummy_var().ty;
                        let mut attributes = vec![];
                        if decorator_list.iter().any(|d| {
                            self.builtin_registry.is_static_method_decorator(d).unwrap_or(false)
                        }) {
                            attributes.push(super::FunAttribute::StaticMethod);
                        }
                        class_method_name_def_ids.push((
                            *method_name,
                            RwLock::new(Self::make_top_level_function_def(
                                global_class_method_name,
                                *method_name,
                                // later unify with parsed type
                                dummy_method_type,
                                attributes,
                                resolver.clone(),
                                Some(b.location),
                            ))
                            .into(),
                            DefinitionId(method_def_id),
                            dummy_method_type,
                            b.clone(),
                        ));
                    }
                }

                // move the ast to the entry of the class in the ast_list
                class_def_ast.1 = Some(ast);
                // get the methods into the top level class_def
                for (name, _, id, ty, ..) in &class_method_name_def_ids {
                    let TopLevelDef::Class { methods, .. } = &mut *class_def_ast.0.write() else {
                        unreachable!()
                    };

                    methods.push((*name, *ty, *id));
                    self.method_class.insert(*id, DefinitionId(class_def_id));
                }
                // now class_def_ast and class_method_def_ast_ids are ok, put them into actual def list in correct order
                self.definition_ast_list.push(class_def_ast);
                for (_, def, _, _, ast) in class_method_name_def_ids {
                    self.definition_ast_list.push((def, Some(ast)));
                }

                let result_ty = if allow_no_constructor || contains_constructor {
                    Some(constructor_ty)
                } else {
                    None
                };
                Ok((class_name, DefinitionId(class_def_id), result_ty))
            }

            ast::StmtKind::FunctionDef { name, .. } => {
                let global_fun_name = if mod_path.is_empty() {
                    name.to_string()
                } else {
                    format!("{mod_path}.{name}")
                };
                if !defined_names.insert(global_fun_name.clone()) {
                    return Err(format!(
                        "top level function `{}` defined twice (at {})",
                        global_fun_name, ast.location
                    ));
                }

                let fun_name = *name;
                let ty_to_be_unified = self.unifier.get_dummy_var().ty;
                // add to the definition list
                self.definition_ast_list.push((
                    RwLock::new(Self::make_top_level_function_def(
                        global_fun_name,
                        *name,
                        // dummy here, unify with correct type later
                        ty_to_be_unified,
                        vec![],
                        resolver,
                        Some(ast.location),
                    ))
                    .into(),
                    Some(ast),
                ));

                // return
                Ok((
                    fun_name,
                    DefinitionId(self.definition_ast_list.len() - 1),
                    Some(ty_to_be_unified),
                ))
            }

            _ => Err(format!(
                "registrations of constructs other than top level classes/functions are not supported (at {})",
                ast.location
            )),
        }
    }

    /// Analyze the AST and modify the corresponding `TopLevelDef`
    pub fn start_analysis(&mut self, inference: bool) -> Result<(), Vec<anyhow::Error>> {
        self.analyze_top_level_class_definition()?;
        self.analyze_top_level_class_fields_methods()?;
        self.analyze_top_level_function()?;
        if inference {
            self.analyze_function_instance()?;
        }
        Ok(())
    }

    /// step 1, analyze the top level class definitions
    ///
    /// Checks for class type variables and ancestors adding them to the `TopLevelDef` list
    fn analyze_top_level_class_definition(&mut self) -> Result<(), Vec<anyhow::Error>> {
        let def_list = &self.definition_ast_list;
        let builtin_registry = &self.builtin_registry;
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;
        let mut errors = Vec::new();

        // Initially only copy the definitions of buitin classes and functions
        // class definitions are added in the same order as they appear in the program
        let mut temp_def_list: Vec<Arc<RwLock<TopLevelDef>>> =
            def_list.iter().take(self.builtin_num).map(|f| f.0.clone()).collect_vec();

        // Check for class generic variables and ancestors
        for (class_def, class_ast) in def_list.iter().skip(self.builtin_num) {
            if class_ast.is_some() && matches!(&*class_def.read(), TopLevelDef::Class { .. }) {
                // Add class type variables and direct parents to the `TopLevelDef`
                if let Err(e) = Self::analyze_class_bases(
                    class_def,
                    class_ast,
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    builtin_registry,
                ) {
                    errors.extend(e);
                }

                // Add class ancestors
                Self::analyze_class_ancestors(class_def, &temp_def_list);

                // special case classes that inherit from Exception
                let TopLevelDef::Class { ancestors: class_ancestors, .. } = &*class_def.read()
                else {
                    unreachable!()
                };

                if class_ancestors
                    .iter()
                    .any(|ann| matches!(ann, TypeAnnotation::CustomClass { id, .. } if *id == PrimDef::Exception.id()))
                {
                    // if inherited from Exception, the body should be a pass
                    let ast::StmtKind::ClassDef { body, .. } = &class_ast.as_ref().unwrap().node
                    else {
                        unreachable!()
                    };
                    for stmt in body {
                        if matches!(
                            stmt.node,
                            ast::StmtKind::FunctionDef { .. } | ast::StmtKind::AnnAssign { .. }
                        ) {
                            errors.push(anyhow!("Classes inherited from exception should have no custom fields/methods"));
                        }
                    }
                }
            }
            temp_def_list.push(class_def.clone());
        }

        // deal with ancestors of Exception object
        let exception_id = PrimDef::Exception.id();
        let TopLevelDef::Class { name, ancestors, object_id, .. } =
            &mut *def_list[exception_id.0].0.write()
        else {
            unreachable!()
        };
        assert_eq!(*name, "Exception".into());
        ancestors.push(make_self_type_annotation(&[], *object_id));

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    /// step 2, class fields and methods
    fn analyze_top_level_class_fields_methods(&mut self) -> Result<(), Vec<anyhow::Error>> {
        // Allow resolving definition IDs in error messages
        if self.unifier.top_level.is_none() {
            let ctx = Arc::new(self.make_top_level_context());
            self.unifier.top_level = Some(ctx);
        }

        let def_list = &self.definition_ast_list;
        let temp_def_list = self.extract_def_list();
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;

        let mut errors: Vec<anyhow::Error> = Vec::new();
        let mut type_var_to_concrete_def: HashMap<Type, TypeAnnotation> = HashMap::new();

        for (class_def, class_ast) in def_list.iter().skip(self.builtin_num) {
            if class_ast.is_some() && matches!(&*class_def.read(), TopLevelDef::Class { .. }) {
                // Collect new entries from this class into a temporary map
                let mut new_entries: HashMap<Type, TypeAnnotation> = HashMap::new();
                if let Err(e) = Self::analyze_single_class_methods_fields(
                    class_def,
                    &class_ast.as_ref().unwrap().node,
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    &mut new_entries,
                    &self.builtin_registry,
                ) {
                    errors.extend(e);
                }

                // Merge new entries into the main map
                type_var_to_concrete_def.extend(new_entries.iter().map(|(k, v)| (*k, v.clone())));

                // The errors need to be reported before copying methods from parent to child classes
                if !errors.is_empty() {
                    return Err(errors);
                }

                let ancestor_count =
                    if let TopLevelDef::Class { ancestors, .. } = &*class_def.read() {
                        ancestors.len()
                    } else {
                        unreachable!()
                    };
                // Methods/fields needs to be processed only if class inherits from another class
                if ancestor_count > 1
                    && let Err(e) = Self::analyze_single_class_ancestors(
                        &mut class_def.write(),
                        &temp_def_list,
                        unifier,
                        primitives_store,
                        &type_var_to_concrete_def,
                    )
                {
                    errors.extend(e);
                }

                let mut subst_list = Some(Vec::new());
                for (ty, def) in &new_entries {
                    match get_type_from_type_annotation_kinds(
                        &temp_def_list,
                        unifier,
                        primitives_store,
                        def,
                        &mut subst_list,
                    ) {
                        Ok(target_ty) => {
                            if let Err(e) = unifier.unify(*ty, target_ty) {
                                errors.push(anyhow!("{}", e.to_display(unifier)));
                            }
                        }
                        Err(e) => {
                            errors.extend(e);
                        }
                    }
                }
                for ty in subst_list.unwrap() {
                    let TypeEnum::TObj { obj_id, params, fields } = &*unifier.get_ty(ty) else {
                        unreachable!()
                    };

                    let mut new_fields = HashMap::new();
                    let mut need_subst = false;
                    for (name, (ty, mutable)) in fields {
                        let substituted = unifier.subst(*ty, params);
                        need_subst |= substituted.is_some();
                        new_fields.insert(*name, (substituted.unwrap_or(*ty), *mutable));
                    }
                    if need_subst {
                        let new_ty = unifier.add_ty(TypeEnum::TObj {
                            obj_id: *obj_id,
                            params: params.clone(),
                            fields: new_fields,
                        });
                        if let Err(e) = unifier.unify(ty, new_ty) {
                            errors.push(anyhow!("{}", e.to_display(unifier)));
                        }
                    }
                }
            }
        }

        {
            let mut subst_list = Some(Vec::new());
            for (ty, def) in &type_var_to_concrete_def {
                match get_type_from_type_annotation_kinds(
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    def,
                    &mut subst_list,
                ) {
                    Ok(target_ty) => {
                        if let Err(e) = unifier.unify(*ty, target_ty) {
                            errors.push(anyhow!("{}", e.to_display(unifier)));
                        }
                    }
                    Err(e) => {
                        errors.extend(e);
                    }
                }
            }
            for ty in subst_list.unwrap() {
                let TypeEnum::TObj { obj_id, params, fields } = &*unifier.get_ty(ty) else {
                    unreachable!()
                };

                let mut new_fields = HashMap::new();
                let mut need_subst = false;
                for (name, (ty, mutable)) in fields {
                    let substituted = unifier.subst(*ty, params);
                    need_subst |= substituted.is_some();
                    new_fields.insert(*name, (substituted.unwrap_or(*ty), *mutable));
                }
                if need_subst {
                    let new_ty = unifier.add_ty(TypeEnum::TObj {
                        obj_id: *obj_id,
                        params: params.clone(),
                        fields: new_fields,
                    });
                    if let Err(e) = unifier.unify(ty, new_ty) {
                        errors.push(anyhow!("{}", e.to_display(unifier)));
                    }
                }
            }
        }

        for (def, _) in def_list.iter().skip(self.builtin_num) {
            match &*def.read() {
                TopLevelDef::Class { resolver: Some(resolver), .. }
                | TopLevelDef::Function { resolver: Some(resolver), .. } => {
                    if let Err(e) =
                        resolver.handle_deferred_eval(unifier, &temp_def_list, primitives_store)
                    {
                        errors.push(e);
                    }
                }
                _ => {}
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    /// step 3, after class methods are done, top level functions have nothing unknown
    fn analyze_top_level_function(&mut self) -> Result<(), Vec<anyhow::Error>> {
        let def_list = &self.definition_ast_list;
        let temp_def_list = self.extract_def_list();
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;

        let mut analyze = |function_def: &Arc<RwLock<TopLevelDef>>, function_ast: &Option<Stmt>| {
            let function_def = &mut *function_def.write();
            let Some(function_ast) = function_ast.as_ref() else {
                // if let TopLevelDef::Function { name, .. } = ``
                return Ok(());
            };

            let TopLevelDef::Function { signature: dummy_ty, resolver, var_id, .. } = function_def
            else {
                // not top level function def, skip
                return Ok(());
            };

            if matches!(unifier.get_ty(*dummy_ty).as_ref(), TypeEnum::TFunc(_)) {
                // already have a function type, is class method, skip
                return Ok(());
            }
            let ast::StmtKind::FunctionDef { args, returns, .. } = &function_ast.node else {
                unreachable!("must be both function");
            };

            let resolver = resolver.as_ref();
            let resolver = resolver.unwrap();
            let resolver = &**resolver;

            let mut function_var_map = VarMap::new();

            let vararg = args
                .vararg
                .as_ref()
                .map(|vararg| -> Result<_, Vec<anyhow::Error>> {
                    let vararg = vararg.as_ref();

                    let annotation = vararg
                        .node
                        .annotation
                        .as_ref()
                        .ok_or_else(|| {
                            vec![anyhow!(
                                "function parameter `{}` needs type annotation at {}",
                                vararg.node.arg,
                                vararg.location
                            )]
                        })?
                        .as_ref();

                    let type_annotation = parse_ast_to_type_annotation_kinds(
                        resolver,
                        temp_def_list.as_slice(),
                        &self.builtin_registry,
                        unifier,
                        primitives_store,
                        annotation,
                        // NOTE: since only class need this, for function
                        // it should be fine to be empty map
                        HashMap::new(),
                    )?;

                    let type_vars_within =
                        get_type_var_contained_in_type_annotation(&type_annotation)
                            .into_iter()
                            .map(|x| -> Result<_, Vec<anyhow::Error>> {
                                let TypeAnnotation::TypeVar(ty) = x else {
                                    unreachable!("must be type var annotation kind")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                Ok::<_, Vec<anyhow::Error>>(TypeVar { id, ty })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    for var in type_vars_within {
                        if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                            // if already have the type inserted, make sure they are the same thing
                            assert_eq!(prev_ty, var.ty);
                        }
                    }

                    let ty = get_type_from_type_annotation_kinds(
                        temp_def_list.as_ref(),
                        unifier,
                        primitives_store,
                        &type_annotation,
                        &mut None,
                    )?;

                    Ok(FuncArg {
                        name: vararg.node.arg,
                        ty,
                        default_value: Some(SymbolValue::Tuple(Vec::default())),
                        is_vararg: true,
                    })
                })
                .transpose()?;

            let mut arg_types = {
                // make sure no duplicate parameter
                let mut defined_parameter_name: HashSet<_> = HashSet::new();
                for x in &args.args {
                    if !defined_parameter_name.insert(x.node.arg) {
                        return Err(vec![anyhow!(
                            "top level function must have unique parameter names \
                            and names should not be the same as the keywords (at {})",
                            x.location
                        )]);
                    }
                }

                let arg_with_default: Vec<(&ast::Located<ast::ArgData<()>>, Option<&Expr>)> = args
                    .args
                    .iter()
                    .rev()
                    .zip(
                        args.defaults
                            .iter()
                            .rev()
                            .map(|x| -> Option<&Expr> { Some(x) })
                            .chain(std::iter::repeat(None)),
                    )
                    .collect_vec();

                arg_with_default
                    .iter()
                    .rev()
                    .map(|(x, default)| -> Result<FuncArg, Vec<anyhow::Error>> {
                        let annotation = x
                            .node
                            .annotation
                            .as_ref()
                            .ok_or_else(|| {
                                vec![anyhow!(
                                    "function parameter `{}` needs type annotation at {}",
                                    x.node.arg,
                                    x.location
                                )]
                            })?
                            .as_ref();

                        let type_annotation = parse_ast_to_type_annotation_kinds(
                            resolver,
                            temp_def_list.as_slice(),
                            &self.builtin_registry,
                            unifier,
                            primitives_store,
                            annotation,
                            // NOTE: since only class need this, for function
                            // it should be fine to be empty map
                            HashMap::new(),
                        )?;

                        let type_vars_within =
                            get_type_var_contained_in_type_annotation(&type_annotation)
                                .into_iter()
                                .map(|x| -> Result<_, Vec<anyhow::Error>> {
                                    let TypeAnnotation::TypeVar(ty) = x else {
                                        unreachable!("must be type var annotation kind")
                                    };

                                    let id = Self::get_var_id(ty, unifier)?;
                                    Ok(TypeVar { id, ty })
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                        for var in type_vars_within {
                            if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                                // if already have the type inserted, make sure they are the same thing
                                assert_eq!(prev_ty, var.ty);
                            }
                        }

                        let ty = get_type_from_type_annotation_kinds(
                            temp_def_list.as_ref(),
                            unifier,
                            primitives_store,
                            &type_annotation,
                            &mut None,
                        )?;

                        Ok(FuncArg {
                            name: x.node.arg,
                            ty,
                            default_value: match default {
                                None => None,
                                Some(default) => Some({
                                    let v = Self::parse_parameter_default_value(
                                        default,
                                        resolver,
                                        &self.builtin_registry,
                                    )?;
                                    Self::check_default_param_type(
                                        &v,
                                        &type_annotation,
                                        primitives_store,
                                        unifier,
                                    )
                                    .map_err(|err| vec![anyhow!("{} (at {})", err, x.location)])?;
                                    v
                                }),
                            },
                            is_vararg: false,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            if let Some(vararg) = vararg {
                arg_types.push(vararg);
            }

            let arg_types = arg_types;

            let return_ty = {
                if let Some(returns) = returns {
                    let return_ty_annotation = {
                        let return_annotation = returns.as_ref();
                        parse_ast_to_type_annotation_kinds(
                            resolver,
                            &temp_def_list,
                            &self.builtin_registry,
                            unifier,
                            primitives_store,
                            return_annotation,
                            // NOTE: since only class need this, for function
                            // it should be fine to be empty map
                            HashMap::new(),
                        )?
                    };

                    let type_vars_within =
                        get_type_var_contained_in_type_annotation(&return_ty_annotation)
                            .into_iter()
                            .map(|x| -> Result<_, Vec<anyhow::Error>> {
                                let TypeAnnotation::TypeVar(ty) = x else {
                                    unreachable!("must be type var here")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                Ok(TypeVar { id, ty })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    for var in type_vars_within {
                        if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                            // if already have the type inserted, make sure they are the same thing
                            assert_eq!(prev_ty, var.ty);
                        }
                    }

                    get_type_from_type_annotation_kinds(
                        &temp_def_list,
                        unifier,
                        primitives_store,
                        &return_ty_annotation,
                        &mut None,
                    )?
                } else {
                    primitives_store.none
                }
            };
            var_id.extend_from_slice(function_var_map
                .iter()
                .filter_map(|(id, ty)| {
                    if matches!(&*unifier.get_ty(*ty), TypeEnum::TVar { range, .. } if range.is_empty()) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect_vec()
                .as_slice()
            );
            let function_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: arg_types,
                ret: return_ty,
                vars: function_var_map,
            }));
            unifier.unify(*dummy_ty, function_ty).map_err(|e| {
                vec![anyhow!("{}", e.at(Some(function_ast.location)).to_display(unifier))]
            })?;
            Ok(())
        };

        let mut errors = Vec::new();
        for (function_def, function_ast) in def_list.iter().skip(self.builtin_num) {
            if function_ast.is_none() {
                continue;
            }
            if let Err(e) = analyze(function_def, function_ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    fn analyze_single_class_methods_fields(
        class_def: &Arc<RwLock<TopLevelDef>>,
        class_ast: &ast::StmtKind<()>,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
        type_var_to_concrete_def: &mut HashMap<Type, TypeAnnotation>,
        builtin_registry: &Arc<dyn BuiltinRegistry>,
    ) -> Result<(), Vec<anyhow::Error>> {
        let TopLevelDef::Class {
            object_id,
            ancestors,
            fields,
            attributes,
            methods,
            resolver,
            type_vars,
            ..
        } = &mut *class_def.write()
        else {
            unreachable!("here must be toplevel class def");
        };
        let ast::StmtKind::ClassDef { name, bases, body, .. } = &class_ast else {
            unreachable!("here must be class def ast")
        };

        let (
            class_id,
            class_name,
            _class_bases_ast,
            class_body_ast,
            _class_ancestor_def,
            class_fields_def,
            class_attributes_def,
            class_methods_def,
            class_type_vars_def,
            class_resolver,
        ) = (
            *object_id, *name, bases, body, ancestors, fields, attributes, methods, type_vars,
            resolver,
        );

        let class_resolver = class_resolver.as_ref().unwrap();
        let class_resolver = class_resolver.as_ref();

        let mut defined_fields: HashSet<_> = HashSet::new();
        for b in class_body_ast {
            match &b.node {
                ast::StmtKind::FunctionDef { args, returns, name, decorator_list, .. } => {
                    let (method_dummy_ty, method_id) = Self::get_class_method_def_info(class_methods_def, *name)?;

                    let mut method_var_map = VarMap::new();

                    let is_static = decorator_list.iter().any(|def| {
                        builtin_registry.is_static_method_decorator(def).unwrap_or(false)
                    });

                    let arg_types: Vec<FuncArg> = {
                        // Function arguments must have:
                        // 1) `self` as first argument (we currently do not support staticmethods)
                        // 2) unique names
                        // 3) names different than keywords
                        match args.args.first() {
                            // `self` must be the first argument XOR the function is static
                            Some(id) if (id.node.arg == "self".into()) ^ is_static => {},
                            None if is_static => {}
                            _ => if is_static {
                                return Err(vec![anyhow!(
                                    "static method {name} cannot take `self` as a parameter (at {})",
                                    b.location
                                )]);
                            } else {
                                return Err(vec![anyhow!(
                                    "{name} method must have a `self` parameter (at {})", b.location
                                )]);
                            },
                        }
                        let mut defined_parameter_name: HashSet<_> = HashSet::new();
                        for arg in args.args.iter().skip(1) {
                            if !defined_parameter_name.insert(arg.node.arg) {
                                return Err(vec![anyhow!("class method must have a unique parameter names (at {})", b.location)]);
                            }
                        }

                        // `self` must not be provided type annotation or default value
                        if !is_static {
                            if args.args.len() == args.defaults.len() {
                                return Err(vec![anyhow!("`self` cannot have a default value (at {})", b.location)]);
                            }
                            if args.args[0].node.annotation.is_some() {
                                return Err(vec![anyhow!("`self` cannot have a type annotation (at {})", b.location)]);
                            }
                        }
                        let mut result = Vec::new();
                        let no_defaults = args.args.len() - args.defaults.len() - usize::from(!is_static);
                        for (idx, x) in args.args.iter().skip(usize::from(!is_static)).enumerate() {
                            let type_ann = {
                                let Some(annotation_expr) = x.node.annotation.as_ref() else {
                                    return Err(vec![anyhow!("type annotation needed for `{}` (at {})", x.node.arg, x.location)]);
                                };
                                parse_ast_to_type_annotation_kinds(
                                    class_resolver,
                                    temp_def_list,
                                    builtin_registry,
                                    unifier,
                                    primitives,
                                    annotation_expr,
                                    vec![(class_id, class_type_vars_def.clone())]
                                        .into_iter()
                                        .collect::<HashMap<_, _>>(),
                                )?
                            };
                            // find type vars within this method parameter type annotation
                            let type_vars_within = get_type_var_contained_in_type_annotation(&type_ann);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(ty) = type_var_within else {
                                    unreachable!("must be type var annotation")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                if let Some(prev_ty) = method_var_map.insert(id, ty) {
                                    // if already in the list, make sure they are the same?
                                    assert_eq!(prev_ty, ty);
                                }
                            }
                            // finish handling type vars
                            let dummy_func_arg = FuncArg {
                                name: x.node.arg,
                                ty: unifier.get_dummy_var().ty,
                                default_value: if idx < no_defaults { None } else {
                                    let default_idx = idx - no_defaults;

                                    Some({
                                        let v = Self::parse_parameter_default_value(&args.defaults[default_idx], class_resolver, builtin_registry)?;
                                        Self::check_default_param_type(&v, &type_ann, primitives, unifier).map_err(|err| vec![anyhow!("{err} (at {})", x.location)])?;
                                        v
                                    })
                                },
                                is_vararg: false,
                            };
                            // push the dummy type and the type annotation
                            // into the list for later unification
                            type_var_to_concrete_def
                                .insert(dummy_func_arg.ty, type_ann.clone());
                            result.push(dummy_func_arg);
                        }
                        result
                    };

                    let ret_type = {
                        if let Some(result) = returns {
                            let result = result.as_ref();
                            let annotation = parse_ast_to_type_annotation_kinds(
                                class_resolver,
                                temp_def_list,
                                builtin_registry,
                                unifier,
                                primitives,
                                result,
                                vec![(class_id, class_type_vars_def.clone())]
                                    .into_iter()
                                    .collect::<HashMap<_, _>>(),
                            )?;
                            // find type vars within this return type annotation
                            let type_vars_within =
                                get_type_var_contained_in_type_annotation(&annotation);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(ty) = type_var_within else {
                                    unreachable!("must be type var annotation");
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                if let Some(prev_ty) = method_var_map.insert(id, ty) {
                                    // if already in the list, make sure they are the same?
                                    assert_eq!(prev_ty, ty);
                                }
                            }
                            let dummy_return_type = unifier.get_dummy_var().ty;
                            type_var_to_concrete_def.insert(dummy_return_type, annotation.clone());
                            dummy_return_type
                        } else {
                            // if do not have return annotation, return none
                            // for uniform handling, still use type annotation
                            let dummy_return_type = unifier.get_dummy_var().ty;
                            type_var_to_concrete_def.insert(
                                dummy_return_type,
                                TypeAnnotation::Primitive(primitives.none),
                            );
                            dummy_return_type
                        }
                    };

                    let TopLevelDef::Function { var_id, .. } =
                        &mut *temp_def_list.get(method_id.0).unwrap().write() else {
                        unreachable!()
                    };
                    var_id.extend_from_slice(method_var_map
                        .iter()
                        .filter_map(|(id, ty)| {
                            if matches!(&*unifier.get_ty(*ty), TypeEnum::TVar { range, .. } if range.is_empty()) {
                                None
                            } else {
                                Some(*id)
                            }
                        })
                        .collect_vec()
                        .as_slice()
                    );
                    let method_type = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: arg_types,
                        ret: ret_type,
                        vars: method_var_map,
                    }));

                    // unify now since function type is not in type annotation define
                    // which should be fine since type within method_type will be subst later
                    unifier
                        .unify(method_dummy_ty, method_type)
                        .map_err(|e| vec![anyhow!("{}", e.to_display(unifier))])?;
                }
                ast::StmtKind::AnnAssign { target, annotation, value, .. } => {
                    if let ExprKind::Name { id: attr, .. } = &target.node {
                        if defined_fields.insert(attr.to_string()) {
                            let dummy_field_type = unifier.get_dummy_var().ty;

                            let annotation = match value {
                                None => {
                                    // handle Kernel[T], KernelInvariant[T]
                                    let (annotation, mutable) = match &annotation.node {
                                        ExprKind::Subscript { slice, .. }
                                            if builtin_registry.has_invariant_ann(annotation).map_err(|err| vec![anyhow!("{err}")])? =>
                                        {
                                            (slice, false)
                                        }
                                        ExprKind::Subscript { slice, .. }
                                            if builtin_registry.has_kernel_ann(annotation).map_err(|err| vec![anyhow!("{err}")])? =>
                                        {
                                            (slice, true)
                                        }
                                        _ if !builtin_registry.supports_kernel_decorators() => (annotation, true),
                                        _ => continue, // ignore fields annotated otherwise
                                    };
                                    class_fields_def.push((*attr, dummy_field_type, mutable));
                                    annotation
                                }
                                // Supporting Class Attributes
                                Some(boxed_expr) => {
                                    // Class attributes are set as immutable regardless
                                    let (annotation, _) = match &annotation.node {
                                        ExprKind::Subscript { slice, .. } => (slice, false),
                                        _ if !builtin_registry.supports_kernel_decorators() => (annotation, false),
                                        _ => continue,
                                    };

                                    match &**boxed_expr {
                                        ast::Located { location: _, custom: (), node: ExprKind::Constant { value: v, kind: _ } } => {
                                            // Restricting the types allowed to be defined as class attributes
                                            match v {
                                                ast::Constant::Bool(_) | ast::Constant::Str(_) | ast::Constant::Int(_) | ast::Constant::Float(_) => {}
                                                _ => {
                                                    return Err(vec![
                                                    anyhow!(
                                                        "unsupported statement in class definition body (at {})",
                                                        b.location
                                                    ),
                                                ])
                                                }
                                            }
                                            class_attributes_def.push((*attr, dummy_field_type, v.clone()));
                                        }
                                        _ => {
                                            return Err(vec![
                                                anyhow!(
                                                    "unsupported statement in class definition body (at {})",
                                                    b.location
                                                ),
                                            ])
                                        }
                                    }
                                    annotation
                                }
                            };
                            let parsed_annotation = parse_ast_to_type_annotation_kinds(
                                class_resolver,
                                temp_def_list,
                                builtin_registry,
                                unifier,
                                primitives,
                                annotation.as_ref(),
                                vec![(class_id, class_type_vars_def.clone())]
                                    .into_iter()
                                    .collect::<HashMap<_, _>>(),
                            )?;

                            let is_bare_auto = matches!(&parsed_annotation, TypeAnnotation::TypeVar(t) if {
                                matches!(&*unifier.get_ty(*t), TypeEnum::TVar { range, .. } if range.is_empty())
                            });

                            if is_bare_auto {
                                // For annotations containing bare Auto
                                if let Ok(Some(resolved_ty)) = class_resolver.resolve_auto_field_type(
                                    class_name, *attr, unifier, temp_def_list, primitives,
                                ) {
                                    unifier.unify(dummy_field_type, resolved_ty)
                                        .map_err(|e| vec![anyhow!("{}", e.to_display(unifier).to_string())])?;
                                    continue;
                                }
                            } else {
                                // For annotations containing nested Auto (like list[Auto]),
                                let auto_tvars = get_type_var_contained_in_type_annotation(&parsed_annotation);
                                let has_auto_tvars = auto_tvars.iter().any(|tv| {
                                    if let TypeAnnotation::TypeVar(t) = tv {
                                        matches!(&*unifier.get_ty(*t), TypeEnum::TVar { range, .. } if range.is_empty())
                                            && !class_type_vars_def.iter().any(|declared_tv| unifier.unioned(*declared_tv, *t))
                                    } else {
                                        false
                                    }
                                });

                                if has_auto_tvars && let Ok(Some(resolved_ty)) = class_resolver.resolve_auto_field_type(
                                    class_name, *attr, unifier, temp_def_list, primitives,
                                ) {
                                    unifier.unify(dummy_field_type, resolved_ty)
                                        .map_err(|e| vec![anyhow!("{}", e.to_display(unifier).to_string())])?;
                                    continue;
                                }
                            }

                            // find type vars within this return type annotation
                            let type_vars_within =
                                get_type_var_contained_in_type_annotation(&parsed_annotation);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(t) = type_var_within else {
                                    unreachable!("must be type var annotation")
                                };

                                // Skip Auto-generated TVars, they will be resolved from runtime values later.
                                if let TypeEnum::TVar { range, .. } = &*unifier.get_ty(t) && range.is_empty() {
                                    continue;
                                }

                                if !class_type_vars_def.iter().any(|declared_tv| unifier.unioned(*declared_tv, t)) {
                                    return Err(vec![
                                        anyhow!(
                                            "class field `{attr}' uses type var `{}' which is not declared in the `Generic' annotation of class `{class_name}' (at {})\n  Note: Class declares the following type variables: [{}]",
                                            unifier.stringify(t),
                                            annotation.location,
                                            class_type_vars_def.iter().map(|tv| unifier.stringify(*tv)).join(", ")
                                        ),
                                    ])
                                }
                            }
                            type_var_to_concrete_def.insert(dummy_field_type, parsed_annotation);
                        } else {
                            return Err(vec![
                                anyhow!(
                                    "same class fields `{attr}` defined twice (at {})",
                                    target.location
                                ),
                            ])
                        }
                    } else {
                        return Err(vec![
                            anyhow!(
                                "unsupported statement type in class definition body (at {})",
                                target.location
                            ),
                        ])
                    }
                }
                ast::StmtKind::Assign { .. } // we don't class attributes
                | ast::StmtKind::Expr { value: _, .. } // typically a docstring; ignoring all expressions matches CPython behavior
                | ast::StmtKind::Pass { .. } => {}
                _ => {
                    return Err(vec![
                        anyhow!(
                            "unsupported statement type in class definition body (at {})",
                            b.location
                        ),
                    ])
                }
            }
        }
        Ok(())
    }

    fn analyze_single_class_ancestors(
        class_def: &mut TopLevelDef,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        _primitives: &PrimitiveStore,
        type_var_to_concrete_def: &HashMap<Type, TypeAnnotation>,
    ) -> Result<(), Vec<anyhow::Error>> {
        let TopLevelDef::Class {
            object_id,
            ancestors,
            fields,
            attributes,
            methods,
            resolver,
            type_vars,
            ..
        } = class_def
        else {
            unreachable!("here must be class def ast")
        };
        let (
            _class_id,
            class_ancestor_def,
            class_fields_def,
            class_attribute_def,
            class_methods_def,
            _class_type_vars_def,
            _class_resolver,
        ) = (*object_id, ancestors, fields, attributes, methods, type_vars, resolver);

        // since when this function is called, the ancestors of the direct parent
        // are supposed to be already handled, so we only need to deal with the direct parent
        let base = class_ancestor_def.get(1).unwrap();
        let TypeAnnotation::CustomClass { id, params: _ } = base else {
            unreachable!("must be class type annotation")
        };
        let TopLevelDef::Class { methods, fields, attributes, .. } =
            &*temp_def_list.get(id.0).unwrap().read()
        else {
            unreachable!("must be top level class def")
        };

        // handle methods override
        // since we need to maintain the order, create a new list
        let mut new_child_methods: IndexMap<StrRef, (Type, DefinitionId)> =
            methods.iter().map(|m| (m.0, (m.1, m.2))).collect();

        for (class_method_name, class_method_ty, class_method_defid) in &*class_methods_def {
            if let Some((ty, _)) = new_child_methods
                .insert(*class_method_name, (*class_method_ty, *class_method_defid))
            {
                let ok = class_method_name == &"__init__".into()
                    || Self::check_overload_function_type(
                        *class_method_ty,
                        ty,
                        unifier,
                        type_var_to_concrete_def,
                    );
                if !ok {
                    return Err(vec![anyhow!(
                        "method {class_method_name} has same name as ancestors' method, but incompatible type"
                    )]);
                }
            }
        }
        class_methods_def.clear();
        class_methods_def
            .extend(new_child_methods.iter().map(|f| (*f.0, f.1.0, f.1.1)).collect_vec());

        // handle class fields
        let mut new_child_fields: IndexMap<StrRef, (Type, bool)> =
            fields.iter().map(|f| (f.0, (f.1, f.2))).collect();
        let mut new_child_attributes: IndexMap<StrRef, (Type, ast::Constant)> =
            attributes.iter().map(|f| (f.0, (f.1, f.2.clone()))).collect();
        // Overriding class fields and attributes is currently not supported
        for (name, ty, mutable) in &*class_fields_def {
            if new_child_fields.insert(*name, (*ty, *mutable)).is_some()
                || new_child_attributes.contains_key(name)
            {
                return Err(vec![anyhow!(
                    "field `{name}` has already declared in the ancestor classes"
                )]);
            }
        }
        for (name, ty, val) in &*class_attribute_def {
            if new_child_attributes.insert(*name, (*ty, val.clone())).is_some()
                || new_child_fields.contains_key(name)
            {
                return Err(vec![anyhow!(
                    "attribute `{name}` has already declared in the ancestor classes"
                )]);
            }
        }

        class_fields_def.clear();
        class_fields_def
            .extend(new_child_fields.iter().map(|f| (*f.0, f.1.0, f.1.1)).collect_vec());
        class_attribute_def.clear();
        class_attribute_def.extend(
            new_child_attributes.iter().map(|f| (*f.0, f.1.0, f.1.1.clone())).collect_vec(),
        );
        Ok(())
    }

    /// step 4, analyze and call type inferencer to fill the `instance_to_stmt` of
    /// [`TopLevelDef::Function`]
    fn analyze_function_instance(&mut self) -> Result<(), Vec<anyhow::Error>> {
        // first get the class constructor type correct for the following type check in function body
        // also do class field instantiation check
        let init_str_id = "__init__".into();
        let mut definition_extension = Vec::new();
        let mut constructors = Vec::new();
        let def_list = self.extract_def_list();
        let primitives_ty = &self.primitives_ty;
        let definition_ast_list = &self.definition_ast_list;
        let unifier = &mut self.unifier;

        // first, fix function typevar ids
        // they may be changed with our use of placeholders
        for (def, _) in definition_ast_list.iter().skip(self.builtin_num) {
            if let TopLevelDef::Function { signature, var_id, .. } = &mut *def.write()
                && let TypeEnum::TFunc(FunSignature { args, ret, vars }) =
                    unifier.get_ty(*signature).as_ref()
            {
                let new_var_ids = vars
                    .values()
                    .map(|v| match &*unifier.get_ty(*v) {
                        TypeEnum::TVar { id, .. } => *id,
                        _ => unreachable!(),
                    })
                    .collect_vec();
                if new_var_ids != *var_id {
                    let new_signature = FunSignature {
                        args: args.clone(),
                        ret: *ret,
                        vars: new_var_ids
                            .iter()
                            .zip(vars.values())
                            .map(|(id, v)| (*id, *v))
                            .collect(),
                    };
                    unifier
                        .unification_table
                        .set_value(*signature, Rc::new(TypeEnum::TFunc(new_signature)));
                    *var_id = new_var_ids;
                }
            }
        }

        let mut analyze = |i, def: &Arc<RwLock<TopLevelDef>>, ast: &Option<Stmt>| {
            if let TopLevelDef::Class {
                constructor,
                ancestors,
                methods,
                fields,
                type_vars,
                name: class_name,
                object_id,
                resolver: _,
                ..
            } = &*def.read()
            {
                let self_type = get_type_from_type_annotation_kinds(
                    &def_list,
                    unifier,
                    primitives_ty,
                    &make_self_type_annotation(type_vars, *object_id),
                    &mut None,
                )?;
                if ancestors
                    .iter()
                    .any(|ann| matches!(ann, TypeAnnotation::CustomClass { id, .. } if id.0 == PrimDef::Exception.id().0))
                {
                    // create constructor for these classes
                    let PrimitiveStore { str: string, int64, .. } = *primitives_ty;
                    let signature = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: vec![
                            FuncArg {
                                name: "msg".into(),
                                ty: string,
                                default_value: Some(SymbolValue::Str(String::new())),
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "param0".into(),
                                ty: int64,
                                default_value: Some(SymbolValue::I64(0)),
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "param1".into(),
                                ty: int64,
                                default_value: Some(SymbolValue::I64(0)),
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "param2".into(),
                                ty: int64,
                                default_value: Some(SymbolValue::I64(0)),
                                is_vararg: false,
                            },
                        ],
                        ret: self_type,
                        vars: VarMap::default(),
                    }));
                    let cons_fun = TopLevelDef::Function {
                        name: format!("{}.{}", class_name, "__init__"),
                        simple_name: init_str_id,
                        signature,
                        var_id: Vec::default(),
                        attributes: Vec::default(),
                        instance_to_symbol: HashMap::default(),
                        instance_to_stmt: HashMap::default(),
                        resolver: None,
                        codegen_callback: Some(Arc::new(GenCall::new(Box::new(exn_constructor)))),
                        loc: None,
                    };
                    constructors.push((i, signature, definition_extension.len()));
                    definition_extension.push((Arc::new(RwLock::new(cons_fun)), None));
                    unifier.unify(constructor.unwrap(), signature).map_err(|e| {
                        vec![anyhow!("{}", e.at(Some(ast.as_ref().map(|ast| ast.location).unwrap())).to_display(unifier))]
                    })?;
                    return Ok(());
                }
                let mut init_id: Option<DefinitionId> = None;
                // get the class constructor type correct
                let (contor_args, contor_type_vars) = {
                    let mut constructor_args: Vec<FuncArg> = Vec::new();
                    let mut type_vars = VarMap::new();
                    for (name, func_sig, id) in methods {
                        if *name == init_str_id {
                            init_id = Some(*id);
                            let func_ty_enum = unifier.get_ty(*func_sig);
                            let TypeEnum::TFunc(FunSignature { args, vars, .. }) =
                                func_ty_enum.as_ref()
                            else {
                                unreachable!("must be typeenum::tfunc")
                            };

                            constructor_args.extend_from_slice(args);
                            type_vars.extend(vars);
                        }
                    }
                    (constructor_args, type_vars)
                };
                let contor_type = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: contor_args,
                    ret: self_type,
                    vars: contor_type_vars,
                }));
                unifier.unify(constructor.unwrap(), contor_type).map_err(|e| {
                    vec![anyhow!(
                        "{}",
                        e.at(Some(ast.as_ref().map(|ast| ast.location).unwrap()))
                            .to_display(unifier)
                    )]
                })?;

                // class field instantiation check
                if let (Some(init_id), false) = (init_id, fields.is_empty()) {
                    let init_ast =
                        definition_ast_list.get(init_id.0).and_then(|ast| ast.1.as_ref()).unwrap();
                    if let ast::StmtKind::FunctionDef { name, body, .. } = &init_ast.node {
                        if *name != init_str_id {
                            unreachable!("must be init function here")
                        }

                        let all_inited = Self::get_all_assigned_field(
                            object_id.0,
                            definition_ast_list,
                            body.as_slice(),
                        )?;
                        for (f, _, _) in fields {
                            if !all_inited.contains(f) {
                                return Err(vec![anyhow!(
                                    "fields `{f}` of class `{class_name}` not fully initialized in the initializer (at {})",
                                    body[0].location,
                                )]);
                            }
                        }
                    }
                }
            }
            Ok(())
        };

        let mut errors = Vec::new();
        for (i, (def, ast)) in definition_ast_list.iter().enumerate().skip(self.builtin_num) {
            if ast.is_none() {
                continue;
            }
            if let Err(e) = analyze(i, def, ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        for (i, signature, id) in constructors {
            let TopLevelDef::Class { methods, .. } = &mut *self.definition_ast_list[i].0.write()
            else {
                unreachable!()
            };

            methods.push((
                init_str_id,
                signature,
                DefinitionId(self.definition_ast_list.len() + id),
            ));
        }
        self.definition_ast_list.extend_from_slice(&definition_extension);

        let ctx = Arc::new(self.make_top_level_context());
        // type inference inside function body
        let def_list = self.extract_def_list();
        let primitives_ty = &self.primitives_ty;
        let definition_ast_list = &self.definition_ast_list;
        let unifier = &mut self.unifier;
        let method_class = &mut self.method_class;
        let mut analyze_2 = |id, def: &Arc<RwLock<TopLevelDef>>, ast: &Option<Stmt>| {
            if ast.is_none() {
                return Ok(());
            }

            let (name, simple_name, signature, resolver) = {
                let TopLevelDef::Function { name, simple_name, signature, resolver, .. } =
                    &*def.read()
                else {
                    return Ok(());
                };

                (name.clone(), *simple_name, *signature, resolver.clone())
            };

            let signature_ty_enum = unifier.get_ty(signature);
            let TypeEnum::TFunc(FunSignature { args, ret, vars, .. }) = signature_ty_enum.as_ref()
            else {
                unreachable!("must be typeenum::tfunc")
            };

            let mut vars = vars.clone();
            // None if is not class method
            let uninst_self_type = {
                if let Some(class_id) = method_class.get(&DefinitionId(id)) {
                    let TopLevelDef::Class { type_vars, fields, .. } =
                        &*definition_ast_list.get(class_id.0).unwrap().0.read()
                    else {
                        unreachable!("must be class def")
                    };

                    let field_types: Vec<Type> = fields.iter().map(|(_, ty, _)| *ty).collect();

                    let ty_ann = make_self_type_annotation(type_vars, *class_id);
                    let self_ty = get_type_from_type_annotation_kinds(
                        &def_list,
                        unifier,
                        primitives_ty,
                        &ty_ann,
                        &mut None,
                    )?;
                    vars.extend(type_vars.iter().map(|ty| {
                        let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*ty) else {
                            unreachable!()
                        };

                        (*id, *ty)
                    }));
                    Some((self_ty, type_vars.clone(), field_types))
                } else {
                    None
                }
            };

            // Collect TVars from class field types so that Auto TVars are treated as
            // bound by is_concrete in function_check.
            let mut field_tvars: Vec<Type> = Vec::new();
            if let Some((_, _, ref field_types)) = uninst_self_type {
                for &ty in field_types {
                    unifier.collect_tvar_handles(ty, &mut field_tvars);
                }
            }
            // carefully handle those with bounds, without bounds and no typevars
            // if class methods, `vars` also contains all class typevars here
            let (type_var_subst_comb, no_range_vars) = {
                let mut no_ranges: Vec<Type> = Vec::new();
                let var_combs = vars
                    .values()
                    .map(|ty| {
                        unifier.get_instantiations(*ty).unwrap_or_else(|| {
                            let TypeEnum::TVar { name, loc, is_const_generic: false, .. } =
                                &*unifier.get_ty(*ty)
                            else {
                                unreachable!()
                            };

                            let rigid = unifier.get_fresh_rigid_var(*name, *loc).ty;
                            no_ranges.push(rigid);
                            vec![rigid]
                        })
                    })
                    .multi_cartesian_product()
                    .collect_vec();
                let mut result: Vec<VarMap> = Vec::default();
                for comb in var_combs {
                    result.push(vars.keys().copied().zip(comb).collect());
                }
                // NOTE: if is empty, means no type var, append a empty subst, ok to do this?
                if result.is_empty() {
                    result.push(VarMap::new());
                }
                (result, no_ranges)
            };

            for subst in type_var_subst_comb {
                // for each instance
                let inst_ret = unifier.subst(*ret, &subst).unwrap_or(*ret);
                let inst_args = {
                    args.iter()
                        .map(|a| FuncArg {
                            name: a.name,
                            ty: unifier.subst(a.ty, &subst).unwrap_or(a.ty),
                            default_value: a.default_value.clone(),
                            is_vararg: false,
                        })
                        .collect_vec()
                };
                let self_type = {
                    uninst_self_type.clone().map(|(self_type, type_vars, _)| {
                        let subst_for_self = {
                            let class_ty_var_ids = type_vars
                                .iter()
                                .map(|x| {
                                    if let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*x) {
                                        *id
                                    } else {
                                        unreachable!("must be type var here");
                                    }
                                })
                                .collect::<HashSet<_>>();
                            subst
                                .iter()
                                .filter_map(|(ty_var_id, ty_var_target)| {
                                    if class_ty_var_ids.contains(ty_var_id) {
                                        Some((*ty_var_id, *ty_var_target))
                                    } else {
                                        None
                                    }
                                })
                                .collect::<VarMap>()
                        };
                        unifier.subst(self_type, &subst_for_self).unwrap_or(self_type)
                    })
                };
                let mut identifiers = {
                    let mut result = HashSet::new();
                    if self_type.is_some() {
                        result.insert("self".into());
                    }
                    result.extend(inst_args.iter().map(|x| x.name));
                    result
                };
                let mut calls: HashMap<CodeLocation, CallId> = HashMap::new();
                let mut inferencer = Inferencer {
                    top_level: ctx.as_ref(),
                    defined_identifiers: identifiers.clone(),
                    function_data: &mut FunctionData {
                        resolver: resolver.clone().unwrap(),
                        return_type: if unifier.unioned(inst_ret, primitives_ty.none) {
                            None
                        } else {
                            Some(inst_ret)
                        },
                        // NOTE: allowed type vars
                        bound_variables: {
                            let mut bv = no_range_vars.clone();
                            bv.extend(&field_tvars);
                            bv
                        },
                    },
                    unifier,
                    variable_mapping: {
                        let mut result: HashMap<StrRef, Type> = HashMap::new();
                        if let Some(self_ty) = self_type {
                            result.insert("self".into(), self_ty);
                        }
                        result.extend(inst_args.iter().map(|x| (x.name, x.ty)));
                        result
                    },
                    primitives: primitives_ty,
                    virtual_checks: &mut Vec::new(),
                    calls: &mut calls,
                    in_handler: false,
                };

                let ast::StmtKind::FunctionDef { body, decorator_list, .. } =
                    ast.clone().map(|ast| ast.node).unwrap()
                else {
                    unreachable!("must be function def ast")
                };

                // Do not further analyse extern functions as the body may contain non-compilable statements
                if decorator_list
                    .iter()
                    .try_fold(false, |acc, dec| {
                        self.builtin_registry.is_extern_decorator(dec).map(|x| x || acc)
                    })
                    .map_err(|err| vec![anyhow!("{err}")])?
                {
                    let TopLevelDef::Function { instance_to_symbol, .. } = &mut *def.write() else {
                        unreachable!()
                    };
                    instance_to_symbol.insert(String::new(), simple_name.to_string());
                    continue;
                }

                let fun_body =
                    body.into_iter()
                        .map(|b| inferencer.fold_stmt(b))
                        .collect::<Result<Vec<_>, _>>()?;

                let returned = inferencer.check_block(fun_body.as_slice(), &mut identifiers)?;
                {
                    // check virtuals
                    let defs = ctx.definitions.read();
                    for (subtype, base, loc) in &*inferencer.virtual_checks {
                        let base_id = {
                            let base = inferencer.unifier.get_ty(*base);
                            if let TypeEnum::TObj { obj_id, .. } = &*base {
                                *obj_id
                            } else {
                                return Err(vec![anyhow!(
                                    "Base type should be a class (at {loc})"
                                )]);
                            }
                        };
                        let subtype_id = {
                            let ty = inferencer.unifier.get_ty(*subtype);
                            if let TypeEnum::TObj { obj_id, .. } = &*ty {
                                *obj_id
                            } else {
                                let base_repr = inferencer.unifier.stringify(*base);
                                let subtype_repr = inferencer.unifier.stringify(*subtype);
                                return Err(vec![anyhow!(
                                    "Expected a subtype of {base_repr}, but got {subtype_repr} (at {loc})"
                                )]);
                            }
                        };
                        let TopLevelDef::Class { ancestors, .. } = &*defs[subtype_id.0].read()
                        else {
                            unreachable!()
                        };

                        let m = ancestors.iter()
                            .find(|kind| matches!(kind, TypeAnnotation::CustomClass { id, .. } if *id == base_id));
                        if m.is_none() {
                            let base_repr = inferencer.unifier.stringify(*base);
                            let subtype_repr = inferencer.unifier.stringify(*subtype);
                            return Err(vec![anyhow!(
                                "Expected a subtype of {base_repr}, but got {subtype_repr} (at {loc})"
                            )]);
                        }
                    }
                }
                if !unifier.unioned(inst_ret, primitives_ty.none) && !returned {
                    let def_ast_list = &definition_ast_list;
                    let ret_str = unifier.internal_stringify(
                        inst_ret,
                        &mut |id| {
                            let TopLevelDef::Class { name, .. } = &*def_ast_list[id].0.read()
                            else {
                                unreachable!("must be class id here")
                            };

                            name.to_string()
                        },
                        &mut |id| format!("typevar{id}"),
                        &mut None,
                    );
                    return Err(vec![anyhow!(
                        "expected return type of `{}` in function `{}` (at {})",
                        ret_str,
                        name,
                        ast.as_ref().map(|ast| ast.location).unwrap()
                    )]);
                }

                let TopLevelDef::Function { instance_to_stmt, .. } = &mut *def.write() else {
                    unreachable!()
                };
                instance_to_stmt.insert(
                    get_subst_key(
                        unifier,
                        self_type,
                        &subst,
                        Some(&vars.keys().copied().collect()),
                    ),
                    FunInstance {
                        body: Arc::new(fun_body),
                        unifier_id: 0,
                        calls: Arc::new(calls),
                        subst,
                    },
                );
            }

            Ok(())
        };

        for (id, (def, ast)) in self.definition_ast_list.iter().enumerate().skip(self.builtin_num) {
            if ast.is_none() {
                continue;
            }
            if let Err(e) = analyze_2(id, def, ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }
}
