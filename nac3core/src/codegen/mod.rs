use crate::{
    codegen::classes::{ListType, NDArrayType, ProxyType, RangeType},
    symbol_resolver::{StaticValue, SymbolResolver},
    toplevel::{helper::PrimDef, numpy::unpack_ndarray_var_tys, TopLevelContext, TopLevelDef},
    typecheck::{
        type_inferencer::{CodeLocation, PrimitiveStore},
        typedef::{CallId, FuncArg, Type, TypeEnum, Unifier},
    },
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    basic_block::BasicBlock,
    builder::Builder,
    context::Context,
    debug_info::{
        AsDIScope, DICompileUnit, DIFlagsConstants, DIScope, DISubprogram, DebugInfoBuilder,
    },
    module::Module,
    passes::PassBuilderOptions,
    targets::{CodeModel, RelocMode, Target, TargetMachine, TargetTriple},
    types::{AnyType, BasicType, BasicTypeEnum},
    values::{BasicValueEnum, FunctionValue, IntValue, PhiValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use itertools::Itertools;
use nac3parser::ast::{Location, Stmt, StrRef};
use parking_lot::{Condvar, Mutex};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

pub mod builtin_fns;
pub mod classes;
pub mod concrete_type;
pub mod expr;
pub mod extern_fns;
mod generator;
pub mod irrt;
pub mod llvm_intrinsics;
pub mod numpy;
pub mod stmt;

#[cfg(test)]
mod test;

use concrete_type::{ConcreteType, ConcreteTypeEnum, ConcreteTypeStore};
pub use generator::{CodeGenerator, DefaultCodeGenerator};

#[derive(Default)]
pub struct StaticValueStore {
    pub lookup: HashMap<Vec<(usize, u64)>, usize>,
    pub store: Vec<HashMap<usize, Arc<dyn StaticValue + Send + Sync>>>,
}

pub type VarValue<'ctx> = (PointerValue<'ctx>, Option<Arc<dyn StaticValue + Send + Sync>>, i64);

/// Additional options for LLVM during codegen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeGenLLVMOptions {
    /// The optimization level to apply on the generated LLVM IR.
    pub opt_level: OptimizationLevel,

    /// Options related to the target machine.
    pub target: CodeGenTargetMachineOptions,
}

impl CodeGenLLVMOptions {
    /// Creates a [`TargetMachine`] using the target options specified by this struct.
    ///
    /// See [`Target::create_target_machine`].
    #[must_use]
    pub fn create_target_machine(&self) -> Option<TargetMachine> {
        self.target.create_target_machine(self.opt_level)
    }
}

/// Additional options for code generation for the target machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeGenTargetMachineOptions {
    /// The target machine triple.
    pub triple: String,
    /// The target machine CPU.
    pub cpu: String,
    /// Additional target machine features.
    pub features: String,
    /// Relocation mode for code generation.
    pub reloc_mode: RelocMode,
    /// Code model for code generation.
    pub code_model: CodeModel,
}

impl CodeGenTargetMachineOptions {
    /// Creates an instance of [`CodeGenTargetMachineOptions`] using the triple of the host machine.
    /// Other options are set to defaults.
    #[must_use]
    pub fn from_host_triple() -> CodeGenTargetMachineOptions {
        CodeGenTargetMachineOptions {
            triple: TargetMachine::get_default_triple().as_str().to_string_lossy().into_owned(),
            cpu: String::default(),
            features: String::default(),
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
        }
    }

    /// Creates an instance of [`CodeGenTargetMachineOptions`] using the properties of the host
    /// machine. Other options are set to defaults.
    #[must_use]
    pub fn from_host() -> CodeGenTargetMachineOptions {
        CodeGenTargetMachineOptions {
            cpu: TargetMachine::get_host_cpu_name().to_string(),
            features: TargetMachine::get_host_cpu_features().to_string(),
            ..CodeGenTargetMachineOptions::from_host_triple()
        }
    }

    /// Creates a [`TargetMachine`] using the target options specified by this struct.
    ///
    /// See [`Target::create_target_machine`].
    #[must_use]
    pub fn create_target_machine(&self, level: OptimizationLevel) -> Option<TargetMachine> {
        let triple = TargetTriple::create(self.triple.as_str());
        let target = Target::from_triple(&triple).unwrap_or_else(|_| {
            panic!("could not create target from target triple {}", self.triple)
        });

        target.create_target_machine(
            &triple,
            self.cpu.as_str(),
            self.features.as_str(),
            level,
            self.reloc_mode,
            self.code_model,
        )
    }
}

pub struct CodeGenContext<'ctx, 'a> {
    /// The LLVM context associated with [this context][CodeGenContext].
    pub ctx: &'ctx Context,

    /// The [Builder] instance for creating LLVM IR statements.
    pub builder: Builder<'ctx>,
    /// The [`DebugInfoBuilder`], [compilation unit information][DICompileUnit], and
    /// [scope information][DIScope] of this context.
    pub debug_info: (DebugInfoBuilder<'ctx>, DICompileUnit<'ctx>, DIScope<'ctx>),

    /// The module for which [this context][CodeGenContext] is generating into.
    pub module: Module<'ctx>,

    /// The [`TopLevelContext`] associated with [this context][CodeGenContext].
    pub top_level: &'a TopLevelContext,
    pub unifier: Unifier,
    pub resolver: Arc<dyn SymbolResolver + Send + Sync>,
    pub static_value_store: Arc<Mutex<StaticValueStore>>,

    /// A [`HashMap`] containing the mapping between the names of variables currently in-scope and
    /// its value information.
    pub var_assignment: HashMap<StrRef, VarValue<'ctx>>,

    pub type_cache: HashMap<Type, BasicTypeEnum<'ctx>>,
    pub primitives: PrimitiveStore,
    pub calls: Arc<HashMap<CodeLocation, CallId>>,
    pub registry: &'a WorkerRegistry,

    /// Cache for constant strings.
    pub const_strings: HashMap<String, BasicValueEnum<'ctx>>,

    /// [`BasicBlock`] containing all `alloca` statements for the current function.
    pub init_bb: BasicBlock<'ctx>,
    pub exception_val: Option<PointerValue<'ctx>>,

    /// The header and exit basic blocks of a loop in this context. See
    /// <https://llvm.org/docs/LoopTerminology.html> for explanation of these terminology.
    pub loop_target: Option<(BasicBlock<'ctx>, BasicBlock<'ctx>)>,

    /// The target [`BasicBlock`] to jump to when performing stack unwind.
    pub unwind_target: Option<BasicBlock<'ctx>>,

    /// The target [`BasicBlock`] to jump to before returning from the function.
    ///
    /// If this field is [None] when generating a return from a function, `ret` with no argument can
    /// be emitted.
    pub return_target: Option<BasicBlock<'ctx>>,

    /// The [`PointerValue`] containing the return value of the function.
    pub return_buffer: Option<PointerValue<'ctx>>,

    // outer catch clauses
    pub outer_catch_clauses:
        Option<(Vec<Option<BasicValueEnum<'ctx>>>, BasicBlock<'ctx>, PhiValue<'ctx>)>,

    /// Whether `sret` is needed for the first parameter of the function.
    ///
    /// See [`need_sret`].
    pub need_sret: bool,

    /// The current source location.
    pub current_loc: Location,
}

impl<'ctx, 'a> CodeGenContext<'ctx, 'a> {
    /// Whether the [current basic block][Builder::get_insert_block] referenced by `builder`
    /// contains a [terminator statement][BasicBlock::get_terminator].
    pub fn is_terminated(&self) -> bool {
        self.builder.get_insert_block().and_then(BasicBlock::get_terminator).is_some()
    }
}

type Fp = Box<dyn Fn(&Module) + Send + Sync>;

pub struct WithCall {
    fp: Fp,
}

impl WithCall {
    #[must_use]
    pub fn new(fp: Fp) -> WithCall {
        WithCall { fp }
    }

    pub fn run(&self, m: &Module) {
        (self.fp)(m);
    }
}

pub struct WorkerRegistry {
    sender: Arc<Sender<Option<CodeGenTask>>>,
    receiver: Arc<Receiver<Option<CodeGenTask>>>,

    /// Whether any thread in this registry has panicked.
    panicked: AtomicBool,

    /// The total number of tasks queued or completed in the registry.
    task_count: Mutex<usize>,

    /// The number of threads available for this registry.
    thread_count: usize,
    wait_condvar: Condvar,
    top_level_ctx: Arc<TopLevelContext>,
    static_value_store: Arc<Mutex<StaticValueStore>>,

    /// LLVM-related options for code generation.
    pub llvm_options: CodeGenLLVMOptions,
}

impl WorkerRegistry {
    /// Creates workers for this registry.
    #[must_use]
    pub fn create_workers<G: CodeGenerator + Send + 'static>(
        generators: Vec<Box<G>>,
        top_level_ctx: Arc<TopLevelContext>,
        llvm_options: &CodeGenLLVMOptions,
        f: &Arc<WithCall>,
    ) -> (Arc<WorkerRegistry>, Vec<thread::JoinHandle<()>>) {
        let (sender, receiver) = unbounded();
        let task_count = Mutex::new(0);
        let wait_condvar = Condvar::new();

        // init: 0 to be empty
        let mut static_value_store = StaticValueStore::default();
        static_value_store.lookup.insert(Vec::default(), 0);
        static_value_store.store.push(HashMap::default());

        let registry = Arc::new(WorkerRegistry {
            sender: Arc::new(sender),
            receiver: Arc::new(receiver),
            thread_count: generators.len(),
            panicked: AtomicBool::new(false),
            static_value_store: Arc::new(Mutex::new(static_value_store)),
            task_count,
            wait_condvar,
            top_level_ctx,
            llvm_options: llvm_options.clone(),
        });

        let mut handles = Vec::new();
        for mut generator in generators {
            let registry = registry.clone();
            let registry2 = registry.clone();
            let f = f.clone();

            let worker_thread_name =
                format!("codegen-worker-{worker_id}", worker_id = generator.get_name());
            let handle = thread::Builder::new()
                .name(worker_thread_name)
                .spawn(move || {
                    registry.worker_thread(generator.as_mut(), &f);
                })
                .unwrap();
            let handle = thread::spawn(move || {
                if let Err(e) = handle.join() {
                    if let Some(e) = e.downcast_ref::<&'static str>() {
                        eprintln!("Got an error: {e}");
                    } else {
                        eprintln!("Got an unknown error: {e:?}");
                    }
                    registry2.panicked.store(true, Ordering::SeqCst);
                    registry2.wait_condvar.notify_all();
                }
            });
            handles.push(handle);
        }
        (registry, handles)
    }

    pub fn wait_tasks_complete(&self, handles: Vec<thread::JoinHandle<()>>) {
        {
            let mut count = self.task_count.lock();
            while *count != 0 {
                if self.panicked.load(Ordering::SeqCst) {
                    break;
                }
                self.wait_condvar.wait(&mut count);
            }
        }
        for _ in 0..self.thread_count {
            self.sender.send(None).unwrap();
        }
        {
            let mut count = self.task_count.lock();
            while *count != self.thread_count {
                if self.panicked.load(Ordering::SeqCst) {
                    break;
                }
                self.wait_condvar.wait(&mut count);
            }
        }
        for handle in handles {
            handle.join().unwrap();
        }
        assert!(!self.panicked.load(Ordering::SeqCst), "tasks panicked");
    }

    /// Adds a task to this [`WorkerRegistry`].
    pub fn add_task(&self, task: CodeGenTask) {
        *self.task_count.lock() += 1;
        self.sender.send(Some(task)).unwrap();
    }

    /// Function executed by worker thread for generating IR for each function.
    fn worker_thread<G: CodeGenerator>(&self, generator: &mut G, f: &Arc<WithCall>) {
        let context = Context::create();
        let mut builder = context.create_builder();
        let mut module = context.create_module(generator.get_name());

        let target_machine = self.llvm_options.create_target_machine().unwrap();
        module.set_data_layout(&target_machine.get_target_data().get_data_layout());
        module.set_triple(&target_machine.get_triple());

        module.add_basic_value_flag(
            "Debug Info Version",
            inkwell::module::FlagBehavior::Warning,
            context.i32_type().const_int(3, false),
        );
        module.add_basic_value_flag(
            "Dwarf Version",
            inkwell::module::FlagBehavior::Warning,
            context.i32_type().const_int(4, false),
        );

        let mut errors = HashSet::new();
        while let Some(task) = self.receiver.recv().unwrap() {
            match gen_func(&context, generator, self, builder, module, task) {
                Ok(result) => {
                    builder = result.0;
                    module = result.1;
                }
                Err((old_builder, e)) => {
                    builder = old_builder;
                    errors.insert(e);
                    // create a new empty module just to continue codegen and collect errors
                    module = context.create_module(&format!("{}_recover", generator.get_name()));

                    let target_machine = self.llvm_options.create_target_machine().unwrap();
                    module.set_data_layout(&target_machine.get_target_data().get_data_layout());
                    module.set_triple(&target_machine.get_triple());
                }
            }
            *self.task_count.lock() -= 1;
            self.wait_condvar.notify_all();
        }
        assert!(
            errors.is_empty(),
            "Codegen error: {}",
            errors.into_iter().sorted().join("\n----------\n")
        );

        let result = module.verify();
        if let Err(err) = result {
            println!("{}", module.print_to_string().to_str().unwrap());
            panic!("{}", err.to_string())
        }

        let pass_options = PassBuilderOptions::create();
        let target_machine = self
            .llvm_options
            .target
            .create_target_machine(self.llvm_options.opt_level)
            .unwrap_or_else(|| {
                panic!(
                    "could not create target machine from properties {:?}",
                    self.llvm_options.target
                )
            });
        let passes = format!("default<O{}>", self.llvm_options.opt_level as u32);
        let result = module.run_passes(passes.as_str(), &target_machine, pass_options);
        if let Err(err) = result {
            panic!(
                "Failed to run optimization for module `{}`: {}",
                module.get_name().to_str().unwrap(),
                err.to_string()
            );
        }

        f.run(&module);
        let mut lock = self.task_count.lock();
        *lock += 1;
        self.wait_condvar.notify_all();
    }
}

pub struct CodeGenTask {
    pub subst: Vec<(Type, ConcreteType)>,
    pub store: ConcreteTypeStore,
    pub symbol_name: String,
    pub signature: ConcreteType,
    pub body: Arc<Vec<Stmt<Option<Type>>>>,
    pub calls: Arc<HashMap<CodeLocation, CallId>>,
    pub unifier_index: usize,
    pub resolver: Arc<dyn SymbolResolver + Send + Sync>,
    pub id: usize,
}

/// Retrieves the [LLVM type][BasicTypeEnum] corresponding to the [Type].
///
/// This function is used to obtain the in-memory representation of `ty`, e.g. a `bool` variable
/// would be represented by an `i8`.
#[allow(clippy::too_many_arguments)]
fn get_llvm_type<'ctx, G: CodeGenerator + ?Sized>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    generator: &G,
    unifier: &mut Unifier,
    top_level: &TopLevelContext,
    type_cache: &mut HashMap<Type, BasicTypeEnum<'ctx>>,
    ty: Type,
) -> BasicTypeEnum<'ctx> {
    use TypeEnum::*;
    // we assume the type cache should already contain primitive types,
    // and they should be passed by value instead of passing as pointer.
    type_cache.get(&unifier.get_representative(ty)).copied().unwrap_or_else(|| {
        let ty_enum = unifier.get_ty(ty);
        let result = match &*ty_enum {
            TObj { obj_id, fields, .. } => {
                // check to avoid treating non-class primitives as classes
                if PrimDef::contains_id(*obj_id) {
                    return match &*unifier.get_ty_immutable(ty) {
                        TObj { obj_id, params, .. } if *obj_id == PrimDef::Option.id() => {
                            get_llvm_type(
                                ctx,
                                module,
                                generator,
                                unifier,
                                top_level,
                                type_cache,
                                *params.iter().next().unwrap().1,
                            )
                            .ptr_type(AddressSpace::default())
                            .into()
                        }

                        TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                            let element_type = get_llvm_type(
                                ctx,
                                module,
                                generator,
                                unifier,
                                top_level,
                                type_cache,
                                *params.iter().next().unwrap().1,
                            );

                            ListType::new(generator, ctx, element_type).as_base_type().into()
                        }

                        TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                            let (dtype, _) = unpack_ndarray_var_tys(unifier, ty);
                            let element_type = get_llvm_type(
                                ctx, module, generator, unifier, top_level, type_cache, dtype,
                            );

                            NDArrayType::new(generator, ctx, element_type).as_base_type().into()
                        }

                        _ => unreachable!(
                            "LLVM type for primitive {} is missing",
                            unifier.stringify(ty)
                        ),
                    };
                }
                // a struct with fields in the order of declaration
                let top_level_defs = top_level.definitions.read();
                let definition = top_level_defs.get(obj_id.0).unwrap();
                let TopLevelDef::Class { fields: fields_list, .. } = &*definition.read() else {
                    unreachable!()
                };

                let name = unifier.stringify(ty);
                let ty = if let Some(t) = module.get_struct_type(&name) {
                    t.ptr_type(AddressSpace::default()).into()
                } else {
                    let struct_type = ctx.opaque_struct_type(&name);
                    type_cache.insert(
                        unifier.get_representative(ty),
                        struct_type.ptr_type(AddressSpace::default()).into(),
                    );
                    let fields = fields_list
                        .iter()
                        .map(|f| {
                            get_llvm_type(
                                ctx,
                                module,
                                generator,
                                unifier,
                                top_level,
                                type_cache,
                                fields[&f.0].0,
                            )
                        })
                        .collect_vec();
                    struct_type.set_body(&fields, false);
                    struct_type.ptr_type(AddressSpace::default()).into()
                };
                return ty;
            }
            TTuple { ty, is_vararg_ctx } => {
                // a struct with fields in the order present in the tuple
                assert!(!is_vararg_ctx, "Tuples in vararg context must be instantiated with the correct number of arguments before calling get_llvm_type");

                let fields = ty
                    .iter()
                    .map(|ty| {
                        get_llvm_type(ctx, module, generator, unifier, top_level, type_cache, *ty)
                    })
                    .collect_vec();
                ctx.struct_type(&fields, false).into()
            }
            TVirtual { .. } => unimplemented!(),
            _ => unreachable!("{}", ty_enum.get_type_name()),
        };
        type_cache.insert(unifier.get_representative(ty), result);
        result
    })
}

/// Retrieves the [LLVM type][`BasicTypeEnum`] corresponding to the [`Type`].
///
/// This function is used mainly to obtain the ABI representation of `ty`, e.g. a `bool` is
/// would be represented by an `i1`.
///
/// The difference between the in-memory representation (as returned by [`get_llvm_type`]) and the
/// ABI representation is that the in-memory representation must be at least byte-sized and must
/// be byte-aligned for the variable to be addressable in memory, whereas there is no such
/// restriction for ABI representations.
#[allow(clippy::too_many_arguments)]
fn get_llvm_abi_type<'ctx, G: CodeGenerator + ?Sized>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    generator: &G,
    unifier: &mut Unifier,
    top_level: &TopLevelContext,
    type_cache: &mut HashMap<Type, BasicTypeEnum<'ctx>>,
    primitives: &PrimitiveStore,
    ty: Type,
) -> BasicTypeEnum<'ctx> {
    // If the type is used in the definition of a function, return `i1` instead of `i8` for ABI
    // consistency.
    return if unifier.unioned(ty, primitives.bool) {
        ctx.bool_type().into()
    } else {
        get_llvm_type(ctx, module, generator, unifier, top_level, type_cache, ty)
    };
}

/// Whether `sret` is needed for a return value with type `ty`.
///
/// When returning a large data structure (e.g. structures that do not fit in 1-2 native words of
/// the target processor) by value, a synthetic parameter with a pointer type will be passed in the
/// slot of the first parameter to act as the location of which the return value is passed into.
///
/// See <https://releases.llvm.org/14.0.0/docs/LangRef.html#parameter-attributes> for more
/// information.
fn need_sret(ty: BasicTypeEnum) -> bool {
    fn need_sret_impl(ty: BasicTypeEnum, maybe_large: bool) -> bool {
        match ty {
            BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_) => false,
            BasicTypeEnum::FloatType(_) if maybe_large => false,
            BasicTypeEnum::StructType(ty) if maybe_large && ty.count_fields() <= 2 => {
                ty.get_field_types().iter().any(|ty| need_sret_impl(*ty, false))
            }
            _ => true,
        }
    }
    need_sret_impl(ty, true)
}

/// Implementation for generating LLVM IR for a function.
pub fn gen_func_impl<
    'ctx,
    G: CodeGenerator,
    F: FnOnce(&mut G, &mut CodeGenContext) -> Result<(), String>,
>(
    context: &'ctx Context,
    generator: &mut G,
    registry: &WorkerRegistry,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    task: CodeGenTask,
    codegen_function: F,
) -> Result<(Builder<'ctx>, Module<'ctx>, FunctionValue<'ctx>), (Builder<'ctx>, String)> {
    let top_level_ctx = registry.top_level_ctx.clone();
    let static_value_store = registry.static_value_store.clone();
    let (mut unifier, primitives) = {
        let (unifier, primitives) = &top_level_ctx.unifiers.read()[task.unifier_index];
        (Unifier::from_shared_unifier(unifier), *primitives)
    };
    unifier.put_primitive_store(&primitives);
    unifier.top_level = Some(top_level_ctx.clone());

    let mut cache = HashMap::new();
    for (a, b) in &task.subst {
        // this should be unification between variables and concrete types
        // and should not cause any problem...
        let b = task.store.to_unifier_type(&mut unifier, &primitives, *b, &mut cache);
        unifier
            .unify(*a, b)
            .or_else(|err| {
                if matches!(&*unifier.get_ty(*a), TypeEnum::TRigidVar { .. }) {
                    unifier.replace_rigid_var(*a, b);
                    Ok(())
                } else {
                    Err(err)
                }
            })
            .unwrap();
    }

    // rebuild primitive store with unique representatives
    let primitives = PrimitiveStore {
        int32: unifier.get_representative(primitives.int32),
        int64: unifier.get_representative(primitives.int64),
        uint32: unifier.get_representative(primitives.uint32),
        uint64: unifier.get_representative(primitives.uint64),
        float: unifier.get_representative(primitives.float),
        bool: unifier.get_representative(primitives.bool),
        none: unifier.get_representative(primitives.none),
        range: unifier.get_representative(primitives.range),
        str: unifier.get_representative(primitives.str),
        exception: unifier.get_representative(primitives.exception),
        option: unifier.get_representative(primitives.option),
        ..primitives
    };

    let mut type_cache: HashMap<_, _> = [
        (primitives.int32, context.i32_type().into()),
        (primitives.int64, context.i64_type().into()),
        (primitives.uint32, context.i32_type().into()),
        (primitives.uint64, context.i64_type().into()),
        (primitives.float, context.f64_type().into()),
        (primitives.bool, context.i8_type().into()),
        (primitives.str, {
            let name = "str";
            match module.get_struct_type(name) {
                None => {
                    let str_type = context.opaque_struct_type("str");
                    let fields = [
                        context.i8_type().ptr_type(AddressSpace::default()).into(),
                        generator.get_size_type(context).into(),
                    ];
                    str_type.set_body(&fields, false);
                    str_type.into()
                }
                Some(t) => t.as_basic_type_enum(),
            }
        }),
        (primitives.range, RangeType::new(context).as_base_type().into()),
        (primitives.exception, {
            let name = "Exception";
            if let Some(t) = module.get_struct_type(name) {
                t.ptr_type(AddressSpace::default()).as_basic_type_enum()
            } else {
                let exception = context.opaque_struct_type("Exception");
                let int32 = context.i32_type().into();
                let int64 = context.i64_type().into();
                let str_ty = module.get_struct_type("str").unwrap().as_basic_type_enum();
                let fields = [int32, str_ty, int32, int32, str_ty, str_ty, int64, int64, int64];
                exception.set_body(&fields, false);
                exception.ptr_type(AddressSpace::default()).as_basic_type_enum()
            }
        }),
    ]
    .iter()
    .copied()
    .collect();
    // NOTE: special handling of option cannot use this type cache since it contains type var,
    // handled inside get_llvm_type instead

    let ConcreteTypeEnum::TFunc { args, ret, .. } = task.store.get(task.signature) else {
        unreachable!()
    };

    let (args, ret) = (
        args.iter()
            .map(|arg| FuncArg {
                name: arg.name,
                ty: task.store.to_unifier_type(&mut unifier, &primitives, arg.ty, &mut cache),
                default_value: arg.default_value.clone(),
                is_vararg: arg.is_vararg,
            })
            .collect_vec(),
        task.store.to_unifier_type(&mut unifier, &primitives, *ret, &mut cache),
    );
    let ret_type = if unifier.unioned(ret, primitives.none) {
        None
    } else {
        Some(get_llvm_abi_type(
            context,
            &module,
            generator,
            &mut unifier,
            top_level_ctx.as_ref(),
            &mut type_cache,
            &primitives,
            ret,
        ))
    };

    let has_sret = ret_type.map_or(false, |ty| need_sret(ty));
    let mut params = args
        .iter()
        .map(|arg| {
            get_llvm_abi_type(
                context,
                &module,
                generator,
                &mut unifier,
                top_level_ctx.as_ref(),
                &mut type_cache,
                &primitives,
                arg.ty,
            )
            .into()
        })
        .collect_vec();

    if has_sret {
        params.insert(0, ret_type.unwrap().ptr_type(AddressSpace::default()).into());
    }

    let fn_type = match ret_type {
        Some(ret_type) if !has_sret => ret_type.fn_type(&params, false),
        _ => context.void_type().fn_type(&params, false),
    };

    let symbol = &task.symbol_name;
    let fn_val =
        module.get_function(symbol).unwrap_or_else(|| module.add_function(symbol, fn_type, None));

    if let Some(personality) = &top_level_ctx.personality_symbol {
        let personality = module.get_function(personality).unwrap_or_else(|| {
            let ty = context.i32_type().fn_type(&[], true);
            module.add_function(personality, ty, None)
        });
        fn_val.set_personality_function(personality);
    }
    if has_sret {
        fn_val.add_attribute(
            AttributeLoc::Param(0),
            context.create_type_attribute(
                Attribute::get_named_enum_kind_id("sret"),
                ret_type.unwrap().as_any_type_enum(),
            ),
        );
    }

    let init_bb = context.append_basic_block(fn_val, "init");
    builder.position_at_end(init_bb);
    let body_bb = context.append_basic_block(fn_val, "body");

    let mut var_assignment = HashMap::new();
    let offset = u32::from(has_sret);
    for (n, arg) in args.iter().enumerate() {
        let param = fn_val.get_nth_param((n as u32) + offset).unwrap();
        let local_type = get_llvm_type(
            context,
            &module,
            generator,
            &mut unifier,
            top_level_ctx.as_ref(),
            &mut type_cache,
            arg.ty,
        );
        let alloca =
            builder.build_alloca(local_type, &format!("{}.addr", &arg.name.to_string())).unwrap();

        // Remap boolean parameters into i8
        let param = if local_type.is_int_type() && param.is_int_value() {
            let expected_ty = local_type.into_int_type();
            let param_val = param.into_int_value();

            if expected_ty.get_bit_width() == 8 && param_val.get_type().get_bit_width() == 1 {
                bool_to_i8(&builder, context, param_val)
            } else {
                param_val
            }
            .into()
        } else {
            param
        };

        builder.build_store(alloca, param).unwrap();
        var_assignment.insert(arg.name, (alloca, None, 0));
    }

    let return_buffer = if has_sret {
        Some(fn_val.get_nth_param(0).unwrap().into_pointer_value())
    } else {
        fn_type.get_return_type().map(|v| builder.build_alloca(v, "$ret").unwrap())
    };

    let static_values = {
        let store = registry.static_value_store.lock();
        store.store[task.id].clone()
    };
    for (k, v) in static_values {
        let (_, static_val, _) = var_assignment.get_mut(&args[k].name).unwrap();
        *static_val = Some(v);
    }

    builder.build_unconditional_branch(body_bb).unwrap();
    builder.position_at_end(body_bb);

    let (dibuilder, compile_unit) = module.create_debug_info_builder(
        /* allow_unresolved */ true,
        /* language */ inkwell::debug_info::DWARFSourceLanguage::Python,
        /* filename */
        &task
            .body
            .first()
            .map_or_else(|| "<nac3_internal>".to_string(), |f| f.location.file.0.to_string()),
        /* directory */ "",
        /* producer */ "NAC3",
        /* is_optimized */ registry.llvm_options.opt_level != OptimizationLevel::None,
        /* compiler command line flags */ "",
        /* runtime_ver */ 0,
        /* split_name */ "",
        /* kind */ inkwell::debug_info::DWARFEmissionKind::Full,
        /* dwo_id */ 0,
        /* split_debug_inling */ true,
        /* debug_info_for_profiling */ false,
        /* sysroot */ "",
        /* sdk */ "",
    );
    let subroutine_type = dibuilder.create_subroutine_type(
        compile_unit.get_file(),
        Some(
            dibuilder
                .create_basic_type("_", 0_u64, 0x00, inkwell::debug_info::DIFlags::PUBLIC)
                .unwrap()
                .as_type(),
        ),
        &[],
        inkwell::debug_info::DIFlags::PUBLIC,
    );
    let (row, col) =
        task.body.first().map_or_else(|| (0, 0), |b| (b.location.row, b.location.column));
    let func_scope: DISubprogram<'_> = dibuilder.create_function(
        /* scope */ compile_unit.as_debug_info_scope(),
        /* func name */ symbol,
        /* linkage_name */ None,
        /* file */ compile_unit.get_file(),
        /* line_no */ row as u32,
        /* DIType */ subroutine_type,
        /* is_local_to_unit */ false,
        /* is_definition */ true,
        /* scope_line */ row as u32,
        /* flags */ inkwell::debug_info::DIFlags::PUBLIC,
        /* is_optimized */ registry.llvm_options.opt_level != OptimizationLevel::None,
    );
    fn_val.set_subprogram(func_scope);

    let mut code_gen_context = CodeGenContext {
        ctx: context,
        resolver: task.resolver,
        top_level: top_level_ctx.as_ref(),
        calls: task.calls,
        loop_target: None,
        return_target: None,
        return_buffer,
        unwind_target: None,
        outer_catch_clauses: None,
        const_strings: HashMap::default(),
        registry,
        var_assignment,
        type_cache,
        primitives,
        init_bb,
        exception_val: Option::default(),
        builder,
        module,
        unifier,
        static_value_store,
        need_sret: has_sret,
        current_loc: Location::default(),
        debug_info: (dibuilder, compile_unit, func_scope.as_debug_info_scope()),
    };

    let loc = code_gen_context.debug_info.0.create_debug_location(
        context,
        row as u32,
        col as u32,
        func_scope.as_debug_info_scope(),
        None,
    );
    code_gen_context.builder.set_current_debug_location(loc);

    let result = codegen_function(generator, &mut code_gen_context);

    // after static analysis, only void functions can have no return at the end.
    if !code_gen_context.is_terminated() {
        code_gen_context.builder.build_return(None).unwrap();
    }

    code_gen_context.builder.unset_current_debug_location();
    code_gen_context.debug_info.0.finalize();

    let CodeGenContext { builder, module, .. } = code_gen_context;
    if let Err(e) = result {
        return Err((builder, e));
    }

    Ok((builder, module, fn_val))
}

/// Generates LLVM IR for a function.
///
/// * `context` - The [LLVM Context][`Context`] used in generating the function body.
/// * `generator` - The [`CodeGenerator`] for generating various program constructs.
/// * `registry` - The [`WorkerRegistry`] responsible for monitoring this function generation task.
/// * `builder` - The [`Builder`] used for generating LLVM IR.
/// * `module` - The [`Module`] of which the generated LLVM function will be inserted into.
/// * `task` - The [`CodeGenTask`] associated with this function generation task.
///
pub fn gen_func<'ctx, G: CodeGenerator>(
    context: &'ctx Context,
    generator: &mut G,
    registry: &WorkerRegistry,
    builder: Builder<'ctx>,
    module: Module<'ctx>,
    task: CodeGenTask,
) -> Result<(Builder<'ctx>, Module<'ctx>, FunctionValue<'ctx>), (Builder<'ctx>, String)> {
    let body = task.body.clone();
    gen_func_impl(context, generator, registry, builder, module, task, |generator, ctx| {
        generator.gen_block(ctx, body.iter())
    })
}

/// Converts the value of a boolean-like value `bool_value` into an `i1`.
fn bool_to_i1<'ctx>(builder: &Builder<'ctx>, bool_value: IntValue<'ctx>) -> IntValue<'ctx> {
    if bool_value.get_type().get_bit_width() == 1 {
        bool_value
    } else {
        builder
            .build_int_compare(
                IntPredicate::NE,
                bool_value,
                bool_value.get_type().const_zero(),
                "tobool",
            )
            .unwrap()
    }
}

/// Converts the value of a boolean-like value `bool_value` into an `i8`.
fn bool_to_i8<'ctx>(
    builder: &Builder<'ctx>,
    ctx: &'ctx Context,
    bool_value: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let value_bits = bool_value.get_type().get_bit_width();
    match value_bits {
        8 => bool_value,
        1 => builder.build_int_z_extend(bool_value, ctx.i8_type(), "frombool").unwrap(),
        _ => bool_to_i8(
            builder,
            ctx,
            builder
                .build_int_compare(
                    IntPredicate::NE,
                    bool_value,
                    bool_value.get_type().const_zero(),
                    "",
                )
                .unwrap(),
        ),
    }
}

/// Generates a sequence of IR which checks whether `value` does not exceed the upper bound of the
/// range as defined by `stop` and `step`.
///
/// Note that the generated IR will **not** check whether value is part of the range or whether
/// value exceeds the lower bound of the range (as evident by the missing `start` argument).
///
/// The generated IR is equivalent to the following Rust code:
///
/// ```rust,ignore
/// let sign = step > 0;
/// let (lo, hi) = if sign { (value, stop) } else { (stop, value) };
/// let cmp = lo < hi;
/// ```
///
/// Returns an `i1` [`IntValue`] representing the result of whether the `value` is in the range.
fn gen_in_range_check<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    value: IntValue<'ctx>,
    stop: IntValue<'ctx>,
    step: IntValue<'ctx>,
) -> IntValue<'ctx> {
    let sign = ctx
        .builder
        .build_int_compare(IntPredicate::SGT, step, ctx.ctx.i32_type().const_zero(), "")
        .unwrap();
    let lo = ctx
        .builder
        .build_select(sign, value, stop, "")
        .map(BasicValueEnum::into_int_value)
        .unwrap();
    let hi = ctx
        .builder
        .build_select(sign, stop, value, "")
        .map(BasicValueEnum::into_int_value)
        .unwrap();

    ctx.builder.build_int_compare(IntPredicate::SLT, lo, hi, "cmp").unwrap()
}
