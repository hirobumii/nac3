use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use parking_lot::{Mutex, RwLock};

use nac3core::{
    codegen::CodeGenContext,
    inkwell::{module::Linkage, values::BasicValue},
    nac3parser::ast::{self, ExprKind, Located, StrRef},
    symbol_resolver::{SymbolResolver, SymbolValue, ValueEnum},
    toplevel::{
        DefinitionId, TopLevelDef,
        composer::{BuiltinKind, BuiltinRegistry},
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{Type, Unifier},
    },
};

pub struct ResolverInternal {
    pub id_to_type: Mutex<HashMap<StrRef, Type>>,
    pub id_to_def: Mutex<HashMap<StrRef, DefinitionId>>,
    pub module_globals: Mutex<HashMap<StrRef, SymbolValue>>,
    pub str_store: Mutex<HashMap<String, i32>>,
}

impl ResolverInternal {
    pub fn add_id_def(&self, id: StrRef, def: DefinitionId) {
        self.id_to_def.lock().insert(id, def);
    }

    pub fn add_id_type(&self, id: StrRef, ty: Type) {
        self.id_to_type.lock().insert(id, ty);
    }

    pub fn add_module_global(&self, id: StrRef, val: SymbolValue) {
        self.module_globals.lock().insert(id, val);
    }
}

pub struct Resolver(pub Arc<ResolverInternal>);

impl SymbolResolver for Resolver {
    fn get_default_param_value(&self, expr: &ast::Expr) -> Option<SymbolValue> {
        match &expr.node {
            ast::ExprKind::Name { id, .. } => self.0.module_globals.lock().get(id).cloned(),
            _ => unimplemented!("other type of expr not supported at {}", expr.location),
        }
    }

    fn get_symbol_type(
        &self,
        unifier: &mut Unifier,
        _: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
        str: StrRef,
    ) -> Result<Type, String> {
        self.0
            .id_to_type
            .lock()
            .get(&str)
            .copied()
            .or_else(|| {
                self.0
                    .module_globals
                    .lock()
                    .get(&str)
                    .cloned()
                    .map(|v| v.get_type(primitives, unifier))
            })
            .ok_or(format!("cannot get type of {str}"))
    }

    fn get_symbol_value<'ctx>(
        &self,
        str: StrRef,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Option<ValueEnum<'ctx>> {
        self.0.module_globals.lock().get(&str).cloned().map(|v| {
            ctx.module
                .get_global(&str.to_string())
                .unwrap_or_else(|| {
                    let ty = v.get_type(&ctx.primitives, &mut ctx.unifier);

                    let init_val = ctx.gen_symbol_val(&v, ty);
                    let llvm_ty = init_val.get_type();

                    let global = ctx.module.add_global(llvm_ty, None, &str.to_string());
                    global.set_linkage(Linkage::LinkOnceAny);
                    global.set_initializer(&init_val);

                    global
                })
                .as_basic_value_enum()
                .into()
        })
    }

    fn get_identifier_def(&self, id: StrRef) -> Result<DefinitionId, HashSet<String>> {
        self.0
            .id_to_def
            .lock()
            .get(&id)
            .copied()
            .ok_or_else(|| HashSet::from([format!("Undefined identifier `{id}`")]))
    }

    fn get_string_id(&self, s: &str) -> i32 {
        let mut str_store = self.0.str_store.lock();
        if let Some(id) = str_store.get(s) {
            *id
        } else {
            let id = i32::try_from(str_store.len())
                .expect("Symbol resolver string store size exceeds max capacity (i32::MAX)");
            str_store.insert(s.to_string(), id);
            id
        }
    }

    fn get_exception_id(&self, _: usize) -> usize {
        unimplemented!()
    }
}

/// Standalone mode builtin registry using string-based matching.
///
/// This implementation matches builtin identifiers by comparing the string
/// representation of names in the AST, ignoring location information.
pub struct StandaloneBuiltinRegistry;

impl BuiltinRegistry for StandaloneBuiltinRegistry {
    fn match_builtin(&self, expr: &Located<ExprKind>) -> Option<BuiltinKind> {
        let get_name = |e: &ExprKind| -> Option<String> {
            match e {
                ExprKind::Name { id, .. } => Some(id.to_string()),
                ExprKind::Subscript { value, .. } => {
                    if let ExprKind::Name { id, .. } = &value.node {
                        Some(id.to_string())
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        let name = get_name(&expr.node)?;

        Some(match name.as_str() {
            // Type annotations
            "Kernel" => BuiltinKind::Kernel,
            "KernelInvariant" => BuiltinKind::KernelInvariant,
            "ConstGeneric" => BuiltinKind::ConstGeneric,
            "none" => BuiltinKind::None,
            "virtual" => BuiltinKind::Virtual,
            "Option" => BuiltinKind::Option,

            // Decorators
            "compile" => BuiltinKind::Compile,
            "extern" => BuiltinKind::ExternFn,
            "kernel" => BuiltinKind::KernelDecorator,
            "portable" => BuiltinKind::Portable,
            "rpc" => BuiltinKind::Rpc,
            "staticmethod" => BuiltinKind::StaticMethod,

            // Core primitives
            "int" => BuiltinKind::Int,
            "float" => BuiltinKind::Float,
            "bool" => BuiltinKind::Bool,
            "str" => BuiltinKind::Str,
            "list" => BuiltinKind::List,
            "tuple" => BuiltinKind::Tuple,
            "Exception" => BuiltinKind::Exception,

            // Type system
            "Generic" => BuiltinKind::Generic,
            "TypeVar" => BuiltinKind::TypeVar,
            "GenericAlias" => BuiltinKind::GenericAlias,
            "_GenericAlias" => BuiltinKind::GenericAliasUnderscore,
            "ModuleType" => BuiltinKind::ModuleType,
            "Literal" => BuiltinKind::Literal,

            // Sized types
            "int32" => BuiltinKind::Int32,
            "int64" => BuiltinKind::Int64,
            "uint32" => BuiltinKind::Uint32,
            "uint64" => BuiltinKind::Uint64,
            "float64" => BuiltinKind::Float64,
            "ndarray" => BuiltinKind::NDArray,

            // Functions
            "range" => BuiltinKind::Range,
            "round" => BuiltinKind::Round,
            "round64" => BuiltinKind::Round64,
            "floor" => BuiltinKind::Floor,
            "floor64" => BuiltinKind::Floor64,
            "ceil" => BuiltinKind::Ceil,
            "ceil64" => BuiltinKind::Ceil64,
            "len" => BuiltinKind::Len,
            "min" => BuiltinKind::Min,
            "max" => BuiltinKind::Max,
            "abs" => BuiltinKind::Abs,
            "Some" => BuiltinKind::Some,

            // NumPy array creation
            "np_ndarray" => BuiltinKind::NpNDArray,
            "np_empty" => BuiltinKind::NpEmpty,
            "np_zeros" => BuiltinKind::NpZeros,
            "np_ones" => BuiltinKind::NpOnes,
            "np_full" => BuiltinKind::NpFull,
            "np_array" => BuiltinKind::NpArray,
            "np_eye" => BuiltinKind::NpEye,
            "np_identity" => BuiltinKind::NpIdentity,

            // NumPy array properties
            "np_size" => BuiltinKind::NpSize,
            "np_shape" => BuiltinKind::NpShape,
            "np_strides" => BuiltinKind::NpStrides,

            // NumPy array manipulation
            "np_broadcast_to" => BuiltinKind::NpBroadcastTo,
            "np_transpose" => BuiltinKind::NpTranspose,
            "np_reshape" => BuiltinKind::NpReshape,

            // NumPy math functions
            "np_round" => BuiltinKind::NpRound,
            "np_floor" => BuiltinKind::NpFloor,
            "np_ceil" => BuiltinKind::NpCeil,
            "np_min" => BuiltinKind::NpMin,
            "np_minimum" => BuiltinKind::NpMinimum,
            "np_max" => BuiltinKind::NpMax,
            "np_maximum" => BuiltinKind::NpMaximum,
            "np_argmax" => BuiltinKind::NpArgmax,
            "np_isnan" => BuiltinKind::NpIsnan,
            "np_isinf" => BuiltinKind::NpIsinf,
            "np_sin" => BuiltinKind::NpSin,
            "np_cos" => BuiltinKind::NpCos,
            "np_exp" => BuiltinKind::NpExp,
            "np_exp2" => BuiltinKind::NpExp2,
            "np_log" => BuiltinKind::NpLog,
            "np_log10" => BuiltinKind::NpLog10,
            "np_log2" => BuiltinKind::NpLog2,
            "np_fabs" => BuiltinKind::NpFabs,
            "np_sqrt" => BuiltinKind::NpSqrt,
            "np_rint" => BuiltinKind::NpRint,
            "np_tan" => BuiltinKind::NpTan,
            "np_arcsin" => BuiltinKind::NpArcsin,
            "np_arccos" => BuiltinKind::NpArccos,
            "np_arctan" => BuiltinKind::NpArctan,
            "np_sinh" => BuiltinKind::NpSinh,
            "np_cosh" => BuiltinKind::NpCosh,
            "np_tanh" => BuiltinKind::NpTanh,
            "np_arcsinh" => BuiltinKind::NpArcsinh,
            "np_arccosh" => BuiltinKind::NpArccosh,
            "np_arctanh" => BuiltinKind::NpArctanh,
            "np_expm1" => BuiltinKind::NpExpm1,
            "np_cbrt" => BuiltinKind::NpCbrt,

            // SciPy special functions
            "sp_spec_erf" => BuiltinKind::SpSpecErf,
            "sp_spec_erfc" => BuiltinKind::SpSpecErfc,
            "sp_spec_gamma" => BuiltinKind::SpSpecGamma,
            "sp_spec_gammaln" => BuiltinKind::SpSpecGammaln,
            "sp_spec_j0" => BuiltinKind::SpSpecJ0,
            "sp_spec_j1" => BuiltinKind::SpSpecJ1,

            // NumPy binary operations
            "np_arctan2" => BuiltinKind::NpArctan2,
            "np_copysign" => BuiltinKind::NpCopysign,
            "np_fmax" => BuiltinKind::NpFmax,
            "np_fmin" => BuiltinKind::NpFmin,
            "np_ldexp" => BuiltinKind::NpLdexp,
            "np_hypot" => BuiltinKind::NpHypot,
            "np_nextafter" => BuiltinKind::NpNextafter,

            // NumPy reduction operations
            "np_any" => BuiltinKind::NpAny,
            "np_all" => BuiltinKind::NpAll,

            // NumPy linear algebra
            "np_dot" => BuiltinKind::NpDot,
            "np_linalg_cholesky" => BuiltinKind::NpLinalgCholesky,
            "np_linalg_qr" => BuiltinKind::NpLinalgQr,
            "np_linalg_svd" => BuiltinKind::NpLinalgSvd,
            "np_linalg_inv" => BuiltinKind::NpLinalgInv,
            "np_linalg_pinv" => BuiltinKind::NpLinalgPinv,
            "np_linalg_matrix_power" => BuiltinKind::NpLinalgMatrixPower,
            "np_linalg_det" => BuiltinKind::NpLinalgDet,

            // SciPy linear algebra
            "sp_linalg_lu" => BuiltinKind::SpLinalgLu,
            "sp_linalg_schur" => BuiltinKind::SpLinalgSchur,
            "sp_linalg_hessenberg" => BuiltinKind::SpLinalgHessenberg,

            _ => return None,
        })
    }
}
