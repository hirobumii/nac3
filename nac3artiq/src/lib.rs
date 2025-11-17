#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::{
    cell::{LazyCell, OnceCell},
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    iter::once,
    path::Path,
    process::Command,
    rc::Rc,
    sync::Arc,
};

use indexmap::IndexMap;
use itertools::Itertools;
use parking_lot::{Mutex, RwLock};
use pyo3::{
    IntoPyObjectExt, create_exception, exceptions,
    prelude::*,
    types::{PyAnyMethods, PyBytes, PyDict, PyNone, PyType},
};
use tempfile::{self, TempDir};

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
        composer::{BuiltinFuncCreator, BuiltinFuncSpec, ComposerConfig, TopLevelComposer},
        helper::get_decorator_flags,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, FuncArg, Type, TypeEnum, Unifier, VarMap, into_var_map},
    },
};
use nac3ld::Linker;

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
            Isa::Host => TargetMachine::get_default_triple(),
            Isa::RiscV32G | Isa::RiscV32IMA => TargetTriple::create("riscv32-unknown-linux"),
            Isa::CortexA9 => TargetTriple::create("armv7-unknown-linux-eabihf"),
        }
    }

    /// Returns the [`String`] representing the target CPU used for compiling to this ISA.
    pub fn get_llvm_target_cpu(self) -> String {
        match self {
            Isa::Host => TargetMachine::get_host_cpu_name().to_string(),
            Isa::RiscV32G | Isa::RiscV32IMA => "generic-rv32".to_string(),
            Isa::CortexA9 => "cortex-a9".to_string(),
        }
    }

    /// Returns the [`String`] representing the target features used for compiling to this ISA.
    pub fn get_llvm_target_features(self) -> String {
        match self {
            Isa::Host => TargetMachine::get_host_cpu_features().to_string(),
            Isa::RiscV32G => "+a,+m,+f,+d".to_string(),
            Isa::RiscV32IMA => "+a,+m".to_string(),
            Isa::CortexA9 => "+dsp,+fp16,+neon,+vfp3,+long-calls".to_string(),
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

#[derive(Clone)]
pub struct BuiltinPythonId {
    int: u64,
    float: u64,
    bool: u64,
    str_class: u64,
    list: u64,
    tuple: u64,
    exception: u64,
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
}

#[derive(Clone)]
pub struct ArtiqPythonId {
    kernel: u64,
    kernel_invariant: u64,
    const_generic_marker: u64,
    none: u64,
    virtual_class: u64,
    option: u64,

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
    deferred_eval_store: DeferredEvaluationStore,
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
                    let nac3_class = Python::attach(|py| {
                        let module = module.bind(py);

                        decorator_list.iter().any(|decorator| {
                            is_decor_fn_same(
                                decorator,
                                module,
                                &[self.primitive_ids.artiq.compile_decor_fn],
                            )
                            .unwrap()
                        })
                    });

                    if !nac3_class {
                        continue;
                    }

                    // Drop unregistered (i.e. host-only) base classes.
                    bases.retain(|base| {
                        Python::attach(|py| -> PyResult<bool> {
                            let Some((path, id)) = class_expr_id_path(base) else {
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
                        })
                        .unwrap()
                    });

                    body.retain(|stmt| {
                        if let StmtKind::FunctionDef { ref decorator_list, .. } = stmt.node {
                            Python::attach(|py| {
                                let module = module.bind(py);

                                // Keep all class functions decorated with `kernel`, `portable`, or `rpc` decorator
                                decorator_list.iter().any(|decorator| {
                                    is_decor_fn_same(
                                        decorator,
                                        module,
                                        &[
                                            self.primitive_ids.artiq.kernel_decor_fn,
                                            self.primitive_ids.artiq.portable_decor_fn,
                                            self.primitive_ids.artiq.rpc_decor_fn,
                                        ],
                                    )
                                    .unwrap()
                                })
                            })
                        } else {
                            true
                        }
                    });

                    true
                }
                StmtKind::FunctionDef { ref decorator_list, .. } => {
                    Python::attach(|py| {
                        let module = module.bind(py);

                        // Keep all top-level functions decorated with `extern`, `kernel`, `portable`, or `rpc` decorator
                        decorator_list.iter().any(|decorator| {
                            is_decor_fn_same(
                                decorator,
                                module,
                                &[
                                    self.primitive_ids.artiq.extern_decor_fn,
                                    self.primitive_ids.artiq.kernel_decor_fn,
                                    self.primitive_ids.artiq.portable_decor_fn,
                                    self.primitive_ids.artiq.rpc_decor_fn,
                                ],
                            )
                            .unwrap()
                        })
                    })
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
        let modules_by_path = LazyCell::new(|| {
            self.modules
                .read()
                .iter()
                .map(|mod_info| (mod_info.file, mod_info.module.clone()))
                .collect::<HashMap<_, _>>()
        });

        let (mut composer, mut builtins_def, mut builtins_ty) = TopLevelComposer::new(
            self.builtins.clone(),
            Self::get_lateinit_builtins(),
            ComposerConfig {
                has_generic_ann_fn: Box::new(|ann| {
                    Python::attach(|py| {
                        is_class_ann_same(
                            ann,
                            modules_by_path[&ann.location.file].bind(py),
                            self.primitive_ids.typing.generic,
                        )
                    })
                    .map_err(|e| e.to_string())
                }),
                has_kernel_ann_fn: Some(Box::new(|ann| {
                    Python::attach(|py| {
                        is_class_ann_same(
                            ann,
                            modules_by_path[&ann.location.file].bind(py),
                            self.primitive_ids.artiq.kernel,
                        )
                    })
                    .map_err(|e| e.to_string())
                })),
                has_invariant_ann_fn: Box::new(|ann| {
                    Python::attach(|py| {
                        is_class_ann_same(
                            ann,
                            modules_by_path[&ann.location.file].bind(py),
                            self.primitive_ids.artiq.kernel_invariant,
                        )
                    })
                    .map_err(|e| e.to_string())
                }),
                is_extern_decorator_fn: Box::new(|decorator| {
                    Python::attach(|py| -> PyResult<bool> {
                        is_decor_fn_same(
                            decorator,
                            modules_by_path[&decorator.location.file].bind(py),
                            &[
                                self.primitive_ids.artiq.extern_decor_fn,
                                self.primitive_ids.artiq.rpc_decor_fn,
                            ],
                        )
                    })
                    .map_err(|e| e.to_string())
                }),
                is_static_method_decorator_fn: Box::new(|decorator| {
                    Python::attach(|py| -> PyResult<bool> {
                        is_decor_fn_same(
                            decorator,
                            modules_by_path[&decorator.location.file].bind(py),
                            &[self.primitive_ids.builtins.staticmethod_decor_fn],
                        )
                    })
                    .map_err(|e| e.to_string())
                }),
            },
            size_t,
        );

        let store_obj = embedding_map.getattr("store_object").unwrap();
        let store_str = embedding_map.getattr("store_str").unwrap();
        let store_fun = embedding_map.getattr("store_function").unwrap().into_py_any(py)?;
        let host_attributes =
            embedding_map.getattr("attributes_writeback").unwrap().into_py_any(py)?;
        let global_value_ids: Arc<RwLock<HashMap<_, _>>> = Arc::new(RwLock::new(HashMap::new()));
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
                    let key: &str = key.extract().unwrap();
                    let val = py_interp::extract_id(&val).unwrap();
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
                    deferred_eval_store: self.deferred_eval_store.clone(),
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
                let class = py_module.getattr(name.to_string().as_str()).unwrap();
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
                            store_fun
                                .call1(
                                    py,
                                    (
                                        def_id.0.into_py_any(py)?,
                                        py_module.getattr(name.to_string()).unwrap(),
                                    ),
                                )
                                .unwrap();
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
                    let class_obj = py_module.getattr(class_name.as_str()).unwrap();
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

            let id = *name_to_pyid.get(&name).unwrap();
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
            deferred_eval_store: self.deferred_eval_store.clone(),
        });
        let resolver =
            Arc::new(Resolver(inner_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;
        let (_, def_id, _) = composer
            .register_top_level(synthesized.pop().unwrap(), Some(resolver.clone()), "", false)
            .unwrap();

        // Process IRRT
        context_ref!(context);
        let irrt = load_irrt(context, resolver.as_ref());

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
            return if e.iter().any(|err| err.contains("<nac3_synthesized_modinit>")) {
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
                    msg.unwrap_or(e.iter().sorted().join("\n----------\n"))
                )))
            } else {
                Err(CompileError::new_err(format!(
                    "compilation failed\n----------\n{}",
                    e.iter().sorted().join("\n----------\n"),
                )))
            };
        }
        let top_level = Arc::new(composer.make_top_level_context());

        {
            let defs = top_level.definitions.read();
            for (class_data, id, is_async) in &rpc_ids {
                let mut def = defs[id.0].write();
                match &mut *def {
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
                                store_fun
                                    .call1(
                                        py,
                                        (
                                            id.0.into_py_any(py)?,
                                            class_def.getattr(name.to_string().as_str()).unwrap(),
                                        ),
                                    )
                                    .unwrap();
                            }
                        }
                    }
                    TopLevelDef::Module { .. } => {
                        unreachable!("Type module cannot be decorated with @rpc")
                    }
                }
            }
        }

        let instance = {
            let defs = top_level.definitions.read();
            let mut definition = defs[def_id.0].write();
            let TopLevelDef::Function { instance_to_stmt, instance_to_symbol, .. } =
                &mut *definition
            else {
                unreachable!()
            };

            instance_to_symbol.insert(String::new(), "__modinit__".into());
            instance_to_stmt[""].clone()
        };

        let task = CodeGenTask {
            subst: Vec::default(),
            symbol_name: "__modinit__".to_string(),
            export_symbol: true,
            body: Arc::new(Vec::default()),
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
                    s.to_string(),
                    self.time_fns,
                    self.special_ids.clone(),
                ))
            })
            .collect();

        let membuffer = membuffers.clone();
        let mut has_return = false;
        py.detach(|| {
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
            let context = ModuleContext::new(context, "main", &self.codegen_options.target);
            let builder = context.ctx.create_builder();
            let mut unifier_cache = vec![OnceCell::new(); top_level.unifiers.read().len()];

            let (context, _, result) = gen_func_impl(
                context,
                builder,
                &mut generator,
                &registry,
                task,
                &mut unifier_cache,
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
            );
            result.unwrap();
            let buffer = context.module.write_bitcode_to_memory();
            let buffer = buffer.as_slice().into();
            membuffer.lock().push(buffer);
        });

        embedding_map.setattr("expects_return", has_return).unwrap();

        let emit_llvm_bc = std::env::var(ENV_NAC3_EMIT_LLVM_BC).is_ok();
        let emit_llvm_ll = std::env::var(ENV_NAC3_EMIT_LLVM_LL).is_ok();

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

        Python::attach(|py| {
            let string_store = self.string_store.read();
            let mut string_store_vec = string_store.iter().collect::<Vec<_>>();
            string_store_vec.sort_by(|(_s1, key1), (_s2, key2)| key1.cmp(key2));
            for (s, key) in string_store_vec {
                let embed_key: i32 =
                    helper.store_str.bind(py).call1((s,)).unwrap().extract().unwrap();
                assert_eq!(
                    embed_key, *key,
                    "string {s} is out of sync between embedding map (key={embed_key}) and \
                    the internal string store (key={key})"
                );
            }
        });

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

    Ok(resolve_qname((path, id), ctx)?.map(Bound::cast_into).transpose()?)
}

/// Returns the original function of the given `decorator` in the `ctx` global context.
fn get_decorator_fn<'py>(
    decorator: &Located<ExprKind>,
    ctx: &Bound<'py, PyModule>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some((path, id)) = decor_expr_id_path(decorator) else {
        return Ok(None);
    };

    resolve_qname((path, id), ctx).inspect(|decorator| {
        assert!(decorator.as_ref().is_none_or(PyAnyMethods::is_callable));
    })
}

fn is_class_ann_same(
    type_hint: &Located<ExprKind>,
    module: &Bound<'_, PyModule>,
    ann_class_id: u64,
) -> PyResult<bool> {
    let class_obj = get_class_type(type_hint, module)?;
    let class_id = py_interp::extract_id(&class_obj.into_pyobject(module.py())?)?;

    Ok(class_id == ann_class_id)
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

fn link_with_lld(elf_filename: &str, obj_filename: &str) -> PyResult<()> {
    let linker_args = ["-shared", "--eh-frame-hdr", "-x", "-o", elf_filename, obj_filename];

    #[cfg(not(windows))]
    let lld_command = "ld.lld";
    #[cfg(windows)]
    let lld_command = "ld.lld.exe";
    match Command::new(lld_command).args(linker_args).status() {
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

        let opt_level = match std::env::var(ENV_NAC3_OPT_LEVEL) {
            Ok(x) if matches!(&*x, "0" | "1" | "2" | "3" | "s" | "z") => x,
            Err(std::env::VarError::NotPresent) => String::from("2"),
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

        let (primitive, _) = TopLevelComposer::make_primitives(size_t_bits);
        let builtins = vec![
            (
                "now_mu".into(),
                FunSignature { args: vec![], ret: primitive.int64, vars: VarMap::new() },
                Arc::new(GenCall::new(Box::new(move |ctx, _, _, _| {
                    Ok(Some(time_fns.emit_now_mu(ctx)))
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
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty).unwrap();
                    time_fns.emit_at_mu(ctx, arg);
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
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty).unwrap();
                    time_fns.emit_delay_mu(ctx, arg);
                    Ok(None)
                }))),
            ),
        ];

        let get_artiq_builtin_id = |mod_name: Option<&str>, name: &str| -> PyResult<u64> {
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
            },
            artiq: ArtiqPythonId {
                kernel: get_artiq_builtin_id(Some("artiq"), "Kernel")?,
                kernel_invariant: get_artiq_builtin_id(Some("artiq"), "KernelInvariant")?,
                virtual_class: get_artiq_builtin_id(Some("artiq"), "virtual")?,
                none: get_artiq_builtin_id(Some("artiq"), "none")?,
                const_generic_marker: get_artiq_builtin_id(Some("artiq"), "_ConstGenericMarker")?,
                option: get_artiq_builtin_id(Some("artiq"), "Option")?,
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

        // Keep this list of exceptions in sync with `EXCEPTION_ID_LOOKUP` in `artiq::firmware::ksupport::eh_artiq`
        // The exceptions declared here must be defined in `artiq.coredevice.exceptions`
        // Verify synchronization by running the test cases in `artiq.test.coredevice.test_exceptions`
        let runtime_exception_names = [
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
            "UnwrapNoneError",
            "CXPError",
        ];

        // Preallocate runtime exception names
        for (i, name) in runtime_exception_names.iter().enumerate() {
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

        Ok(Nac3 {
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
            deferred_eval_store: DeferredEvaluationStore::new(),
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
            |name: &str| -> PyResult<u64> { special_ids.get_item(name)?.unwrap().extract::<u64>() };

        self.special_ids = SpecialPythonId {
            parallel: get_special_ids("parallel")?,
            legacy_parallel: get_special_ids("legacy_parallel")?,
            sequential: get_special_ids("sequential")?,
        };

        Ok(())
    }

    fn compile_method_to_file<'py>(
        &mut self,
        obj: &Bound<'py, PyAny>,
        method_name: &str,
        args: Vec<Bound<'py, PyAny>>,
        filename: &str,
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
                link_with_lld(
                    filename,
                    working_directory.join("module.o").to_string_lossy().to_string().as_str(),
                )?;
                Ok(())
            } else {
                let object_mem = target_machine
                    .write_to_memory_buffer(module, FileType::Object)
                    .expect("couldn't write module to object file buffer");
                if let Some(path) = std::env::var_os("NAC3_EMIT_OBJ") {
                    std::fs::write(&path, object_mem.as_slice()).map_err(CompileError::new_err)?;
                }
                if let Ok(dyn_lib) = Linker::ld(object_mem.as_slice()) {
                    if let Ok(mut file) = fs::File::create(filename) {
                        file.write_all(&dyn_lib).expect("couldn't write linked library to file");
                        Ok(())
                    } else {
                        Err(CompileError::new_err("failed to create file"))
                    }
                } else {
                    Err(CompileError::new_err("linker failed to process object file"))
                }
            }
        };

        self.compile_method(obj, method_name, args, embedding_map, py, &link_fn)
    }

    fn compile_method_to_mem<'py>(
        &mut self,
        obj: &Bound<'py, PyAny>,
        method_name: &str,
        args: Vec<Bound<'py, PyAny>>,
        embedding_map: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Py<PyAny>> {
        let target_machine = self.codegen_options.target.create_target_machine();
        let link_fn = |module: &Module| {
            if self.isa == Isa::Host {
                let working_directory = self.working_directory.path().to_owned();
                target_machine
                    .write_to_file(module, FileType::Object, &working_directory.join("module.o"))
                    .expect("couldn't write module to file");

                let filename_path = self.working_directory.path().join("module.elf");
                let filename = filename_path.to_str().unwrap();
                link_with_lld(
                    filename,
                    working_directory.join("module.o").to_string_lossy().to_string().as_str(),
                )?;

                Ok(PyBytes::new(py, &fs::read(filename).unwrap()).into())
            } else {
                let object_mem = target_machine
                    .write_to_memory_buffer(module, FileType::Object)
                    .expect("couldn't write module to object file buffer");
                if let Ok(dyn_lib) = Linker::ld(object_mem.as_slice()) {
                    Ok(PyBytes::new(py, &dyn_lib).into())
                } else {
                    Err(CompileError::new_err("linker failed to process object file"))
                }
            }
        };

        self.compile_method(obj, method_name, args, embedding_map, py, &link_fn)
    }
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
    m.add_class::<Nac3>()?;
    Ok(())
}
