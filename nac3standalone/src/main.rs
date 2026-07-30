use std::{collections::HashMap, fs, num::NonZeroUsize, path::Path, sync::Arc};

use anyhow::anyhow;
use clap::Parser;
use nac3core::{
    codegen::{
        CodeGenOptions, CodeGenTask, DefaultCodeGenerator, TargetMachineOptions, WithCall,
        WorkerRegistry, concrete_type::ConcreteTypeStore, context_ref, irrt::load_irrt,
    },
    inkwell::{
        OptimizationLevel,
        memory_buffer::MemoryBuffer,
        module::{Linkage, Module},
        passes::PassBuilderOptions,
        support::is_multithreaded,
        targets::{FileType, InitializationConfig, RelocMode, Target},
    },
    nac3parser::{
        ast::{Constant, Expr, ExprKind, StmtKind, StrRef},
        parser,
    },
    symbol_resolver::SymbolResolver,
    toplevel::{
        TopLevelDef,
        composer::{BuiltinRegistry, DefaultBuiltinRegistry, TopLevelComposer},
        helper::parse_parameter_default_value,
        type_annotation::{
            get_type_from_type_annotation_kinds, parse_ast_to_type_annotation_kinds,
        },
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, Type, Unifier, VarMap},
    },
};
use parking_lot::{Mutex, RwLock};

use basic_symbol_resolver::{Resolver, ResolverInternal};

mod basic_symbol_resolver;

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

    /// Whether to emit LLVM bitcode at the end of every module.
    #[arg(long, default_value_t = false)]
    emit_llvm_bc: bool,

    /// Whether to emit LLVM IR text at the end of every module.
    #[arg(long, default_value_t = false)]
    emit_llvm_ir: bool,

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
    builtin_registry: &dyn BuiltinRegistry,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
) -> Result<Type, Vec<anyhow::Error>> {
    let ExprKind::Call { func, args, .. } = &var.node else {
        return Err(vec![anyhow!(
            "expression {var:?} cannot be handled as a generic parameter in global scope"
        )]);
    };

    match &func.node {
        ExprKind::Name { id, .. } if id == &"TypeVar".into() => {
            let ExprKind::Constant { value: Constant::Str(ty_name), .. } = &args[0].node else {
                return Err(vec![anyhow!(
                    "Expected string constant for first parameter of `TypeVar`, got {:?}",
                    &args[0].node
                )]);
            };
            let generic_name: StrRef = ty_name.as_str().into();

            let constraints = args
                .iter()
                .skip(1)
                .map(|x| {
                    let ty = parse_ast_to_type_annotation_kinds(
                        resolver,
                        def_list,
                        builtin_registry,
                        unifier,
                        primitives,
                        x,
                        HashMap::new(),
                    )?;
                    get_type_from_type_annotation_kinds(
                        def_list, unifier, primitives, &ty, &mut None,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let loc = func.location;

            if constraints.len() == 1 {
                return Err(vec![anyhow!("A single constraint is not allowed (at {loc})")]);
            }

            Ok(unifier.get_fresh_var_with_range(&constraints, Some(generic_name), Some(loc)).ty)
        }

        ExprKind::Name { id, .. } if id == &"ConstGeneric".into() => {
            if args.len() != 2 {
                return Err(vec![anyhow!(
                    "Expected 2 arguments for `ConstGeneric`, got {}",
                    args.len()
                )]);
            }

            let ExprKind::Constant { value: Constant::Str(ty_name), .. } = &args[0].node else {
                return Err(vec![anyhow!(
                    "Expected string constant for first parameter of `ConstGeneric`, got {:?}",
                    &args[0].node
                )]);
            };
            let generic_name: StrRef = ty_name.as_str().into();

            let ty = parse_ast_to_type_annotation_kinds(
                resolver,
                def_list,
                builtin_registry,
                unifier,
                primitives,
                &args[1],
                HashMap::new(),
            )?;
            let constraint =
                get_type_from_type_annotation_kinds(def_list, unifier, primitives, &ty, &mut None)?;
            let loc = func.location;

            Ok(unifier.get_fresh_const_generic_var(constraint, Some(generic_name), Some(loc)).ty)
        }

        _ => Err(vec![anyhow!(
            "expression {var:?} cannot be handled as a generic parameter in global scope"
        )]),
    }
}

fn handle_assignment_pattern(
    targets: &[Expr],
    value: &Expr,
    resolver: Arc<dyn SymbolResolver + Send + Sync>,
    internal_resolver: &ResolverInternal,
    composer: &mut TopLevelComposer,
) -> Result<(), String> {
    if targets.len() == 1 {
        let target = &targets[0];

        match &target.node {
            ExprKind::Name { id, .. } => {
                let def_list = composer.extract_def_list();
                let unifier = &mut composer.unifier;
                let primitives = &composer.primitives_ty;
                let builtin_registry = &*composer.builtin_registry;

                handle_typevar_definition(
                    value,
                    &*resolver,
                    &def_list,
                    builtin_registry,
                    unifier,
                    primitives,
                ).map_or_else(|_| parse_parameter_default_value(value, &*resolver, builtin_registry).map_or_else(|_| Err(format!(
                        "fails to evaluate this expression `{:?}` as a constant or generic parameter (at {})",
                        target.node, target.location,
                    )), |val| {
                    internal_resolver.add_module_global(*id, val);
                    Ok(())
                }), |var| {
                    internal_resolver.add_id_type(*id, var);
                    Ok(())
                })
            }
            ExprKind::List { elts, .. } | ExprKind::Tuple { elts, .. } => {
                handle_assignment_pattern(elts, value, resolver, internal_resolver, composer)?;
                Ok(())
            }
            _ => Err(format!("assignment to {target:?} is not supported (at {})", target.location)),
        }
    } else {
        match &value.node {
            ExprKind::List { elts, .. } | ExprKind::Tuple { elts, .. } => {
                if elts.len() == targets.len() {
                    for (tar, val) in targets.iter().zip(elts) {
                        handle_assignment_pattern(
                            std::slice::from_ref(tar),
                            val,
                            resolver.clone(),
                            internal_resolver,
                            composer,
                        )?;
                    }
                    Ok(())
                } else {
                    Err(format!(
                        "number of elements to unpack does not match (expect {}, found {}) (at {})",
                        targets.len(),
                        elts.len(),
                        value.location
                    ))
                }
            }
            _ => Err(format!("unpack of this expression is not supported (at {})", value.location)),
        }
    }
}

fn main() {
    let cli = CommandLineArgs::parse();
    let CommandLineArgs {
        file_name,
        threads,
        opt_level,
        emit_llvm_bc,
        emit_llvm_ir,
        triple,
        mcpu,
        target_features,
    } = cli;

    Target::initialize_all(&InitializationConfig::default());

    let program = match fs::read_to_string(file_name.clone()) {
        Ok(program) => program,
        Err(err) => {
            panic!("Cannot open input file: {err}");
        }
    };

    // The default behavior for -O<n> where n>3 defaults to O3 for both Clang and GCC
    let opt_level = opt_level.min(3);
    let codegen_opt_level = opt_level.to_string();
    let target_opt_level = match opt_level {
        0 => OptimizationLevel::None,
        1 => OptimizationLevel::Less,
        2 => OptimizationLevel::Default,
        3 => OptimizationLevel::Aggressive,
        _ => unreachable!(),
    };

    let host_target_machine = TargetMachineOptions::from_host(target_opt_level);
    let triple = triple.unwrap_or_else(|| host_target_machine.triple.clone());
    let mcpu = mcpu
        .map(|arg| if arg == "native" { host_target_machine.cpu.clone() } else { arg })
        .unwrap_or_default();
    let target_features = target_features.unwrap_or_default();
    let threads = if is_multithreaded() {
        if threads == 0 {
            std::thread::available_parallelism().map_or_else(|_| 1usize, NonZeroUsize::get)
        } else {
            threads as usize
        }
    } else {
        if threads != 1 {
            println!(
                "Warning: Number of threads specified in command-line but multithreading is disabled in LLVM at build time! Defaulting to single-threaded compilation"
            );
        }
        1
    };

    let target = TargetMachineOptions {
        triple,
        cpu: mcpu,
        features: target_features,
        reloc_mode: RelocMode::PIC,
        ..host_target_machine
    };
    // Create a target machine now to get sizeof(size_t), which is required for type inference
    // (specifically, the type of the length of an ndarray).
    let target_machine = target.create_target_machine();
    let size_t_bits = target_machine.get_target_data().get_pointer_byte_size(None) * 8;

    let builtin_registry = Arc::new(DefaultBuiltinRegistry);

    let (mut composer, builtins_def, builtins_ty) =
        TopLevelComposer::new(vec![], vec![], builtin_registry, size_t_bits);

    let internal_resolver: Arc<ResolverInternal> = ResolverInternal {
        id_to_type: builtins_ty.into(),
        id_to_def: builtins_def.into(),
        module_globals: Mutex::default(),
        str_store: Mutex::default(),
    }
    .into();
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;

    let emit_llvm = |module: &Module<'_>, filename: &str| {
        if emit_llvm_bc {
            module.write_bitcode_to_path(Path::new(format!("{filename}.bc").as_str()));
        }
        if emit_llvm_ir {
            module.print_to_file(Path::new(format!("{filename}.ll").as_str())).unwrap();
        }
    };

    context_ref!(context);

    let irrt = load_irrt(context, &target, resolver.as_ref());
    emit_llvm(&irrt, "irrt");

    // Process the Python script
    let parser_result = parser::parse_program(&program, file_name.into()).unwrap();

    for stmt in parser_result {
        match &stmt.node {
            StmtKind::Assign { targets, value, .. } => {
                if let Err(err) = handle_assignment_pattern(
                    targets,
                    value,
                    resolver.clone(),
                    internal_resolver.as_ref(),
                    &mut composer,
                ) {
                    panic!("{err}");
                }
            }

            // allow (and ignore) "from __future__ import annotations"
            StmtKind::ImportFrom { module, names, .. }
                if module == &Some("__future__".into())
                    && names.len() == 1
                    && names[0].name == "annotations".into() => {}
            _ => {
                let (name, def_id, ty) = composer
                    .register_top_level(stmt, Some(resolver.clone()), "__main__", true)
                    .unwrap();
                internal_resolver.add_id_def(name, def_id);
                if let Some(ty) = ty {
                    internal_resolver.add_id_type(name, ty);
                }
            }
        }
    }

    let signature =
        FunSignature { args: vec![], ret: composer.primitives_ty.int32, vars: VarMap::new() };
    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();
    let signature = store.from_signature(
        &mut composer.unifier,
        &composer.primitives_ty,
        &signature,
        &mut cache,
    );
    let signature = store.add_cty(signature);

    if let Err(errors) = composer.start_analysis(true) {
        let error_count = errors.len();
        eprintln!("{error_count} error(s) occurred during top level analysis.");

        for (error_i, error) in errors.iter().enumerate() {
            let error_num = error_i + 1;
            eprintln!("=========== ERROR {error_num}/{error_count} ============");
            eprintln!("{error}");
        }
        eprintln!("==================================");

        panic!("top level analysis failed");
    }

    let top_level = Arc::new(composer.make_top_level_context());

    let (instance, location) = {
        let defs = &top_level.definitions;
        let TopLevelDef::Function { instance_to_stmt, instance_to_symbol, loc, .. } = &mut *defs
            [resolver
                .get_identifier_def("run".into())
                .unwrap_or_else(|_| panic!("cannot find run() entry point"))
                .0]
            .write()
        else {
            unreachable!()
        };

        instance_to_symbol.insert(String::new(), "run".to_string());
        (instance_to_stmt[""].clone(), *loc)
    };

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "run".to_string(),
        export_symbol: true,
        body: instance.body,
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
    let threads =
        (0..threads).map(|i| Box::new(DefaultCodeGenerator::new(format!("module{i}")))).collect();
    let options =
        CodeGenOptions { debug: codegen_opt_level == "0", opt_level: codegen_opt_level, target };
    let (registry, handles) = WorkerRegistry::create_workers(threads, top_level, &options, &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);

    // Link all modules together into `main`
    let buffers = membuffers.lock();
    let main = context
        .create_module_from_ir(MemoryBuffer::create_from_memory_range(&buffers[0], "main"))
        .unwrap();
    emit_llvm(&main, "main");

    for buffer in buffers.iter().skip(1) {
        let other = context
            .create_module_from_ir(MemoryBuffer::create_from_memory_range(buffer, "main"))
            .unwrap();

        main.link_in_module(other).unwrap();
    }
    drop(buffers);
    emit_llvm(&main, "main.merged");

    main.link_in_module(irrt).unwrap();
    emit_llvm(&main, "main.fat");

    // Private all functions except "run"
    let mut function_iter = main.get_first_function();
    while let Some(func) = function_iter {
        if func.count_basic_blocks() > 0 && func.get_name().to_str().unwrap() != "run" {
            func.set_linkage(Linkage::Private);
        }
        function_iter = func.get_next_function();
    }

    emit_llvm(&main, "main.pre-opt");

    // Optimize `main`
    let pass_options = PassBuilderOptions::create();
    pass_options.set_merge_functions(true);
    let passes = format!("globaldce,strip-dead-prototypes,default<O{opt_level}>");
    let result = main.run_passes(passes.as_str(), &target_machine, pass_options);
    if let Err(err) = result {
        panic!("Failed to run optimization for module `main`: {}", err.to_string());
    }
    emit_llvm(&main, "main.post-opt");

    // Write output
    target_machine
        .write_to_file(&main, FileType::Object, Path::new("module.o"))
        .expect("couldn't write module to file");
}
