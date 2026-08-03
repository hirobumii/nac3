#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::{
    collections::{HashMap, HashSet},
    env, fmt, fs,
    io::Write,
    iter::once,
    path::Path,
    process::Command,
    rc::Rc,
    sync::Arc,
};

use indexmap::IndexMap;
use itertools::Itertools as _;
use nac3binutils::{Linker, symbolizer};
use nac3core::{
    codegen::{
        CodeGenOptions, CodeGenTask, CodeGenerator, ModuleContext, TargetMachineOptions, WithCall,
        WorkerRegistry, concrete_type::ConcreteTypeStore, context_ref, gen_func_impl,
        irrt::load_irrt,
    },
    inkwell::{
        OptimizationLevel,
        memory_buffer::MemoryBuffer,
        module::{Linkage, Module},
        passes::PassBuilderOptions,
        support::is_multithreaded,
        targets::{FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple},
    },
    nac3parser::{
        ast::{self, Constant, ExprKind, FileName, Located, Stmt, StmtKind, StrRef},
        parser::parse_program,
    },
    symbol_resolver::SymbolResolver,
    toplevel::{
        DefinitionId, GenCall, TopLevelDef,
        builtins::get_exn_constructor,
        composer::{BuiltinFuncCreator, BuiltinFuncSpec, BuiltinRegistry, TopLevelComposer},
        helper::{PrimDef, get_decorator_flags},
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, FuncArg, Type, TypeEnum, Unifier, VarMap, into_var_map},
    },
};
use parking_lot::{Mutex, RwLock};
use pyo3::{
    IntoPyObjectExt, create_exception, exceptions,
    prelude::*,
    types::{PyAnyMethods, PyBytes, PyDict, PyList, PyNone, PyTuple, PyType},
};
use tempfile::{self, TempDir};

use codegen::{
    ArtiqCodeGenerator, attributes_writeback, gen_core_log, gen_rtio_log, rpc_codegen_callback,
};
use symbol_resolver::{DeferredEvaluationStore, InnerResolver, PythonHelper, Resolver};
use timeline::TimeFns;

mod codegen;
mod debug;
mod py_interp;
mod symbol_resolver;
mod timeline;

const ENV_NAC3_EMIT_LLVM_BC: &str = "NAC3_EMIT_LLVM_BC";
const ENV_NAC3_EMIT_LLVM_LL: &str = "NAC3_EMIT_LLVM_LL";
const ENV_NAC3_OPT_LEVEL: &str = "NAC3_OPT_LEVEL";
const ENV_NAC3_EMIT_OBJ: &str = "NAC3_EMIT_OBJ";
const ENV_NAC3_EMIT_ELF: &str = "NAC3_EMIT_ELF";
const ENV_NAC3_EMIT_DEBUG_ELF: &str = "NAC3_EMIT_DEBUG_ELF";

#[derive(PartialEq, Clone, Copy)]
enum Isa {
    Host,
    RiscV32G,
    RiscV32IMA,
    CortexA9,
}

impl Isa {
    /// Returns the [`TargetTriple`] used for compiling to this ISA.
    pub fn get_llvm_target_triple(self) -> TargetTriple {
        match self {
            Self::Host => TargetMachine::get_default_triple(),
            Self::RiscV32G | Self::RiscV32IMA => TargetTriple::create("riscv32-unknown-linux"),
            Self::CortexA9 => TargetTriple::create("armv7-unknown-linux-eabihf"),
        }
    }

    /// Returns the [`String`] representing the target CPU used for compiling to this ISA.
    pub fn get_llvm_target_cpu(self) -> String {
        match self {
            Self::Host => TargetMachine::get_host_cpu_name().to_string(),
            Self::RiscV32G | Self::RiscV32IMA => "generic-rv32".to_string(),
            Self::CortexA9 => "cortex-a9".to_string(),
        }
    }

    /// Returns the [`String`] representing the target features used for compiling to this ISA.
    pub fn get_llvm_target_features(self) -> String {
        match self {
            Self::Host => TargetMachine::get_host_cpu_features().to_string(),
            Self::RiscV32G => "+a,+m,+f,+d".to_string(),
            Self::RiscV32IMA => "+a,+m".to_string(),
            Self::CortexA9 => "+dsp,+fp16,+neon,+vfp3,+long-calls".to_string(),
        }
    }

    /// Returns an instance of [`CodeGenTargetMachineOptions`] representing the target machine
    /// options used for compiling to this ISA.
    pub fn get_llvm_target_options(
        self,
        target_opt_level: OptimizationLevel,
    ) -> TargetMachineOptions {
        TargetMachineOptions {
            triple: self.get_llvm_target_triple().as_str().to_string_lossy().into_owned(),
            cpu: self.get_llvm_target_cpu(),
            features: self.get_llvm_target_features(),
            reloc_mode: RelocMode::PIC,
            ..TargetMachineOptions::from_host(target_opt_level)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyId {
    Type(u64),
    Decorator(Vec<u64>),
}

impl From<u64> for PyId {
    fn from(id: u64) -> Self {
        Self::Type(id)
    }
}

/// Errors that can occur during builtin identifier matching
#[derive(Debug, Clone)]
enum BuiltinMatchError {
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

/// ARTIQ mode builtin registry using Python object ID matching.
///
/// This implementation matches builtin identifiers by comparing Python object IDs
pub struct ArtiqBuiltinRegistry {
    /// Mapping from Python object ID to `PrimDef`
    id_to_builtin: HashMap<u64, PrimDef>,
    /// Python modules indexed by file for context resolution
    modules: Arc<HashMap<FileName, Arc<Py<PyModule>>>>,
}

impl ArtiqBuiltinRegistry {
    #[must_use]
    pub fn new(
        primitive_ids: &PrimitivePythonId,
        modules: Arc<HashMap<FileName, Arc<Py<PyModule>>>>,
    ) -> Self {
        let mut id_to_builtin = HashMap::new();

        // Core primitives
        id_to_builtin.insert(primitive_ids.builtins.float, PrimDef::Float);
        id_to_builtin.insert(primitive_ids.numpy.float64, PrimDef::Float);
        id_to_builtin.insert(primitive_ids.builtins.bool, PrimDef::Bool);
        id_to_builtin.insert(primitive_ids.builtins.str_class, PrimDef::Str);
        id_to_builtin.insert(primitive_ids.builtins.list, PrimDef::List);
        id_to_builtin.insert(primitive_ids.builtins.tuple, PrimDef::Tuple);
        id_to_builtin.insert(primitive_ids.builtins.exception, PrimDef::Exception);

        // Core functions
        id_to_builtin.insert(primitive_ids.builtins.range, PrimDef::Range);
        id_to_builtin.insert(primitive_ids.builtins.enumerate, PrimDef::Enumerate);
        id_to_builtin.insert(primitive_ids.builtins.round, PrimDef::FunRound);
        id_to_builtin.insert(primitive_ids.builtins.round64, PrimDef::FunRound64);
        id_to_builtin.insert(primitive_ids.builtins.floor, PrimDef::FunFloor);
        id_to_builtin.insert(primitive_ids.builtins.floor64, PrimDef::FunFloor64);
        id_to_builtin.insert(primitive_ids.builtins.ceil, PrimDef::FunCeil);
        id_to_builtin.insert(primitive_ids.builtins.ceil64, PrimDef::FunCeil64);
        id_to_builtin.insert(primitive_ids.builtins.len, PrimDef::FunLen);
        id_to_builtin.insert(primitive_ids.builtins.min, PrimDef::FunMin);
        id_to_builtin.insert(primitive_ids.builtins.max, PrimDef::FunMax);
        id_to_builtin.insert(primitive_ids.builtins.abs, PrimDef::FunAbs);
        id_to_builtin.insert(primitive_ids.builtins.some, PrimDef::FunSome);
        id_to_builtin.insert(primitive_ids.builtins.staticmethod_decor_fn, PrimDef::StaticMethod);

        // Type qualifier
        id_to_builtin.insert(primitive_ids.artiq.auto, PrimDef::Auto);
        id_to_builtin.insert(primitive_ids.artiq.kernel, PrimDef::Kernel);
        id_to_builtin.insert(primitive_ids.artiq.kernel_invariant, PrimDef::KernelInvariant);
        id_to_builtin.insert(primitive_ids.artiq.const_generic_marker, PrimDef::ConstGeneric);
        id_to_builtin.insert(primitive_ids.artiq.none, PrimDef::None);
        id_to_builtin.insert(primitive_ids.artiq.virtual_class, PrimDef::Virtual);
        id_to_builtin.insert(primitive_ids.artiq.option, PrimDef::Option);

        // Context managers
        id_to_builtin.insert(primitive_ids.artiq.critical, PrimDef::Critical);

        // Decorators
        id_to_builtin.insert(primitive_ids.artiq.compile_decor_fn, PrimDef::Compile);
        id_to_builtin.insert(primitive_ids.artiq.extern_decor_fn, PrimDef::ExternFn);
        id_to_builtin.insert(primitive_ids.artiq.kernel_decor_fn, PrimDef::KernelDecorator);
        id_to_builtin.insert(primitive_ids.artiq.portable_decor_fn, PrimDef::Portable);
        id_to_builtin.insert(primitive_ids.artiq.rpc_decor_fn, PrimDef::Rpc);

        // Typing
        id_to_builtin.insert(primitive_ids.typing.generic, PrimDef::Generic);
        id_to_builtin.insert(primitive_ids.typing.literal, PrimDef::Literal);

        // NumPy
        id_to_builtin.insert(primitive_ids.numpy.int32, PrimDef::Int32);
        id_to_builtin.insert(primitive_ids.numpy.int64, PrimDef::Int64);
        id_to_builtin.insert(primitive_ids.numpy.uint32, PrimDef::UInt32);
        id_to_builtin.insert(primitive_ids.numpy.uint64, PrimDef::UInt64);
        id_to_builtin.insert(primitive_ids.numpy.ndarray, PrimDef::NDArray);

        id_to_builtin.insert(primitive_ids.numpy.empty, PrimDef::FunNpEmpty);
        id_to_builtin.insert(primitive_ids.numpy.zeros, PrimDef::FunNpZeros);
        id_to_builtin.insert(primitive_ids.numpy.ones, PrimDef::FunNpOnes);
        id_to_builtin.insert(primitive_ids.numpy.full, PrimDef::FunNpFull);
        id_to_builtin.insert(primitive_ids.numpy.array, PrimDef::FunNpArray);
        id_to_builtin.insert(primitive_ids.numpy.eye, PrimDef::FunNpEye);
        id_to_builtin.insert(primitive_ids.numpy.identity, PrimDef::FunNpIdentity);

        id_to_builtin.insert(primitive_ids.numpy.size, PrimDef::FunNpSize);
        id_to_builtin.insert(primitive_ids.numpy.shape, PrimDef::FunNpShape);

        id_to_builtin.insert(primitive_ids.numpy.broadcast_to, PrimDef::FunNpBroadcastTo);
        id_to_builtin.insert(primitive_ids.numpy.transpose, PrimDef::FunNpTranspose);
        id_to_builtin.insert(primitive_ids.numpy.reshape, PrimDef::FunNpReshape);

        id_to_builtin.insert(primitive_ids.numpy.round, PrimDef::FunNpRound);
        id_to_builtin.insert(primitive_ids.numpy.floor, PrimDef::FunNpFloor);
        id_to_builtin.insert(primitive_ids.numpy.ceil, PrimDef::FunNpCeil);
        id_to_builtin.insert(primitive_ids.numpy.min, PrimDef::FunNpMin);
        id_to_builtin.insert(primitive_ids.numpy.minimum, PrimDef::FunNpMinimum);
        id_to_builtin.insert(primitive_ids.numpy.max, PrimDef::FunNpMax);
        id_to_builtin.insert(primitive_ids.numpy.maximum, PrimDef::FunNpMaximum);
        id_to_builtin.insert(primitive_ids.numpy.argmin, PrimDef::FunNpArgmin);
        id_to_builtin.insert(primitive_ids.numpy.argmax, PrimDef::FunNpArgmax);
        id_to_builtin.insert(primitive_ids.numpy.isnan, PrimDef::FunNpIsNan);
        id_to_builtin.insert(primitive_ids.numpy.isinf, PrimDef::FunNpIsInf);
        id_to_builtin.insert(primitive_ids.numpy.sin, PrimDef::FunNpSin);
        id_to_builtin.insert(primitive_ids.numpy.cos, PrimDef::FunNpCos);
        id_to_builtin.insert(primitive_ids.numpy.exp, PrimDef::FunNpExp);
        id_to_builtin.insert(primitive_ids.numpy.exp2, PrimDef::FunNpExp2);
        id_to_builtin.insert(primitive_ids.numpy.log, PrimDef::FunNpLog);
        id_to_builtin.insert(primitive_ids.numpy.log10, PrimDef::FunNpLog10);
        id_to_builtin.insert(primitive_ids.numpy.log2, PrimDef::FunNpLog2);
        id_to_builtin.insert(primitive_ids.numpy.fabs, PrimDef::FunNpFabs);
        id_to_builtin.insert(primitive_ids.numpy.sqrt, PrimDef::FunNpSqrt);
        id_to_builtin.insert(primitive_ids.numpy.rint, PrimDef::FunNpRint);
        id_to_builtin.insert(primitive_ids.numpy.tan, PrimDef::FunNpTan);
        id_to_builtin.insert(primitive_ids.numpy.arcsin, PrimDef::FunNpArcsin);
        id_to_builtin.insert(primitive_ids.numpy.arccos, PrimDef::FunNpArccos);
        id_to_builtin.insert(primitive_ids.numpy.arctan, PrimDef::FunNpArctan);
        id_to_builtin.insert(primitive_ids.numpy.sinh, PrimDef::FunNpSinh);
        id_to_builtin.insert(primitive_ids.numpy.cosh, PrimDef::FunNpCosh);
        id_to_builtin.insert(primitive_ids.numpy.tanh, PrimDef::FunNpTanh);
        id_to_builtin.insert(primitive_ids.numpy.arcsinh, PrimDef::FunNpArcsinh);
        id_to_builtin.insert(primitive_ids.numpy.arccosh, PrimDef::FunNpArccosh);
        id_to_builtin.insert(primitive_ids.numpy.arctanh, PrimDef::FunNpArctanh);
        id_to_builtin.insert(primitive_ids.numpy.expm1, PrimDef::FunNpExpm1);
        id_to_builtin.insert(primitive_ids.numpy.cbrt, PrimDef::FunNpCbrt);

        id_to_builtin.insert(primitive_ids.numpy.spec_erf, PrimDef::FunSpSpecErf);
        id_to_builtin.insert(primitive_ids.numpy.spec_erfc, PrimDef::FunSpSpecErfc);
        id_to_builtin.insert(primitive_ids.numpy.spec_gamma, PrimDef::FunSpSpecGamma);
        id_to_builtin.insert(primitive_ids.numpy.spec_gammaln, PrimDef::FunSpSpecGammaln);
        id_to_builtin.insert(primitive_ids.numpy.spec_j0, PrimDef::FunSpSpecJ0);
        id_to_builtin.insert(primitive_ids.numpy.spec_j1, PrimDef::FunSpSpecJ1);

        id_to_builtin.insert(primitive_ids.numpy.arctan2, PrimDef::FunNpArctan2);
        id_to_builtin.insert(primitive_ids.numpy.copysign, PrimDef::FunNpCopysign);
        id_to_builtin.insert(primitive_ids.numpy.fmax, PrimDef::FunNpFmax);
        id_to_builtin.insert(primitive_ids.numpy.fmin, PrimDef::FunNpFmin);
        id_to_builtin.insert(primitive_ids.numpy.ldexp, PrimDef::FunNpLdExp);
        id_to_builtin.insert(primitive_ids.numpy.hypot, PrimDef::FunNpHypot);
        id_to_builtin.insert(primitive_ids.numpy.nextafter, PrimDef::FunNpNextAfter);

        id_to_builtin.insert(primitive_ids.numpy.any, PrimDef::FunNpAny);
        id_to_builtin.insert(primitive_ids.numpy.all, PrimDef::FunNpAll);

        id_to_builtin.insert(primitive_ids.numpy.dot, PrimDef::FunNpDot);
        id_to_builtin.insert(primitive_ids.numpy.linalg_cholesky, PrimDef::FunNpLinalgCholesky);
        id_to_builtin.insert(primitive_ids.numpy.linalg_qr, PrimDef::FunNpLinalgQr);
        id_to_builtin.insert(primitive_ids.numpy.linalg_svd, PrimDef::FunNpLinalgSvd);
        id_to_builtin.insert(primitive_ids.numpy.linalg_inv, PrimDef::FunNpLinalgInv);
        id_to_builtin.insert(primitive_ids.numpy.linalg_pinv, PrimDef::FunNpLinalgPinv);
        id_to_builtin
            .insert(primitive_ids.numpy.linalg_matrix_power, PrimDef::FunNpLinalgMatrixPower);
        id_to_builtin.insert(primitive_ids.numpy.linalg_det, PrimDef::FunNpLinalgDet);

        id_to_builtin.insert(primitive_ids.numpy.linalg_lu, PrimDef::FunSpLinalgLu);
        id_to_builtin.insert(primitive_ids.numpy.linalg_schur, PrimDef::FunSpLinalgSchur);
        id_to_builtin.insert(primitive_ids.numpy.linalg_hessenberg, PrimDef::FunSpLinalgHessenberg);
        Self { id_to_builtin, modules }
    }

    fn get_python_id(&self, expr: &Located<ExprKind>) -> Result<u64, BuiltinMatchError> {
        let module = self
            .modules
            .get(&expr.location.file)
            .ok_or(BuiltinMatchError::ModuleNotFound { file: expr.location.file })?;

        Python::attach(|py| {
            let module = module.bind(py);

            // Try to resolve as a class
            if let Some(class_obj) = get_class_type(expr, module)
                .map_err(|e| BuiltinMatchError::PythonError(e.to_string()))?
            {
                return py_interp::extract_id(class_obj.as_any())
                    .map_err(|e| BuiltinMatchError::PythonError(e.to_string()));
            }

            // If not a class, try to resolve as a func
            if let Some(obj) = get_decorator_fn(expr, module)
                .map_err(|e| BuiltinMatchError::PythonError(e.to_string()))?
            {
                return py_interp::extract_id(&obj)
                    .map_err(|e| BuiltinMatchError::PythonError(e.to_string()));
            }

            // If it's neither a class nor a func, it's not a builtin we recognize
            Err(BuiltinMatchError::ResolutionError(format!(
                "Cannot resolve {:?} - not a class or func",
                expr.node
            )))
        })
    }
}

impl BuiltinRegistry for ArtiqBuiltinRegistry {
    fn match_builtin(&self, expr: &Located<ExprKind>) -> Option<PrimDef> {
        let py_id = self.get_python_id(expr).ok()?;

        if let Some(kind) = self.id_to_builtin.get(&py_id) {
            return Some(*kind);
        }

        None
    }

    fn supports_kernel_decorators(&self) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct BuiltinPythonId {
    int: u64,
    float: u64,
    bool: u64,
    str_class: u64,
    list: u64,
    tuple: u64,
    exception: u64,
    range: u64,
    enumerate: u64,
    round: u64,
    round64: u64,
    floor: u64,
    floor64: u64,
    ceil: u64,
    ceil64: u64,
    min: u64,
    max: u64,
    abs: u64,
    len: u64,
    some: u64,
    staticmethod_decor_fn: u64,
}

#[derive(Clone)]
pub struct TypesPythonId {
    generic_alias: u64,
    module_type: u64,
}

#[derive(Clone)]
pub struct TypingPythonId {
    generic: u64,
    generic_alias: u64,
    typevar: u64,
    literal: u64,
}

#[derive(Clone)]
pub struct NumpyPythonId {
    int32: u64,
    int64: u64,
    uint32: u64,
    uint64: u64,
    float64: u64,
    bool_: u64,
    str_: u64,
    ndarray: u64,

    empty: u64,
    zeros: u64,
    ones: u64,
    full: u64,
    array: u64,
    eye: u64,
    identity: u64,

    size: u64,
    shape: u64,

    broadcast_to: u64,
    transpose: u64,
    reshape: u64,

    round: u64,
    floor: u64,
    ceil: u64,
    min: u64,
    minimum: u64,
    max: u64,
    maximum: u64,
    argmin: u64,
    argmax: u64,
    isnan: u64,
    isinf: u64,
    sin: u64,
    cos: u64,
    exp: u64,
    exp2: u64,
    log: u64,
    log10: u64,
    log2: u64,
    fabs: u64,
    sqrt: u64,
    rint: u64,
    tan: u64,
    arcsin: u64,
    arccos: u64,
    arctan: u64,
    sinh: u64,
    cosh: u64,
    tanh: u64,
    arcsinh: u64,
    arccosh: u64,
    arctanh: u64,
    expm1: u64,
    cbrt: u64,

    spec_erf: u64,
    spec_erfc: u64,
    spec_gamma: u64,
    spec_gammaln: u64,
    spec_j0: u64,
    spec_j1: u64,

    arctan2: u64,
    copysign: u64,
    fmax: u64,
    fmin: u64,
    ldexp: u64,
    hypot: u64,
    nextafter: u64,

    any: u64,
    all: u64,

    dot: u64,
    linalg_cholesky: u64,
    linalg_qr: u64,
    linalg_svd: u64,
    linalg_inv: u64,
    linalg_pinv: u64,
    linalg_matrix_power: u64,
    linalg_det: u64,

    linalg_lu: u64,
    linalg_schur: u64,
    linalg_hessenberg: u64,
}

#[derive(Clone)]
pub struct ArtiqPythonId {
    auto: u64,
    kernel: u64,
    kernel_invariant: u64,
    const_generic_marker: u64,
    none: u64,
    virtual_class: u64,
    option: u64,

    // Context Managers
    critical: u64,

    // Decorator Functions
    compile_decor_fn: u64,
    extern_decor_fn: u64,
    kernel_decor_fn: u64,
    portable_decor_fn: u64,
    rpc_decor_fn: u64,
}

#[derive(Clone)]
pub struct PrimitivePythonId {
    builtins: BuiltinPythonId,
    types: TypesPythonId,
    typing: TypingPythonId,
    numpy: NumpyPythonId,
    artiq: ArtiqPythonId,
}

#[derive(Clone, Default)]
pub struct SpecialPythonId {
    parallel: u64,
    legacy_parallel: u64,
    sequential: u64,
}

type TopLevelComponent = (Stmt, String, Arc<Py<PyModule>>);

/// A Python module and some information needed for compilation.
struct ModuleInfo {
    /// Handle to the [`PyModule`].
    module: Arc<Py<PyModule>>,
    /// The file from which the module was loaded from, corresponding to `module.__file__`, or
    /// `<expcontent>` if it is submitted to NAC3 by content.
    file: FileName,
}

// TopLevelComposer is unsendable as it holds the unification table, which is
// unsendable due to Rc. Arc would cause a performance hit.
#[pyclass(unsendable, name = "NAC3")]
struct Nac3 {
    isa: Isa,
    time_fns: &'static (dyn TimeFns + Sync),
    primitive: PrimitiveStore,
    builtins: Vec<BuiltinFuncSpec>,
    pyid_to_def: Arc<RwLock<HashMap<u64, DefinitionId>>>,
    primitive_ids: PrimitivePythonId,
    working_directory: TempDir,
    top_levels: Vec<TopLevelComponent>,
    string_store: Arc<RwLock<HashMap<String, i32>>>,
    exception_ids: Arc<RwLock<HashMap<usize, usize>>>,
    special_ids: SpecialPythonId,
    /// Modules registered with NAC3.
    modules: Arc<RwLock<Vec<ModuleInfo>>>,
    /// LLVM-related options for code generation.
    codegen_options: CodeGenOptions,
}

create_exception!(nac3artiq, CompileError, exceptions::PyException);

impl Nac3 {
    fn register_module(
        &mut self,
        module: &Arc<Py<PyModule>>,
        registered_class_ids: &HashSet<u64>,
    ) -> PyResult<()> {
        let (module_name, source_file, source) = Python::attach(|py| -> PyResult<_> {
            let module = module.bind(py);
            let source_file = module.getattr("__file__");
            let (source_file, source) = if let Ok(source_file) = source_file {
                let source_file = source_file.extract::<&str>()?;
                (
                    source_file.to_string(),
                    fs::read_to_string(source_file).map_err(|e| {
                        exceptions::PyIOError::new_err(format!("failed to read input file: {e}"))
                    })?,
                )
            } else {
                // kernels submitted by content have no file
                // but still can provide source by StringLoader
                let get_src_fn = module.getattr("__loader__")?.getattr("get_source")?;
                (String::from("<expcontent>"), get_src_fn.call1((PyNone::get(py),))?.extract()?)
            };
            Ok((
                module.getattr("__name__")?.extract::<String>()?,
                FileName::from(source_file),
                source,
            ))
        })?;

        let parser_result = parse_program(&source, source_file)
            .map_err(|e| exceptions::PySyntaxError::new_err(format!("parse error: {e}")))?;

        for mut stmt in parser_result {
            let include = match stmt.node {
                StmtKind::Import { .. } => true,
                StmtKind::ClassDef { ref decorator_list, ref mut body, ref mut bases, .. } => {
                    // Check if the class is a NAC3 class by looking for `compile` decorator
                    let nac3_class = Python::attach(|py| -> PyResult<_> {
                        let module = module.bind(py);

                        decorator_list.iter().try_fold(false, |acc, decorator| {
                            Ok(acc
                                || is_decor_fn_same(
                                    decorator,
                                    module,
                                    &[self.primitive_ids.artiq.compile_decor_fn],
                                )?)
                        })
                    })?;

                    if !nac3_class {
                        continue;
                    }

                    // Drop unregistered (i.e. host-only) base classes.
                    *bases = {
                        let len = bases.len();
                        bases.drain(..).try_fold(Vec::with_capacity(len), |mut bases, base| {
                            let retain = Python::attach(|py| -> PyResult<_> {
                                let Some((path, id)) = class_expr_id_path(&base) else {
                                    return Ok(true);
                                };

                                let module = module.bind(py);
                                let Some(base_obj) = resolve_qname((path, id), module)? else {
                                    return Ok(false);
                                };
                                let base_id = py_interp::extract_id(&base_obj)?;

                                Ok(base_id == self.primitive_ids.builtins.exception
                                    || base_id == self.primitive_ids.typing.generic
                                    || registered_class_ids.contains(&base_id))
                            })?;

                            if retain {
                                bases.push(base);
                            }

                            Ok::<_, PyErr>(bases)
                        })?
                    };

                    *body = {
                        let len = body.len();
                        body.drain(..).try_fold(Vec::with_capacity(len), |mut body, stmt| {
                            let retain =
                                if let StmtKind::FunctionDef { ref decorator_list, .. } = stmt.node
                                {
                                    Python::attach(|py| -> PyResult<_> {
                                        let module = module.bind(py);

                                        // Keep all class functions decorated with `kernel`, `portable`, or `rpc` decorator
                                        decorator_list.iter().try_fold(false, |acc, decorator| {
                                            Ok(acc
                                                || is_decor_fn_same(
                                                    decorator,
                                                    module,
                                                    &[
                                                        self.primitive_ids.artiq.kernel_decor_fn,
                                                        self.primitive_ids.artiq.portable_decor_fn,
                                                        self.primitive_ids.artiq.rpc_decor_fn,
                                                    ],
                                                )?)
                                        })
                                    })?
                                } else {
                                    true
                                };

                            if retain {
                                body.push(stmt);
                            }

                            Ok::<_, PyErr>(body)
                        })?
                    };

                    true
                }
                StmtKind::FunctionDef { ref decorator_list, .. } => {
                    Python::attach(|py| -> PyResult<_> {
                        let module = module.bind(py);

                        // Keep all top-level functions decorated with `extern`, `kernel`, `portable`, or `rpc` decorator
                        decorator_list.iter().try_fold(false, |acc, decorator| {
                            Ok(acc
                                || is_decor_fn_same(
                                    decorator,
                                    module,
                                    &[
                                        self.primitive_ids.artiq.extern_decor_fn,
                                        self.primitive_ids.artiq.kernel_decor_fn,
                                        self.primitive_ids.artiq.portable_decor_fn,
                                        self.primitive_ids.artiq.rpc_decor_fn,
                                    ],
                                )?)
                        })
                    })?
                }

                _ => false,
            };

            if include {
                self.top_levels.push((stmt, module_name.clone(), module.clone()));
            }
        }

        self.modules.write().push(ModuleInfo { module: module.clone(), file: source_file });

        Ok(())
    }

    fn report_modinit(
        arg_names: &[String],
        method_name: &str,
        resolver: &Arc<dyn SymbolResolver + Send + Sync>,
        top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
    ) -> Option<String> {
        let base_ty =
            match resolver.get_symbol_type(unifier, top_level_defs, primitives, "base".into()) {
                Ok(ty) => ty,
                Err(e) => return Some(format!("type error inside object launching kernel: {e}")),
            };

        let fun_ty = if method_name.is_empty() {
            base_ty
        } else if let TypeEnum::TObj { fields, .. } = &*unifier.get_ty(base_ty) {
            match fields.get(&(*method_name).into()) {
                Some(t) => t.0,
                None => {
                    return Some(format!(
                        "object launching kernel does not have method `{method_name}`"
                    ));
                }
            }
        } else {
            return Some("cannot launch kernel by calling a non-callable".into());
        };

        if let TypeEnum::TFunc(FunSignature { args, .. }) = &*unifier.get_ty(fun_ty) {
            if arg_names.len() > args.len() {
                return Some(format!(
                    "launching kernel function with too many arguments (expect {}, found {})",
                    args.len(),
                    arg_names.len(),
                ));
            }
            for (i, FuncArg { ty, default_value, name, .. }) in args.iter().enumerate() {
                let in_name = match arg_names.get(i) {
                    Some(n) => n,
                    None if default_value.is_none() => {
                        return Some(format!(
                            "argument `{name}` not provided when launching kernel function"
                        ));
                    }
                    _ => break,
                };
                let in_ty = match resolver.get_symbol_type(
                    unifier,
                    top_level_defs,
                    primitives,
                    in_name.clone().into(),
                ) {
                    Ok(t) => t,
                    Err(e) => {
                        return Some(format!(
                            "type error ({e}) at parameter #{i} when calling kernel function"
                        ));
                    }
                };
                if let Err(e) = unifier.unify(in_ty, *ty) {
                    return Some(format!(
                        "type error ({}) at parameter #{i} when calling kernel function",
                        e.to_display(unifier),
                    ));
                }
            }
        } else {
            return Some("cannot launch kernel by calling a non-callable".into());
        }
        None
    }

    /// Returns a [`Vec`] of builtins that needs to be initialized during method compilation time.
    fn get_lateinit_builtins() -> Vec<Box<BuiltinFuncCreator>> {
        vec![
            Box::new(|primitives, unifier| {
                let arg_ty = unifier.get_fresh_var(Some("T".into()), None);

                (
                    "core_log".into(),
                    FunSignature {
                        args: vec![FuncArg {
                            name: "arg".into(),
                            ty: arg_ty.ty,
                            default_value: None,
                            is_vararg: false,
                        }],
                        ret: primitives.none,
                        vars: into_var_map([arg_ty]),
                    },
                    Arc::new(GenCall::new(Box::new(move |ctx, obj, fun, args| {
                        gen_core_log(ctx, obj.as_ref(), fun, &args)?;

                        Ok(None)
                    }))),
                )
            }),
            Box::new(|primitives, unifier| {
                let arg_ty = unifier.get_fresh_var(Some("T".into()), None);

                (
                    "rtio_log".into(),
                    FunSignature {
                        args: vec![
                            FuncArg {
                                name: "channel".into(),
                                ty: primitives.str,
                                default_value: None,
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "arg".into(),
                                ty: arg_ty.ty,
                                default_value: None,
                                is_vararg: false,
                            },
                        ],
                        ret: primitives.none,
                        vars: into_var_map([arg_ty]),
                    },
                    Arc::new(GenCall::new(Box::new(move |ctx, obj, fun, args| {
                        gen_rtio_log(ctx, obj.as_ref(), fun, &args)?;

                        Ok(None)
                    }))),
                )
            }),
        ]
    }

    fn compile_method<'py, T>(
        &self,
        obj: &Bound<'py, PyAny>,
        method_name: &str,
        args: Vec<Bound<'py, PyAny>>,
        embedding_map: &Bound<'py, PyAny>,
        py: Python<'py>,
        link_fn: &dyn Fn(&Module) -> PyResult<T>,
    ) -> PyResult<T> {
        let size_t = self.primitive.size_t;

        // Cache all imported modules indexed by their path for symbol resolution context
        let modules_by_path = Arc::new(
            self.modules
                .read()
                .iter()
                .map(|mod_info| (mod_info.file, mod_info.module.clone()))
                .collect(),
        );

        let builtin_registry: Arc<dyn BuiltinRegistry> =
            Arc::new(ArtiqBuiltinRegistry::new(&self.primitive_ids, modules_by_path));

        let (mut composer, mut builtins_def, mut builtins_ty) = TopLevelComposer::new(
            self.builtins.clone(),
            Self::get_lateinit_builtins(),
            builtin_registry,
            size_t,
        );

        let store_obj = embedding_map.getattr("store_object")?;
        let store_str = embedding_map.getattr("store_str")?;
        let store_fun = embedding_map.getattr("store_function")?.into_py_any(py)?;
        let host_attributes = embedding_map.getattr("attributes_writeback")?.into_py_any(py)?;
        let global_value_ids: Arc<RwLock<IndexMap<_, _>>> = Arc::new(RwLock::new(IndexMap::new()));
        let helper = PythonHelper {
            store_obj: Arc::new(store_obj.clone().into_py_any(py)?),
            store_str: Arc::new(store_str.into_py_any(py)?),
        };

        let pyid_to_type = Arc::new(RwLock::new(HashMap::<u64, Type>::new()));
        let exception_names = [
            "ZeroDivisionError",
            "IndexError",
            "ValueError",
            "RuntimeError",
            "AssertionError",
            "KeyError",
            "NotImplementedError",
            "OverflowError",
            "IOError",
            "UnwrapNoneError",
        ];
        add_exceptions(&mut composer, &mut builtins_def, &mut builtins_ty, &exception_names);

        let deferred_eval_store = DeferredEvaluationStore::new();

        // Stores a mapping from module id to attributes
        let mut module_cache: HashMap<u64, _> = HashMap::new();
        let mut register_module_to_cache = |module: &Arc<Py<PyModule>>| -> PyResult<_> {
            let py_module = module.bind(py);

            let module_id = py_interp::extract_id(py_module)?;

            module_cache.get(&module_id).cloned().map(Ok).unwrap_or({
                let module_name: String = py_module.getattr("__name__")?.extract()?;
                let module_file: Option<String> =
                    py_module.getattr_opt("__file__")?.map(|file| file.extract()).transpose()?;

                let mut name_to_pyid: HashMap<StrRef, u64> = HashMap::new();
                let members = py_module.dict();
                for (key, val) in members {
                    let key: &str = key.extract()?;
                    let val = py_interp::extract_id(&val)?;
                    name_to_pyid.insert(key.into(), val);
                }
                let resolver = Arc::new(Resolver(Arc::new(InnerResolver {
                    id_to_type: builtins_ty.clone().into(),
                    id_to_def: builtins_def.clone().into(),
                    pyid_to_def: self.pyid_to_def.clone(),
                    pyid_to_type: pyid_to_type.clone(),
                    primitive_ids: self.primitive_ids.clone(),
                    global_value_ids: global_value_ids.clone(),
                    name_to_pyid: name_to_pyid.clone(),
                    module: module.clone(),
                    id_to_pyval: RwLock::default(),
                    id_to_primitive: RwLock::default(),
                    field_to_val: RwLock::default(),
                    helper: helper.clone(),
                    string_store: self.string_store.clone(),
                    exception_ids: self.exception_ids.clone(),
                    deferred_eval_store: deferred_eval_store.clone(),
                }))) as Arc<dyn SymbolResolver + Send + Sync>;
                let name_to_pyid = Rc::new(name_to_pyid);
                let module_location =
                    module_file.map(|file| ast::Location::new(1, 1, FileName::from(file)));
                module_cache.insert(
                    module_id,
                    (name_to_pyid.clone(), resolver.clone(), module_name.clone(), module_location),
                );
                Ok((name_to_pyid, resolver, module_name, module_location))
            })
        };

        let mut rpc_ids = vec![];
        for (stmt, path, module) in &self.top_levels {
            let py_module = module.bind(py).cast::<PyModule>()?;
            let class_obj;
            if let StmtKind::ClassDef { name, .. } = &stmt.node {
                let class = py_module.getattr(name.to_string().as_str())?;
                if py_interp::extract_issubclass(&class, py_interp::get_exception_class(py)?)?
                    && class.getattr("artiq_builtin").is_err()
                {
                    class_obj = Some(class);
                } else {
                    class_obj = None;
                }
            } else {
                class_obj = None;
            }
            let (name_to_pyid, resolver, _, _) = register_module_to_cache(module)?;

            // Register all imported modules to allow resolution of classes/functions within those modules
            if let StmtKind::Import { names, .. } = &stmt.node {
                for name in names {
                    let id = name.asname.unwrap_or(name.name);
                    let imp_mod = py_module
                        .dict()
                        .get_item(id.to_string().as_str())?
                        .unwrap_or_else(|| panic!("Unable to find key `{id}` in module `{module}`"))
                        .cast_into::<PyModule>()?
                        .unbind();
                    register_module_to_cache(&Arc::new(imp_mod))?;
                }
                continue;
            }

            let (name, def_id, ty) = composer
                .register_top_level(stmt.clone(), Some(resolver.clone()), path, false)
                .map_err(|e| {
                    CompileError::new_err(format!("compilation failed\n----------\n{e}"))
                })?;
            if let Some(class_obj) = class_obj {
                self.exception_ids
                    .write()
                    .insert(def_id.0, store_obj.call1((class_obj,))?.extract()?);
            }

            match &stmt.node {
                StmtKind::FunctionDef { decorator_list, .. } => {
                    for decorator in decorator_list {
                        let decor_fn = get_decorator_fn(decorator, py_module)?;
                        let decor_fn_id = py_interp::extract_id(&decor_fn.into_pyobject(py)?)?;

                        if decor_fn_id == self.primitive_ids.artiq.rpc_decor_fn {
                            store_fun.call1(
                                py,
                                (def_id.0.into_py_any(py)?, py_module.getattr(name.to_string())?),
                            )?;
                            let is_async = decorator_list
                                .iter()
                                .flat_map(get_decorator_flags)
                                .any(|constant| constant == Constant::Str("async".into()));
                            rpc_ids.push((None, def_id, is_async));
                        } else if ![
                            self.primitive_ids.artiq.kernel_decor_fn,
                            self.primitive_ids.artiq.portable_decor_fn,
                            self.primitive_ids.artiq.extern_decor_fn,
                            self.primitive_ids.builtins.staticmethod_decor_fn,
                        ]
                        .contains(&decor_fn_id)
                        {
                            return Err(CompileError::new_err(format!(
                                "compilation failed\n----------\nDecorator {} is not supported (at {})",
                                decor_expr_id_path(decorator)
                                    .map(|(path, id)| path.iter().chain(once(&id)).join("."))
                                    .unwrap(),
                                stmt.location
                            )));
                        }
                    }
                }
                StmtKind::ClassDef { name, body, .. } => {
                    let class_name = name.to_string();
                    let class_obj = py_module.getattr(class_name.as_str())?;
                    for stmt in body {
                        if let StmtKind::FunctionDef { name, decorator_list, .. } = &stmt.node {
                            for decorator in decorator_list {
                                let decor_fn = get_decorator_fn(decorator, py_module)?;
                                let decor_fn_id =
                                    py_interp::extract_id(&decor_fn.into_pyobject(py)?)?;

                                if decor_fn_id == self.primitive_ids.artiq.rpc_decor_fn {
                                    if name == &"__init__".into() {
                                        return Err(CompileError::new_err(format!(
                                            "compilation failed\n----------\nThe constructor of class {} should not be decorated with rpc decorator (at {})",
                                            class_name, stmt.location
                                        )));
                                    }

                                    let is_async = decorator_list
                                        .iter()
                                        .flat_map(get_decorator_flags)
                                        .any(|constant| constant == Constant::Str("async".into()));
                                    rpc_ids.push((
                                        Some((class_obj.clone(), *name)),
                                        def_id,
                                        is_async,
                                    ));
                                } else if ![
                                    self.primitive_ids.artiq.kernel_decor_fn,
                                    self.primitive_ids.artiq.portable_decor_fn,
                                    self.primitive_ids.builtins.staticmethod_decor_fn,
                                ]
                                .contains(&decor_fn_id)
                                {
                                    return Err(CompileError::new_err(format!(
                                        "compilation failed\n----------\nDecorator {} is not supported (at {})",
                                        decor_expr_id_path(decorator)
                                            .map(|(path, id)| path
                                                .iter()
                                                .chain(once(&id))
                                                .join("."))
                                            .unwrap(),
                                        stmt.location
                                    )));
                                }
                            }
                        }
                    }
                }
                _ => (),
            }

            let id = name_to_pyid[&name];
            self.pyid_to_def.write().insert(id, def_id);
            {
                let mut pyid_to_ty = pyid_to_type.write();
                if let Some(ty) = ty {
                    pyid_to_ty.insert(id, ty);
                }
            }
        }

        // Adding top level module definitions
        for (module_id, (module_name_to_pyid, module_resolver, module_name, module_location)) in
            module_cache
        {
            let def_id = composer
                .register_top_level_module(
                    &module_name,
                    &module_name_to_pyid,
                    module_resolver,
                    module_location,
                )
                .map_err(|e| {
                    CompileError::new_err(format!("compilation failed\n----------\n{e}"))
                })?;

            self.pyid_to_def.write().insert(module_id, def_id);
        }

        let mut name_to_pyid: HashMap<StrRef, u64> = HashMap::new();
        let module = PyModule::new(py, "tmp")?;
        module.add("base", obj)?;
        name_to_pyid.insert("base".into(), py_interp::extract_id(obj)?);
        let mut arg_names = vec![];
        for (i, arg) in args.into_iter().enumerate() {
            let name = format!("tmp{i}");
            module.add(&*name, &arg)?;
            name_to_pyid.insert(name.clone().into(), py_interp::extract_id(&arg)?);
            arg_names.push(name);
        }
        let synthesized = if method_name.is_empty() {
            format!("def __modinit__():\n    base({})", arg_names.join(", "))
        } else {
            format!("def __modinit__():\n    base.{method_name}({})", arg_names.join(", "))
        };
        let mut synthesized =
            parse_program(&synthesized, "<nac3_synthesized_modinit>".to_string().into()).unwrap();
        let inner_resolver = Arc::new(InnerResolver {
            id_to_type: builtins_ty.clone().into(),
            id_to_def: builtins_def.clone().into(),
            pyid_to_def: self.pyid_to_def.clone(),
            pyid_to_type: pyid_to_type.clone(),
            primitive_ids: self.primitive_ids.clone(),
            global_value_ids: global_value_ids.clone(),
            id_to_pyval: RwLock::default(),
            id_to_primitive: RwLock::default(),
            field_to_val: RwLock::default(),
            name_to_pyid,
            module: Arc::new(module.unbind()),
            helper: helper.clone(),
            string_store: self.string_store.clone(),
            exception_ids: self.exception_ids.clone(),
            deferred_eval_store: deferred_eval_store.clone(),
        });
        let resolver =
            Arc::new(Resolver(inner_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;
        let (_, def_id, _) = composer
            .register_top_level(synthesized.pop().unwrap(), Some(resolver.clone()), "", false)
            .unwrap();

        // Process IRRT
        context_ref!(context);
        let irrt = load_irrt(context, &self.codegen_options.target, resolver.as_ref());

        let fun_signature =
            FunSignature { args: vec![], ret: self.primitive.none, vars: VarMap::new() };
        let mut store = ConcreteTypeStore::new();
        let mut cache = HashMap::new();
        let signature = store.from_signature(
            &mut composer.unifier,
            &self.primitive,
            &fun_signature,
            &mut cache,
        );
        let signature = store.add_cty(signature);

        if let Err(e) = composer.start_analysis(true) {
            // report error of __modinit__ separately
            return if e.iter().any(|err| err.to_string().contains("<nac3_synthesized_modinit>")) {
                let msg = Self::report_modinit(
                    &arg_names,
                    method_name,
                    &resolver,
                    &composer.extract_def_list(),
                    &mut composer.unifier,
                    &self.primitive,
                );
                Err(CompileError::new_err(format!(
                    "compilation failed\n----------\n{}",
                    msg.unwrap_or_else(|| e
                        .iter()
                        .sorted_by(|lhs, rhs| Ord::cmp(&lhs.to_string(), &rhs.to_string()))
                        .join("\n----------\n"))
                )))
            } else {
                Err(CompileError::new_err(format!(
                    "compilation failed\n----------\n{}",
                    e.iter()
                        .sorted_by(|lhs, rhs| Ord::cmp(&lhs.to_string(), &rhs.to_string()))
                        .join("\n----------\n"),
                )))
            };
        }
        let top_level = Arc::new(composer.make_top_level_context());

        {
            let defs = &top_level.definitions;
            for (class_data, id, is_async) in &rpc_ids {
                match &mut *defs[id.0].write() {
                    TopLevelDef::Function { codegen_callback, .. } => {
                        *codegen_callback = Some(rpc_codegen_callback(*is_async));
                    }
                    TopLevelDef::Class { methods, .. } => {
                        let (class_def, method_name) = class_data.as_ref().unwrap();
                        for (name, _, id) in &*methods {
                            if name != method_name {
                                continue;
                            }
                            if let TopLevelDef::Function { codegen_callback, .. } =
                                &mut *defs[id.0].write()
                            {
                                *codegen_callback = Some(rpc_codegen_callback(*is_async));
                                store_fun.call1(
                                    py,
                                    (
                                        id.0.into_py_any(py)?,
                                        class_def.getattr(name.to_string().as_str()).unwrap(),
                                    ),
                                )?;
                            }
                        }
                    }
                    TopLevelDef::Module { .. } => {
                        unreachable!("Type module cannot be decorated with @rpc")
                    }
                }
            }
        }

        let (instance, location) = {
            let defs = &top_level.definitions;
            let TopLevelDef::Function { instance_to_stmt, instance_to_symbol, loc, .. } =
                &mut *defs[def_id.0].write()
            else {
                unreachable!()
            };

            instance_to_symbol.insert(String::new(), "__modinit__".into());
            (instance_to_stmt[""].clone(), *loc)
        };

        let task = CodeGenTask {
            subst: Vec::default(),
            symbol_name: "__modinit__".to_string(),
            export_symbol: true,
            body: Arc::new(Vec::default()),
            location: location.unwrap_or_default(),
            signature,
            resolver,
            store,
            unifier_index: instance.unifier_id,
            calls: instance.calls,
            id: 0,
        };

        let membuffers: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();

        let membuffer = membuffers.clone();

        let f = Arc::new(WithCall::new(Box::new(move |module| {
            let buffer = module.write_bitcode_to_memory();
            let buffer = buffer.as_slice().into();
            membuffer.lock().push(buffer);
        })));
        let num_threads = if is_multithreaded() { 4 } else { 1 };
        let thread_names: Vec<String> = (0..num_threads).map(|_| "main".to_string()).collect();
        let threads: Vec<_> = thread_names
            .iter()
            .map(|s| {
                Box::new(ArtiqCodeGenerator::new(
                    s.clone(),
                    self.time_fns,
                    self.special_ids.clone(),
                ))
            })
            .collect();

        let membuffer = membuffers.clone();
        let mut has_return = false;
        let result = py.detach(|| {
            let mut generator = ArtiqCodeGenerator::new(
                "main".to_string(),
                self.time_fns,
                self.special_ids.clone(),
            );
            let (registry, handles) = WorkerRegistry::create_workers(
                threads,
                top_level.clone(),
                &self.codegen_options,
                &f,
            );

            context_ref!(context);
            let mut context = ModuleContext::new(context, "main", &self.codegen_options.target);

            gen_func_impl(
                &mut context,
                &mut generator,
                &registry,
                task,
                // no cached unifier to use
                &mut None::<Unifier>,
                |generator, ctx| {
                    assert_eq!(instance.body.len(), 1, "toplevel module should have 1 statement");
                    let StmtKind::Expr { value: ref expr, .. } = instance.body[0].node else {
                        unreachable!("toplevel statement must be an expression")
                    };
                    let ExprKind::Call { .. } = expr.node else {
                        unreachable!("toplevel expression must be a function call")
                    };

                    let return_obj = generator
                        .gen_expr(ctx, expr)?
                        .val
                        .map(|value| (expr.custom.unwrap(), value));
                    has_return = return_obj.is_some();
                    registry.wait_tasks_complete(handles);
                    attributes_writeback(ctx, inner_resolver.as_ref(), &host_attributes, return_obj)
                },
            )?;
            let buffer = context.module.write_bitcode_to_memory();
            let buffer = buffer.as_slice().into();
            membuffer.lock().push(buffer);
            anyhow::Ok(())
        });
        if let Err(e) = result {
            return Err(match e.downcast::<PyErr>() {
                Ok(e) => e,
                Err(e) => CompileError::new_err(format!("compilation failed\n----------\n{e}")),
            });
        }

        embedding_map.setattr("expects_return", has_return)?;

        let emit_llvm_bc = env::var(ENV_NAC3_EMIT_LLVM_BC).is_ok();
        let emit_llvm_ll = env::var(ENV_NAC3_EMIT_LLVM_LL).is_ok();

        let emit_llvm = |module: &Module<'_>, filename: &str| {
            if emit_llvm_bc {
                module.write_bitcode_to_path(Path::new(format!("{filename}.bc").as_str()));
            }
            if emit_llvm_ll {
                module.print_to_file(Path::new(format!("{filename}.ll").as_str())).unwrap();
            }
        };

        // Link all modules into `main`.
        let buffers = membuffers.lock();
        let main = context
            .create_module_from_ir(MemoryBuffer::create_from_memory_range(
                buffers.last().unwrap(),
                "main",
            ))
            .unwrap();
        emit_llvm(&main, "main");

        for buffer in buffers.iter().rev().skip(1) {
            let other = context
                .create_module_from_ir(MemoryBuffer::create_from_memory_range(buffer, "main"))
                .unwrap();

            main.link_in_module(other).map_err(|err| CompileError::new_err(err.to_string()))?;
        }
        drop(buffers);
        emit_llvm(&main, "main.merged");

        main.link_in_module(irrt).map_err(|err| CompileError::new_err(err.to_string()))?;
        emit_llvm(&main, "main.fat");

        let mut function_iter = main.get_first_function();
        while let Some(func) = function_iter {
            if func.count_basic_blocks() > 0 && func.get_name().to_str().unwrap() != "__modinit__" {
                func.set_linkage(Linkage::Private);
            }
            function_iter = func.get_next_function();
        }

        // Demote all global variables that will not be referenced in the kernel to private
        let preserved_symbols: Vec<&'static [u8]> = vec![b"typeinfo", b"now"];
        let mut global_option = main.get_first_global();
        while let Some(global) = global_option {
            if !preserved_symbols.contains(&(global.get_name().to_bytes())) {
                global.set_linkage(Linkage::Private);
            }
            global_option = global.get_next_global();
        }

        emit_llvm(&main, "main.pre-opt");

        let target_machine = self.codegen_options.target.create_target_machine();
        let pass_options = PassBuilderOptions::create();
        pass_options.set_merge_functions(true);
        let passes =
            format!("globaldce,strip-dead-prototypes,default<O{}>", self.codegen_options.opt_level);
        let result = main.run_passes(passes.as_str(), &target_machine, pass_options);
        if let Err(err) = result {
            panic!("Failed to run optimization for module `main`: {}", err.to_string());
        }
        emit_llvm(&main, "main.post-opt");

        Python::attach(|py| -> PyResult<()> {
            let string_store = self.string_store.read();
            let mut string_store_vec = string_store.iter().collect::<Vec<_>>();
            string_store_vec.sort_by_key(|(_, id)| *id);
            for (s, key) in string_store_vec {
                let embed_key: i32 = helper.store_str.bind(py).call1((s,))?.extract()?;
                assert_eq!(
                    embed_key, *key,
                    "string {s} is out of sync between embedding map (key={embed_key}) and \
                    the internal string store (key={key})"
                );
            }
            drop(string_store);
            Ok(())
        })?;

        link_fn(&main)
    }
}

/// Returns the (possibly qualified) path of a class name expression, or [`None`] if the class name
/// is not composed of a path.
///
/// The returned tuple contains the prefix path and the simple identifier of the class respectively.
fn class_expr_id_path(class_expr: &Located<ExprKind>) -> Option<(Vec<StrRef>, StrRef)> {
    match &class_expr.node {
        ExprKind::Name { id, .. } => Some((Vec::default(), *id)),
        ExprKind::Attribute { value, attr, .. } => {
            class_expr_id_path(value).map(|(prefix_path, prefix_attr)| {
                (prefix_path.into_iter().chain(once(prefix_attr)).collect(), *attr)
            })
        }
        ExprKind::Subscript { value, .. } => class_expr_id_path(value),
        _ => None,
    }
}

/// Returns the (possibly qualified) path of a decorator function, or [`None`] if the decorator is
/// not composed of a path.
///
/// The returned tuple contains the prefix path and the simple identifier of the decorator
/// respectively.
fn decor_expr_id_path(decor_expr: &Located<ExprKind>) -> Option<(Vec<StrRef>, StrRef)> {
    match &decor_expr.node {
        ExprKind::Name { id, .. } => {
            // Bare decorator
            Some((Vec::default(), *id))
        }
        ExprKind::Attribute { value, attr, .. } => {
            // Path-qualified decorator
            decor_expr_id_path(value).map(|(prefix_path, prefix_attr)| {
                (prefix_path.into_iter().chain(once(prefix_attr)).collect(), *attr)
            })
        }
        ExprKind::Call { func, .. } => {
            // Decorators that are calls (e.g. "@rpc()") have Call for the node,
            // need to extract the id from within.
            decor_expr_id_path(func)
        }
        _ => None,
    }
}

/// Resolves a possibly-qualified name consisting of the prefix `path` and identifier `id` in the
/// given global context `ctx`.
fn resolve_qname<'py>(
    (path, id): (Vec<StrRef>, StrRef),
    ctx: &Bound<'py, PyModule>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let resolve_ctx = |(path, id): (&[StrRef], StrRef), ctx: &Bound<'py, PyModule>| {
        let mut resolution_ctx = ctx.as_any().clone();
        for id in path {
            resolution_ctx = resolution_ctx.getattr(id.to_string())?;
        }

        resolution_ctx.getattr_opt(id.to_string())
    };

    resolve_ctx((&path, id), ctx)?.map_or_else(
        || resolve_ctx((&path, id), py_interp::builtins::module(ctx.py())?),
        |attr| Ok(Some(attr)),
    )
}

fn get_class_type<'py>(
    type_hint: &Located<ExprKind>,
    ctx: &Bound<'py, PyModule>,
) -> PyResult<Option<Bound<'py, PyType>>> {
    let Some((path, id)) = class_expr_id_path(type_hint) else {
        return Ok(None);
    };
    let obj = resolve_qname((path, id), ctx)?;

    if let Some(obj) = obj {
        if obj.is_instance_of::<PyType>() { Ok(Some(obj.cast_into()?)) } else { Ok(None) }
    } else {
        Ok(None)
    }
}

/// Returns the original function of the given `decorator` in the `ctx` global context.
fn get_decorator_fn<'py>(
    decorator: &Located<ExprKind>,
    ctx: &Bound<'py, PyModule>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some((path, id)) = decor_expr_id_path(decorator) else {
        return Ok(None);
    };

    resolve_qname((path, id), ctx).map(|obj| match obj {
        Some(ref o) if o.is_callable() => Ok(obj),
        Some(_) | None => Ok(None),
    })?
}

/// Checks whether the decorator expression in the `module` context refers to the same function
/// as any function in `decor_fn_ids`.
fn is_decor_fn_same(
    decorator: &Located<ExprKind>,
    module: &Bound<'_, PyModule>,
    decor_fn_ids: &[u64],
) -> PyResult<bool> {
    let decor_fn = get_decorator_fn(decorator, module)?;
    let fn_id = py_interp::extract_id(&decor_fn.into_pyobject(module.py())?)?;

    Ok(decor_fn_ids.contains(&fn_id))
}

fn link_with_ld(elf_filename: &str, obj_filename: &str) -> PyResult<()> {
    let linker_args = ["-shared", "--eh-frame-hdr", "-x", "-o", elf_filename, obj_filename];

    #[cfg(not(windows))]
    let ld_command = "ld";
    #[cfg(windows)]
    let ld_command = "ld.exe";
    match Command::new(ld_command).args(linker_args).status() {
        Ok(linker_status) => {
            if linker_status.success() {
                Ok(())
            } else {
                Err(CompileError::new_err("linker returned non-zero status code"))
            }
        }
        Err(err) => Err(CompileError::new_err(format!("failed to start linker: {err}"))),
    }
}

fn add_exceptions(
    composer: &mut TopLevelComposer,
    builtin_def: &mut HashMap<StrRef, DefinitionId>,
    builtin_ty: &mut HashMap<StrRef, Type>,
    error_names: &[&str],
) -> Vec<Type> {
    let mut types = Vec::new();
    // note: this is only for builtin exceptions, i.e. the exception name is "0:{exn}"
    for name in error_names {
        let def_id = composer.definition_ast_list.len();
        let (exception_fn, exception_class, exception_cons, exception_type) = get_exn_constructor(
            name,
            // class id
            def_id,
            // constructor id
            def_id + 1,
            &mut composer.unifier,
            &composer.primitives_ty,
        );
        composer.definition_ast_list.push((Arc::new(RwLock::new(exception_class)), None));
        composer.definition_ast_list.push((Arc::new(RwLock::new(exception_fn)), None));
        builtin_ty.insert((*name).into(), exception_cons);
        builtin_def.insert((*name).into(), DefinitionId(def_id));
        types.push(exception_type);
    }
    types
}

#[pymethods]
impl Nac3 {
    #[new]
    fn new(isa: &str, artiq_builtins: &Bound<'_, PyDict>) -> PyResult<Self> {
        let isa = match isa {
            "host" => Isa::Host,
            "rv32g" => Isa::RiscV32G,
            "rv32ima" => Isa::RiscV32IMA,
            "cortexa9" => Isa::CortexA9,
            _ => return Err(exceptions::PyValueError::new_err("invalid ISA")),
        };

        let opt_level = match env::var(ENV_NAC3_OPT_LEVEL) {
            Ok(x) if matches!(&*x, "0" | "1" | "2" | "3" | "s" | "z") => x,
            Err(env::VarError::NotPresent) => String::from("2"),
            unknown => {
                return Err(exceptions::PyValueError::new_err(format!(
                    "unknown opt level: {unknown:?}"
                )));
            }
        };
        // We always use the `Default` target-specific optimization level,
        // since `nac3ld` only supports relocation types that are used in optimized code.
        let target_opt_level = OptimizationLevel::Default;

        let target_options = isa.get_llvm_target_options(target_opt_level);
        let time_fns: &(dyn TimeFns + Sync) = match isa {
            Isa::RiscV32G => &timeline::NOW_PINNING_TIME_FNS_64,
            Isa::RiscV32IMA => &timeline::NOW_PINNING_TIME_FNS,
            Isa::CortexA9 | Isa::Host => &timeline::EXTERN_TIME_FNS,
        };
        let size_t_bits =
            target_options.create_target_machine().get_target_data().get_pointer_byte_size(None)
                * 8;

        let (primitive, _) = TopLevelComposer::make_unifier(size_t_bits);
        let builtins = vec![
            (
                "now_mu".into(),
                FunSignature { args: vec![], ret: primitive.int64, vars: VarMap::new() },
                Arc::new(GenCall::new(Box::new(move |ctx, _, _, _| {
                    Ok(Some(time_fns.emit_now_mu(ctx)?))
                }))),
            ),
            (
                "at_mu".into(),
                FunSignature {
                    args: vec![FuncArg {
                        name: "t".into(),
                        ty: primitive.int64,
                        default_value: None,
                        is_vararg: false,
                    }],
                    ret: primitive.none,
                    vars: VarMap::new(),
                },
                Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;
                    time_fns.emit_at_mu(ctx, arg)?;
                    Ok(None)
                }))),
            ),
            (
                "delay_mu".into(),
                FunSignature {
                    args: vec![FuncArg {
                        name: "dt".into(),
                        ty: primitive.int64,
                        default_value: None,
                        is_vararg: false,
                    }],
                    ret: primitive.none,
                    vars: VarMap::new(),
                },
                Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;
                    time_fns.emit_delay_mu(ctx, arg)?;
                    Ok(None)
                }))),
            ),
        ];

        let get_artiq_builtin_id = |mod_name: Option<&str>, name: &str| -> PyResult<_> {
            let dict = if let Some(mod_name) = mod_name {
                artiq_builtins
                    .get_item(mod_name)?
                    .unwrap_or_else(|| {
                        panic!("no module key '{mod_name}' present in artiq_builtins")
                    })
                    .cast_into::<PyDict>()?
            } else {
                artiq_builtins.clone()
            };

            let builtin = dict
                .get_item(name)?
                .unwrap_or_else(|| panic!("no key '{name}' present in artiq_builtins"));
            py_interp::extract_id(&builtin)
        };

        let primitive_ids = PrimitivePythonId {
            builtins: BuiltinPythonId {
                int: get_artiq_builtin_id(None, "int")?,
                float: get_artiq_builtin_id(None, "float")?,
                bool: get_artiq_builtin_id(None, "bool")?,
                str_class: get_artiq_builtin_id(None, "str")?,
                list: get_artiq_builtin_id(None, "list")?,
                tuple: get_artiq_builtin_id(None, "tuple")?,
                exception: get_artiq_builtin_id(None, "Exception")?,
                range: get_artiq_builtin_id(None, "range")?,
                enumerate: get_artiq_builtin_id(None, "enumerate")?,
                round: get_artiq_builtin_id(None, "round")?,
                round64: get_artiq_builtin_id(None, "round64")?,
                floor: get_artiq_builtin_id(None, "floor")?,
                floor64: get_artiq_builtin_id(None, "floor64")?,
                ceil: get_artiq_builtin_id(None, "ceil")?,
                ceil64: get_artiq_builtin_id(None, "ceil64")?,
                len: get_artiq_builtin_id(None, "len")?,
                min: get_artiq_builtin_id(None, "min")?,
                max: get_artiq_builtin_id(None, "max")?,
                abs: get_artiq_builtin_id(None, "abs")?,
                some: get_artiq_builtin_id(None, "some")?,
                staticmethod_decor_fn: get_artiq_builtin_id(None, "staticmethod")?,
            },
            types: TypesPythonId {
                generic_alias: get_artiq_builtin_id(Some("types"), "GenericAlias")?,
                module_type: get_artiq_builtin_id(Some("types"), "ModuleType")?,
            },
            typing: TypingPythonId {
                generic: get_artiq_builtin_id(Some("typing"), "Generic")?,
                generic_alias: get_artiq_builtin_id(Some("typing"), "_GenericAlias")?,
                typevar: get_artiq_builtin_id(Some("typing"), "TypeVar")?,
                literal: get_artiq_builtin_id(Some("typing"), "Literal")?,
            },
            numpy: NumpyPythonId {
                int32: get_artiq_builtin_id(Some("numpy"), "int32")?,
                int64: get_artiq_builtin_id(Some("numpy"), "int64")?,
                uint32: get_artiq_builtin_id(Some("numpy"), "uint32")?,
                uint64: get_artiq_builtin_id(Some("numpy"), "uint64")?,
                float64: get_artiq_builtin_id(Some("numpy"), "float64")?,
                bool_: get_artiq_builtin_id(Some("numpy"), "bool_")?,
                str_: get_artiq_builtin_id(Some("numpy"), "str_")?,
                ndarray: get_artiq_builtin_id(Some("numpy"), "ndarray")?,

                empty: get_artiq_builtin_id(Some("numpy"), "np_empty")?,
                zeros: get_artiq_builtin_id(Some("numpy"), "np_zeros")?,
                ones: get_artiq_builtin_id(Some("numpy"), "np_ones")?,
                full: get_artiq_builtin_id(Some("numpy"), "np_full")?,
                array: get_artiq_builtin_id(Some("numpy"), "np_array")?,
                eye: get_artiq_builtin_id(Some("numpy"), "np_eye")?,
                identity: get_artiq_builtin_id(Some("numpy"), "np_identity")?,

                size: get_artiq_builtin_id(Some("numpy"), "np_size")?,
                shape: get_artiq_builtin_id(Some("numpy"), "np_shape")?,

                broadcast_to: get_artiq_builtin_id(Some("numpy"), "np_broadcast_to")?,
                transpose: get_artiq_builtin_id(Some("numpy"), "np_transpose")?,
                reshape: get_artiq_builtin_id(Some("numpy"), "np_reshape")?,

                round: get_artiq_builtin_id(Some("numpy"), "np_round")?,
                floor: get_artiq_builtin_id(Some("numpy"), "np_floor")?,
                ceil: get_artiq_builtin_id(Some("numpy"), "np_ceil")?,
                min: get_artiq_builtin_id(Some("numpy"), "np_min")?,
                minimum: get_artiq_builtin_id(Some("numpy"), "np_minimum")?,
                max: get_artiq_builtin_id(Some("numpy"), "np_max")?,
                maximum: get_artiq_builtin_id(Some("numpy"), "np_maximum")?,
                argmin: get_artiq_builtin_id(Some("numpy"), "np_argmin")?,
                argmax: get_artiq_builtin_id(Some("numpy"), "np_argmax")?,
                isnan: get_artiq_builtin_id(Some("numpy"), "np_isnan")?,
                isinf: get_artiq_builtin_id(Some("numpy"), "np_isinf")?,
                sin: get_artiq_builtin_id(Some("numpy"), "np_sin")?,
                cos: get_artiq_builtin_id(Some("numpy"), "np_cos")?,
                exp: get_artiq_builtin_id(Some("numpy"), "np_exp")?,
                exp2: get_artiq_builtin_id(Some("numpy"), "np_exp2")?,
                log: get_artiq_builtin_id(Some("numpy"), "np_log")?,
                log10: get_artiq_builtin_id(Some("numpy"), "np_log10")?,
                log2: get_artiq_builtin_id(Some("numpy"), "np_log2")?,
                fabs: get_artiq_builtin_id(Some("numpy"), "np_fabs")?,
                sqrt: get_artiq_builtin_id(Some("numpy"), "np_sqrt")?,
                rint: get_artiq_builtin_id(Some("numpy"), "np_rint")?,
                tan: get_artiq_builtin_id(Some("numpy"), "np_tan")?,
                arcsin: get_artiq_builtin_id(Some("numpy"), "np_arcsin")?,
                arccos: get_artiq_builtin_id(Some("numpy"), "np_arccos")?,
                arctan: get_artiq_builtin_id(Some("numpy"), "np_arctan")?,
                sinh: get_artiq_builtin_id(Some("numpy"), "np_sinh")?,
                cosh: get_artiq_builtin_id(Some("numpy"), "np_cosh")?,
                tanh: get_artiq_builtin_id(Some("numpy"), "np_tanh")?,
                arcsinh: get_artiq_builtin_id(Some("numpy"), "np_arcsinh")?,
                arccosh: get_artiq_builtin_id(Some("numpy"), "np_arccosh")?,
                arctanh: get_artiq_builtin_id(Some("numpy"), "np_arctanh")?,
                expm1: get_artiq_builtin_id(Some("numpy"), "np_expm1")?,
                cbrt: get_artiq_builtin_id(Some("numpy"), "np_cbrt")?,

                spec_erf: get_artiq_builtin_id(Some("numpy"), "sp_spec_erf")?,
                spec_erfc: get_artiq_builtin_id(Some("numpy"), "sp_spec_erfc")?,
                spec_gamma: get_artiq_builtin_id(Some("numpy"), "sp_spec_gamma")?,
                spec_gammaln: get_artiq_builtin_id(Some("numpy"), "sp_spec_gammaln")?,
                spec_j0: get_artiq_builtin_id(Some("numpy"), "sp_spec_j0")?,
                spec_j1: get_artiq_builtin_id(Some("numpy"), "sp_spec_j1")?,

                arctan2: get_artiq_builtin_id(Some("numpy"), "np_arctan2")?,
                copysign: get_artiq_builtin_id(Some("numpy"), "np_copysign")?,
                fmax: get_artiq_builtin_id(Some("numpy"), "np_fmax")?,
                fmin: get_artiq_builtin_id(Some("numpy"), "np_fmin")?,
                ldexp: get_artiq_builtin_id(Some("numpy"), "np_ldexp")?,
                hypot: get_artiq_builtin_id(Some("numpy"), "np_hypot")?,
                nextafter: get_artiq_builtin_id(Some("numpy"), "np_nextafter")?,

                any: get_artiq_builtin_id(Some("numpy"), "np_any")?,
                all: get_artiq_builtin_id(Some("numpy"), "np_all")?,

                dot: get_artiq_builtin_id(Some("numpy"), "np_dot")?,
                linalg_cholesky: get_artiq_builtin_id(Some("numpy"), "np_linalg_cholesky")?,
                linalg_qr: get_artiq_builtin_id(Some("numpy"), "np_linalg_qr")?,
                linalg_svd: get_artiq_builtin_id(Some("numpy"), "np_linalg_svd")?,
                linalg_inv: get_artiq_builtin_id(Some("numpy"), "np_linalg_inv")?,
                linalg_pinv: get_artiq_builtin_id(Some("numpy"), "np_linalg_pinv")?,
                linalg_matrix_power: get_artiq_builtin_id(Some("numpy"), "np_linalg_matrix_power")?,
                linalg_det: get_artiq_builtin_id(Some("numpy"), "np_linalg_det")?,

                linalg_lu: get_artiq_builtin_id(Some("numpy"), "sp_linalg_lu")?,
                linalg_schur: get_artiq_builtin_id(Some("numpy"), "sp_linalg_schur")?,
                linalg_hessenberg: get_artiq_builtin_id(Some("numpy"), "sp_linalg_hessenberg")?,
            },
            artiq: ArtiqPythonId {
                auto: get_artiq_builtin_id(Some("artiq"), "Auto")?,
                kernel: get_artiq_builtin_id(Some("artiq"), "Kernel")?,
                kernel_invariant: get_artiq_builtin_id(Some("artiq"), "KernelInvariant")?,
                virtual_class: get_artiq_builtin_id(Some("artiq"), "virtual")?,
                none: get_artiq_builtin_id(Some("artiq"), "none")?,
                const_generic_marker: get_artiq_builtin_id(Some("artiq"), "_ConstGenericMarker")?,
                option: get_artiq_builtin_id(Some("artiq"), "Option")?,
                critical: get_artiq_builtin_id(Some("artiq"), "critical")?,
                compile_decor_fn: get_artiq_builtin_id(Some("artiq"), "compile")?,
                extern_decor_fn: get_artiq_builtin_id(Some("artiq"), "extern")?,
                kernel_decor_fn: get_artiq_builtin_id(Some("artiq"), "kernel")?,
                portable_decor_fn: get_artiq_builtin_id(Some("artiq"), "portable")?,
                rpc_decor_fn: get_artiq_builtin_id(Some("artiq"), "rpc")?,
            },
        };

        let working_directory = tempfile::Builder::new().prefix("nac3-").tempdir().unwrap();
        fs::write(working_directory.path().join("kernel.ld"), include_bytes!("kernel.ld")).unwrap();

        let mut string_store: HashMap<String, i32> = HashMap::default();

        // Preallocate runtime exception names
        for (i, name) in RUNTIME_EXCEPTION_NAMES.iter().enumerate() {
            let exn_name = if name.find(':').is_none() {
                format!("0:artiq.coredevice.exceptions.{name}")
            } else {
                (*name).to_string()
            };

            let id = i32::try_from(i).unwrap();
            string_store.insert(exn_name, id);
        }

        let codegen_options =
            CodeGenOptions { debug: opt_level == "0", target: target_options, opt_level };

        Ok(Self {
            isa,
            time_fns,
            primitive,
            builtins,
            primitive_ids,
            top_levels: Vec::default(),
            pyid_to_def: Arc::default(),
            working_directory,
            string_store: Arc::new(string_store.into()),
            exception_ids: Arc::default(),
            special_ids: SpecialPythonId::default(),
            modules: Arc::default(),
            codegen_options,
        })
    }

    fn analyze<'py>(
        &mut self,
        functions: &Bound<'py, PyDict>,
        classes: &Bound<'py, PyDict>,
        special_ids: &Bound<'py, PyDict>,
    ) -> PyResult<()> {
        let (modules, class_ids) = {
            let mut modules: IndexMap<u64, Arc<Py<PyModule>>> = IndexMap::new();
            let mut class_ids: HashSet<u64> = HashSet::new();

            for (_, module) in functions {
                if !module.is_none() {
                    modules.insert(
                        py_interp::extract_id(module.cast::<PyModule>()?)?,
                        Arc::new(module.cast_into::<PyModule>()?.unbind()),
                    );
                }
            }

            for (class, module) in classes {
                if !module.is_none() {
                    modules.insert(
                        py_interp::extract_id(module.cast::<PyModule>()?)?,
                        Arc::new(module.cast_into::<PyModule>()?.unbind()),
                    );
                }
                class_ids.insert(py_interp::extract_id(&class)?);
            }

            (modules, class_ids)
        };

        for module in modules.into_values() {
            self.register_module(&module, &class_ids)?;
        }

        let get_special_ids =
            |name: &str| -> PyResult<_> { special_ids.get_item(name)?.unwrap().extract::<u64>() };

        self.special_ids = SpecialPythonId {
            parallel: get_special_ids("parallel")?,
            legacy_parallel: get_special_ids("legacy_parallel")?,
            sequential: get_special_ids("sequential")?,
        };

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_method_to_file<'py>(
        &mut self,
        obj: &Bound<'py, PyAny>,
        method_name: &str,
        args: Vec<Bound<'py, PyAny>>,
        output_filename: &str,
        debug_filename: Option<&str>,
        embedding_map: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> PyResult<()> {
        let target_machine = self.codegen_options.target.create_target_machine();
        let link_fn = |module: &Module| {
            if self.isa == Isa::Host {
                let working_directory = self.working_directory.path().to_owned();
                target_machine
                    .write_to_file(module, FileType::Object, &working_directory.join("module.o"))
                    .expect("couldn't write module to file");
                link_with_ld(
                    output_filename,
                    working_directory.join("module.o").to_string_lossy().to_string().as_str(),
                )?;
                Ok(())
            } else {
                let object_mem = target_machine
                    .write_to_memory_buffer(module, FileType::Object)
                    .expect("couldn't write module to object file buffer");
                if let Some(path) = env::var_os(ENV_NAC3_EMIT_OBJ) {
                    fs::write(&path, object_mem.as_slice()).map_err(CompileError::new_err)?;
                }
                Linker::ld(object_mem.as_slice()).map_or_else(
                    |e| {
                        Err(CompileError::new_err(format!(
                            "linker failed to process object file: {e:?}"
                        )))
                    },
                    |(dyn_lib, debug_lib)| {
                        if let Some(path) = env::var_os(ENV_NAC3_EMIT_ELF) {
                            fs::write(&path, &dyn_lib).map_err(CompileError::new_err)?;
                        }
                        if let Some(path) = env::var_os(ENV_NAC3_EMIT_DEBUG_ELF) {
                            fs::write(&path, &debug_lib).map_err(CompileError::new_err)?;
                        }
                        fs::File::create(output_filename)
                            .map_or_else(
                                |_| Err(CompileError::new_err("failed to create object file")),
                                |mut elf_file| {
                                    elf_file
                                        .write_all(&dyn_lib)
                                        .expect("couldn't write linked library to file");
                                    Ok(())
                                },
                            )
                            .and(debug_filename.map_or(Ok(()), |filename| {
                                fs::File::create(filename).map_or_else(
                                    |_| Err(CompileError::new_err("failed to create debug file")),
                                    |mut debug_file| {
                                        debug_file
                                            .write_all(&debug_lib)
                                            .expect("couldn't write debug library to file");
                                        Ok(())
                                    },
                                )
                            }))
                    },
                )
            }
        };

        self.compile_method(obj, method_name, args, embedding_map, py, &link_fn)?;
        Ok(())
    }

    fn compile_method_to_mem<'py>(
        &mut self,
        obj: &Bound<'py, PyAny>,
        method_name: &str,
        args: Vec<Bound<'py, PyAny>>,
        embedding_map: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let target_machine = self.codegen_options.target.create_target_machine();
        let link_fn = |module: &Module| {
            if self.isa == Isa::Host {
                let working_directory = self.working_directory.path().to_owned();
                target_machine
                    .write_to_file(module, FileType::Object, &working_directory.join("module.o"))
                    .expect("couldn't write module to file");

                let filename_path = self.working_directory.path().join("module.elf");
                let filename = filename_path.to_str().unwrap();
                link_with_ld(
                    filename,
                    working_directory.join("module.o").to_string_lossy().to_string().as_str(),
                )?;

                Ok(PyTuple::new(py, vec![fs::read(filename).unwrap()]))
            } else {
                let object_mem = target_machine
                    .write_to_memory_buffer(module, FileType::Object)
                    .expect("couldn't write module to object file buffer");
                if let Some(path) = env::var_os(ENV_NAC3_EMIT_OBJ) {
                    fs::write(&path, object_mem.as_slice()).map_err(CompileError::new_err)?;
                }
                Linker::ld(object_mem.as_slice()).map_or_else(
                    |e| {
                        Err(CompileError::new_err(format!(
                            "linker failed to process object file: {e:?}"
                        )))
                    },
                    |(dyn_lib, debug_lib)| {
                        if let Some(path) = env::var_os(ENV_NAC3_EMIT_ELF) {
                            fs::write(&path, &dyn_lib).map_err(CompileError::new_err)?;
                        }
                        if let Some(path) = env::var_os(ENV_NAC3_EMIT_DEBUG_ELF) {
                            fs::write(&path, &debug_lib).map_err(CompileError::new_err)?;
                        }
                        Ok(PyTuple::new(py, vec![dyn_lib, debug_lib]))
                    },
                )
            }
        };

        self.compile_method(obj, method_name, args, embedding_map, py, &link_fn)?
    }
}

// The names of the exceptions follow one of two schemes:
//
// - Python builtin exceptions are always prefixed with `0:`. When thrown on-device, these
//   exceptions are thrown verbatim correspondingly on the host.
// - ARTIQ-specific runtime exceptions are referred to by their class name (they are
//   implicitly prefixed with `artiq.coredevice.exceptions` - see below). When thrown
//   on-device, these exceptions are mapped into their corresponding reserved exception ID
//   in `EXCEPTION_ID_LOOKUP` (located in `artiq::firmware::ksupport::eh_artiq`) and thrown.
//   The host then looks up the exception ID in the string store of the embedding map to
//   obtain the fully-qualified exception class name, and throws the corresponding
//   exception.
//
// This list of exception must be kept in sync with `EXCEPTION_ID_LOOKUP`, and all
// exceptions declared here must be defined in `artiq.coredevice.exceptions`.
//
// Synchronization shall be verified by running the test cases in
// `artiq.test.coredevice.test_exceptions`.
const RUNTIME_EXCEPTION_NAMES: &[&str] = &[
    "RTIOUnderflow",
    "RTIOOverflow",
    "RTIODestinationUnreachable",
    "DMAError",
    "I2CError",
    "CacheError",
    "SPIError",
    "SubkernelError",
    "0:AssertionError",
    "0:AttributeError",
    "0:IndexError",
    "0:IOError",
    "0:KeyError",
    "0:NotImplementedError",
    "0:OverflowError",
    "0:RuntimeError",
    "0:TimeoutError",
    "0:TypeError",
    "0:ValueError",
    "0:ZeroDivisionError",
    "0:LinAlgError",
    "0:MemoryError",
    "UnwrapNoneError",
    "CXPError",
    "GrabberSerialError",
];

#[pyclass]
#[pyo3(name = "CallRecord")]
struct CallRecordWrapper {
    #[pyo3(get)]
    name: &'static str,
    #[pyo3(get)]
    address: Option<u32>,
    #[pyo3(get)]
    line: u32,
    #[pyo3(get)]
    column: u32,
    #[pyo3(get)]
    file: &'static str,
    #[pyo3(get)]
    dir: Option<&'static str>,
}

impl From<&symbolizer::CallRecord> for CallRecordWrapper {
    fn from(rec: &symbolizer::CallRecord) -> Self {
        Self {
            name: rec.get_name(),
            address: rec.address,
            line: rec.line,
            column: rec.column,
            file: rec.file,
            dir: rec.dir,
        }
    }
}

#[pyfunction]
fn symbolize<'py>(
    elf_bin: &Bound<'py, PyBytes>,
    pc: &Bound<'py, PyList>,
) -> PyResult<Vec<CallRecordWrapper>> {
    // Backtrace addresses are u32 on the wire, but the host reads them as signed
    // i32 (and offsets them by -1), so addresses with the top bit set arrive as
    // negative ints. Wrap them back into u32 instead of rejecting them.
    let pc = pc.extract::<Vec<i64>>()?.into_iter().map(|pc| pc as u32).collect();
    Ok(symbolizer::symbolize(elf_bin.extract()?, pc).iter().map(Into::into).collect())
}

#[cfg(feature = "init-llvm-profile")]
unsafe extern "C" {
    fn __llvm_profile_initialize();
}

#[pymodule]
fn nac3artiq<'py>(py: Python<'py>, m: &Bound<'py, PyModule>) -> PyResult<()> {
    #[cfg(feature = "init-llvm-profile")]
    unsafe {
        __llvm_profile_initialize();
    }

    Target::initialize_all(&InitializationConfig::default());
    m.add("CompileError", py.get_type::<CompileError>())?;
    m.add("RUNTIME_EXCEPTION_NAMES", RUNTIME_EXCEPTION_NAMES)?;
    m.add_class::<Nac3>()?;
    m.add_class::<CallRecordWrapper>()?;
    m.add_function(wrap_pyfunction!(symbolize, m)?)?;
    Ok(())
}
