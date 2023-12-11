use clap::Parser;
use inkwell::{
    memory_buffer::MemoryBuffer,
    passes::PassBuilderOptions,
    support::is_multithreaded,
    targets::*,
    OptimizationLevel,
};
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, fs, path::Path, sync::Arc};

use nac3core::{
    codegen::{
        concrete_type::ConcreteTypeStore, irrt::load_irrt, CodeGenLLVMOptions,
        CodeGenTargetMachineOptions, CodeGenTask, DefaultCodeGenerator, WithCall, WorkerRegistry,
    },
    symbol_resolver::SymbolResolver,
    toplevel::{
        composer::TopLevelComposer, helper::parse_parameter_default_value, type_annotation::*,
        TopLevelDef,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, Type, Unifier},
    },
};
use nac3parser::{
    ast::{Constant, Expr, ExprKind, StmtKind, StrRef},
    parser,
};

mod basic_symbol_resolver;
use basic_symbol_resolver::*;
use nac3core::toplevel::composer::ComposerConfig;

/// Command-line argument parser definition.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CommandLineArgs {
    /// The name of the input file.
    file_name: String,

    /// The number of threads allocated to processing the source file. If 0 is passed to this
    /// parameter, all available threads will be used for compilation.
    #[arg(short = 'T', default_value_t = 1)]
    threads: u32,

    /// The level to optimize the LLVM IR.
    #[arg(short = 'O', default_value_t = 2, value_parser = clap::value_parser!(u32).range(0..=3))]
    opt_level: u32,

    /// Whether to emit LLVM IR at the end of every module.
    ///
    /// If multithreaded compilation is also enabled, each thread will emit its own module.
    #[arg(long, default_value_t = false)]
    emit_llvm: bool,

    /// The target triple to compile for.
    #[arg(long)]
    triple: Option<String>,

    /// The target CPU to compile for.
    #[arg(long)]
    mcpu: Option<String>,

    /// Additional target features to enable/disable, specified using the `+`/`-` prefixes.
    #[arg(long)]
    target_features: Option<String>,
}

fn handle_typevar_definition(
    var: &Expr,
    resolver: &(dyn SymbolResolver + Send + Sync),
    def_list: &[Arc<RwLock<TopLevelDef>>],
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
) -> Result<Type, String> {
    let ExprKind::Call { func, args, .. } = &var.node else {
        return Err(format!(
            "expression {var:?} cannot be handled as a generic parameter in global scope"
        ))
    };

    match &func.node {
        ExprKind::Name { id, .. } if id == &"TypeVar".into() => {
            let ExprKind::Constant { value: Constant::Str(ty_name), .. } = &args[0].node else {
                return Err(format!("Expected string constant for first parameter of `TypeVar`, got {:?}", &args[0].node))
            };
            let generic_name: StrRef = ty_name.to_string().into();

            let constraints = args
                .iter()
                .skip(1)
                .map(|x| -> Result<Type, String> {
                    let ty = parse_ast_to_type_annotation_kinds(
                        resolver,
                        def_list,
                        unifier,
                        primitives,
                        x,
                        HashMap::default(),
                        None,
                    )?;
                    get_type_from_type_annotation_kinds(
                        def_list, unifier, &ty, &mut None
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let loc = func.location;

            if constraints.len() == 1 {
                return Err(format!("A single constraint is not allowed (at {loc})"))
            }

            Ok(unifier.get_fresh_var_with_range(&constraints, Some(generic_name), Some(loc)).0)
        }

        ExprKind::Name { id, .. } if id == &"ConstGeneric".into() => {
            if args.len() != 2 {
                return Err(format!("Expected 2 arguments for `ConstGeneric`, got {}", args.len()))
            }

            let ExprKind::Constant { value: Constant::Str(ty_name), .. } = &args[0].node else {
                return Err(format!(
                    "Expected string constant for first parameter of `ConstGeneric`, got {:?}",
                    &args[0].node
                ))
            };
            let generic_name: StrRef = ty_name.to_string().into();

            let ty = parse_ast_to_type_annotation_kinds(
                resolver,
                def_list,
                unifier,
                primitives,
                &args[1],
                HashMap::default(),
                None,
            )?;
            let constraint = get_type_from_type_annotation_kinds(
                def_list, unifier, &ty, &mut None
            )?;
            let loc = func.location;

            Ok(unifier.get_fresh_const_generic_var(constraint, Some(generic_name), Some(loc)).0)
        }

        _ => Err(format!(
            "expression {var:?} cannot be handled as a generic parameter in global scope"
        ))
    }
}

fn handle_assignment_pattern(
    targets: &[Expr],
    value: &Expr,
    resolver: &(dyn SymbolResolver + Send + Sync),
    internal_resolver: &ResolverInternal,
    def_list: &[Arc<RwLock<TopLevelDef>>],
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
) -> Result<(), String> {
    if targets.len() == 1 {
        match &targets[0].node {
            ExprKind::Name { id, .. } => {
                if let Ok(var) = handle_typevar_definition(
                    value,
                    resolver,
                    def_list,
                    unifier,
                    primitives,
                ) {
                    internal_resolver.add_id_type(*id, var);
                    Ok(())
                } else if let Ok(val) =
                    parse_parameter_default_value(value, resolver)
                {
                    internal_resolver.add_module_global(*id, val);
                    Ok(())
                } else {
                    Err(format!("fails to evaluate this expression `{:?}` as a constant or generic parameter at {}",
                        targets[0].node,
                        targets[0].location,
                    ))
                }
            }
            ExprKind::List { elts, .. } | ExprKind::Tuple { elts, .. } => {
                handle_assignment_pattern(
                    elts,
                    value,
                    resolver,
                    internal_resolver,
                    def_list,
                    unifier,
                    primitives,
                )?;
                Ok(())
            }
            _ => Err(format!(
                "assignment to {:?} is not supported at {}",
                targets[0], targets[0].location
            )),
        }
    } else {
        match &value.node {
            ExprKind::List { elts, .. } | ExprKind::Tuple { elts, .. } => {
                if elts.len() == targets.len() {
                    for (tar, val) in targets.iter().zip(elts) {
                        handle_assignment_pattern(
                            std::slice::from_ref(tar),
                            val,
                            resolver,
                            internal_resolver,
                            def_list,
                            unifier,
                            primitives,
                        )?;
                    }
                    Ok(())
                } else {
                    Err(format!(
                        "number of elements to unpack does not match (expect {}, found {}) at {}",
                        targets.len(),
                        elts.len(),
                        value.location
                    ))
                }
            }
            _ => Err(format!(
                "unpack of this expression is not supported at {}",
                value.location
            )),
        }
    }
}

fn main() {
    let cli = CommandLineArgs::parse();
    let CommandLineArgs {
        file_name,
        threads,
        opt_level,
        emit_llvm,
        triple,
        mcpu,
        target_features,
    } = cli;

    Target::initialize_all(&InitializationConfig::default());

    let host_target_machine = CodeGenTargetMachineOptions::from_host();
    let triple = triple.unwrap_or(host_target_machine.triple.clone());
    let mcpu = mcpu
        .map(|arg| if arg == "native" { host_target_machine.cpu.clone() } else { arg })
        .unwrap_or_default();
    let target_features = target_features.unwrap_or_default();
    let threads = if is_multithreaded() {
        if threads == 0 {
            std::thread::available_parallelism()
                .map(|threads| threads.get() as u32)
                .unwrap_or(1u32)
        } else {
            threads
        }
    } else {
        if threads != 1 {
            println!("Warning: Number of threads specified in command-line but multithreading is disabled in LLVM at build time! Defaulting to single-threaded compilation");
        }
        1
    };
    let opt_level = match opt_level {
        0 => OptimizationLevel::None,
        1 => OptimizationLevel::Less,
        2 => OptimizationLevel::Default,
        // The default behavior for -O<n> where n>3 defaults to O3 for both Clang and GCC
        _ => OptimizationLevel::Aggressive,
    };

    let program = match fs::read_to_string(file_name.clone()) {
        Ok(program) => program,
        Err(err) => {
            println!("Cannot open input file: {err}");
            return;
        }
    };

    let primitive: PrimitiveStore = TopLevelComposer::make_primitives().0;
    let (mut composer, builtins_def, builtins_ty) =
        TopLevelComposer::new(vec![], ComposerConfig::default());

    let internal_resolver: Arc<ResolverInternal> = ResolverInternal {
        id_to_type: builtins_ty.into(),
        id_to_def: builtins_def.into(),
        class_names: Mutex::default(),
        module_globals: Mutex::default(),
        str_store: Mutex::default(),
    }.into();
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;

    let parser_result = parser::parse_program(&program, file_name.into()).unwrap();

    for stmt in parser_result {
        match &stmt.node {
            StmtKind::Assign { targets, value, .. } => {
                let def_list = composer.extract_def_list();
                let unifier = &mut composer.unifier;
                let primitives = &composer.primitives_ty;
                if let Err(err) = handle_assignment_pattern(
                    targets,
                    value,
                    resolver.as_ref(),
                    internal_resolver.as_ref(),
                    &def_list,
                    unifier,
                    primitives,
                ) {
                    eprintln!("{err}");
                    return;
                }
            },
            // allow (and ignore) "from __future__ import annotations"
            StmtKind::ImportFrom { module, names, .. }
                if module == &Some("__future__".into()) && names.len() == 1 && names[0].name == "annotations".into() => (),
            _ => {
                let (name, def_id, ty) =
                    composer.register_top_level(stmt, Some(resolver.clone()), "__main__", true).unwrap();
                internal_resolver.add_id_def(name, def_id);
                if let Some(ty) = ty {
                    internal_resolver.add_id_type(name, ty);
                }
            }
        }
    }

    let signature = FunSignature { args: vec![], ret: primitive.int32, vars: HashMap::new() };
    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();
    let signature = store.from_signature(&mut composer.unifier, &primitive, &signature, &mut cache);
    let signature = store.add_cty(signature);

    composer.start_analysis(true).unwrap();

    let top_level = Arc::new(composer.make_top_level_context());

    let instance = {
        let defs = top_level.definitions.read();
        let mut instance = defs[resolver
            .get_identifier_def("run".into())
            .unwrap_or_else(|_| panic!("cannot find run() entry point"))
            .0]
            .write();
        if let TopLevelDef::Function { instance_to_stmt, instance_to_symbol, .. } = &mut *instance {
            instance_to_symbol.insert(String::new(), "run".to_string());
            instance_to_stmt[""].clone()
        } else {
            unreachable!()
        }
    };

    let llvm_options = CodeGenLLVMOptions {
        opt_level,
        target: CodeGenTargetMachineOptions {
            triple,
            cpu: mcpu,
            features: target_features,
            reloc_mode: RelocMode::PIC,
            ..host_target_machine
        },
    };

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "run".to_string(),
        body: instance.body,
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
    let threads = (0..threads)
        .map(|i| Box::new(DefaultCodeGenerator::new(format!("module{i}"), 64)))
        .collect();
    let (registry, handles) = WorkerRegistry::create_workers(threads, top_level, &llvm_options, &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);

    let buffers = membuffers.lock();
    let context = inkwell::context::Context::create();
    let main = context
        .create_module_from_ir(MemoryBuffer::create_from_memory_range(&buffers[0], "main"))
        .unwrap();
    if emit_llvm {
        main.write_bitcode_to_path(Path::new("main.bc"));
    }

    for (idx, buffer) in buffers.iter().skip(1).enumerate() {
        let other = context
            .create_module_from_ir(MemoryBuffer::create_from_memory_range(buffer, "main"))
            .unwrap();

        if emit_llvm {
            other.write_bitcode_to_path(Path::new(&format!("module{idx}.bc")));
        }

        main.link_in_module(other).unwrap();
    }

    let irrt = load_irrt(&context);
    if emit_llvm {
        irrt.write_bitcode_to_path(Path::new("irrt.bc"));
    }
    main.link_in_module(irrt).unwrap();

    let mut function_iter = main.get_first_function();
    while let Some(func) = function_iter {
        if func.count_basic_blocks() > 0 && func.get_name().to_str().unwrap() != "run" {
            func.set_linkage(inkwell::module::Linkage::Private);
        }
        function_iter = func.get_next_function();
    }

    let target_machine = llvm_options.target
        .create_target_machine(llvm_options.opt_level)
        .expect("couldn't create target machine");

    let pass_options = PassBuilderOptions::create();
    pass_options.set_merge_functions(true);
    let passes = format!("default<O{}>", opt_level as u32);
    let result = main.run_passes(passes.as_str(), &target_machine, pass_options);
    if let Err(err) = result {
        panic!("Failed to run optimization for module `main`: {}", err.to_string());
    }

    target_machine
        .write_to_file(&main, FileType::Object, Path::new("module.o"))
        .expect("couldn't write module to file");
}
