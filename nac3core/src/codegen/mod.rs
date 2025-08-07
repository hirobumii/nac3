use std::{
    collections::{HashMap, HashSet},
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use crossbeam::channel::{Receiver, Sender, unbounded};
use inkwell::{
    AddressSpace, IntPredicate, OptimizationLevel,
    basic_block::BasicBlock,
    builder::Builder,
    context::{Context, ContextRef},
    debug_info::{
        AsDIScope, DICompileUnit, DIFlagsConstants, DIScope, DISubprogram, DebugInfoBuilder,
    },
    module::Module,
    passes::PassBuilderOptions,
    targets::{CodeModel, RelocMode, Target, TargetMachine, TargetTriple},
    types::{BasicTypeEnum, FloatType, IntType, PointerType},
    values::{BasicValueEnum, FunctionValue, IntValue, PhiValue, PointerValue},
};
use itertools::Itertools;
use parking_lot::{Condvar, Mutex};

use nac3parser::ast::{Location, Stmt, StrRef};

use crate::{
    codegen::{llvm_fns::FunctionStore, stmt::get_personality},
    symbol_resolver::{StaticValue, SymbolResolver},
    toplevel::{
        TopLevelContext, TopLevelDef,
        helper::{PrimDef, extract_ndims},
        numpy::unpack_ndarray_var_tys,
    },
    typecheck::{
        type_inferencer::{CodeLocation, PrimitiveStore},
        typedef::{CallId, FuncArg, Type, TypeEnum, Unifier},
    },
};
use concrete_type::{ConcreteType, ConcreteTypeEnum, ConcreteTypeStore};
pub use generator::{CodeGenerator, DefaultCodeGenerator};
pub use llvm_fns::FunctionDecl;
use types::{
    ExceptionType, ListType, OptionType, ProxyType, RangeType, StringType, TupleType,
    ndarray::NDArrayType,
};

pub mod builtin_fns;
pub mod concrete_type;
pub mod expr;
pub mod extern_fns;
mod generator;
pub mod irrt;
mod llvm_fns;
pub mod llvm_intrinsics;
pub mod numpy;
pub mod stmt;
pub mod types;
pub mod values;

#[cfg(test)]
mod test;

mod macros {
    /// Codegen-variant of [`std::unreachable`] which accepts an instance of [`CodeGenContext`] as
    /// its first argument to provide Python source information to indicate the codegen location
    /// causing the assertion.
    macro_rules! codegen_unreachable {
        ($ctx:expr $(,)?) => {
            std::unreachable!("unreachable code while processing {}", &$ctx.current_loc)
        };
        ($ctx:expr, $($arg:tt)*) => {
            std::unreachable!("unreachable code while processing {}: {}", &$ctx.current_loc, std::format!("{}", std::format_args!($($arg)+)))
        };
    }

    pub(crate) use codegen_unreachable;
}

#[derive(Default)]
pub struct StaticValueStore {
    pub lookup: HashMap<Vec<(usize, u64)>, usize>,
    pub store: Vec<HashMap<usize, Arc<dyn StaticValue + Send + Sync>>>,
}

pub type VarValue<'ctx> = (PointerValue<'ctx>, Option<Arc<dyn StaticValue + Send + Sync>>, i64);

/// Additional options for LLVM during codegen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeGenOptions {
    /// The optimization level (0/1/2/3/s/z) to apply on the generated LLVM IR.
    pub opt_level: String,

    /// Whether we should insert debugging statements in the generated code.
    pub debug: bool,

    /// Options related to the target machine.
    pub target: TargetMachineOptions,
}

/// Additional options for code generation for the target machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetMachineOptions {
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
    /// Optimization level for backend/target-specific code generation.
    ///
    /// Note that this does not determine the set of optimization passes that run on LLVM IR.
    ///
    /// # Target-specific notes
    ///
    /// On ARM, GOT entries are created when this level is [`OptimizationLevel::None`], which
    /// our linker (`nac3ld`) does not support. You must at least use [`OptimizationLevel::Less`].
    pub target_opt_level: OptimizationLevel,
}

impl TargetMachineOptions {
    /// Creates an instance of [`CodeGenTargetMachineOptions`] using the triple of the host machine.
    /// Other options are set to defaults.
    #[must_use]
    pub fn from_host_triple(level: OptimizationLevel) -> TargetMachineOptions {
        TargetMachineOptions {
            triple: TargetMachine::get_default_triple().as_str().to_string_lossy().into_owned(),
            cpu: String::default(),
            features: String::default(),
            reloc_mode: RelocMode::Default,
            code_model: CodeModel::Default,
            target_opt_level: level,
        }
    }

    /// Creates an instance of [`CodeGenTargetMachineOptions`] using the properties of the host
    /// machine. Other options are set to defaults.
    #[must_use]
    pub fn from_host(level: OptimizationLevel) -> TargetMachineOptions {
        TargetMachineOptions {
            cpu: TargetMachine::get_host_cpu_name().to_string(),
            features: TargetMachine::get_host_cpu_features().to_string(),
            ..TargetMachineOptions::from_host_triple(level)
        }
    }

    /// Creates a [`TargetMachine`] using the target options specified by this struct.
    ///
    /// See [`Target::create_target_machine`].
    #[must_use]
    pub fn create_target_machine(&self) -> TargetMachine {
        let triple = TargetTriple::create(self.triple.as_str());
        let target = Target::from_triple(&triple).unwrap_or_else(|e| {
            panic!("could not create target from target triple {}: {e}", self.triple)
        });

        target
            .create_target_machine(
                &triple,
                self.cpu.as_str(),
                self.features.as_str(),
                self.target_opt_level,
                self.reloc_mode,
                self.code_model,
            )
            .expect("could not create target machine")
    }
}

pub struct CodeGenContext<'ctx, 'a> {
    /// The [`CoreContext`] instance which includes the module and target-specific information.
    pub inner: ModuleContext<'ctx>,

    /// The [`Builder`] instance for creating LLVM IR statements.
    pub builder: Builder<'ctx>,
    /// The [`DebugInfoBuilder`], [compilation unit information][DICompileUnit], and
    /// [scope information][DIScope] of this context.
    pub debug_info: (DebugInfoBuilder<'ctx>, DICompileUnit<'ctx>, DIScope<'ctx>),

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

    /// The current source location.
    pub current_loc: Location,
}

impl<'ctx> std::ops::Deref for CodeGenContext<'ctx, '_> {
    type Target = ModuleContext<'ctx>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for CodeGenContext<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl CodeGenContext<'_, '_> {
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
    pub codegen_options: CodeGenOptions,
}

impl WorkerRegistry {
    /// Creates workers for this registry.
    #[must_use]
    pub fn create_workers<G: CodeGenerator + Send + 'static>(
        generators: Vec<Box<G>>,
        top_level_ctx: Arc<TopLevelContext>,
        codegen_options: &CodeGenOptions,
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
            codegen_options: codegen_options.clone(),
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
        context_ref!(ctx);
        let options = &self.codegen_options.target;
        let mut context = ModuleContext::new(ctx, generator.get_name(), options);
        let mut builder = context.ctx.create_builder();

        let mut errors = HashSet::new();
        while let Some(task) = self.receiver.recv().unwrap() {
            let (context_, builder_, result) = gen_func(context, builder, generator, self, task);
            builder = builder_;
            context = match result {
                Ok(_) => context_,
                Err(e) => {
                    errors.insert(e);
                    ModuleContext::new(
                        context_.ctx,
                        &format!("{}_recover", generator.get_name()),
                        &self.codegen_options.target,
                    )
                }
            };
            *self.task_count.lock() -= 1;
            self.wait_condvar.notify_all();
        }
        assert!(
            errors.is_empty(),
            "Codegen error: {}",
            errors.into_iter().sorted().join("\n----------\n")
        );

        let result = context.module.verify();
        if let Err(err) = result {
            println!("{}", context.module.print_to_string().to_str().unwrap());
            panic!("{}", err.to_string())
        }

        let pass_options = PassBuilderOptions::create();
        let passes = format!("default<O{}>", self.codegen_options.opt_level);
        let result = context.module.run_passes(passes.as_str(), &context.target, pass_options);
        if let Err(err) = result {
            panic!(
                "Failed to run optimization for module `{}`: {}",
                context.module.get_name().to_str().unwrap(),
                err.to_string()
            );
        }

        f.run(&context.module);
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
    pub export_symbol: bool,
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
fn get_llvm_type<'ctx>(
    ctx: &ModuleContext<'ctx>,
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
                            let element_type = get_llvm_type(
                                ctx,
                                unifier,
                                top_level,
                                type_cache,
                                *params.iter().next().unwrap().1,
                            );

                            OptionType::new(ctx, &element_type).as_abi_type().into()
                        }

                        TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                            let element_type = get_llvm_type(
                                ctx,
                                unifier,
                                top_level,
                                type_cache,
                                *params.iter().next().unwrap().1,
                            );

                            ListType::new(ctx, &element_type).as_abi_type().into()
                        }

                        TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                            let (dtype, ndims) = unpack_ndarray_var_tys(unifier, ty);
                            let ndims = extract_ndims(unifier, ndims);
                            let element_type = get_llvm_type(
                                ctx,  unifier, top_level, type_cache, dtype,
                            );

                            NDArrayType::new(ctx, element_type, ndims).as_abi_type().into()
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
                let ty = if let Some(t) = ctx.module.get_struct_type(&name) {
                    t.ptr_type(AddressSpace::default()).into()
                } else {
                    let struct_type = ctx.ctx.opaque_struct_type(&name);
                    type_cache.insert(
                        unifier.get_representative(ty),
                        struct_type.ptr_type(AddressSpace::default()).into(),
                    );
                    let fields = fields_list
                        .iter()
                        .map(|f| {
                            get_llvm_type(
                                ctx,
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
                        get_llvm_type(ctx,  unifier, top_level, type_cache, *ty)
                    })
                    .collect_vec();
                TupleType::new(ctx, &fields).as_abi_type().into()
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
fn get_llvm_abi_type<'ctx>(
    ctx: &ModuleContext<'ctx>,
    unifier: &mut Unifier,
    top_level: &TopLevelContext,
    type_cache: &mut HashMap<Type, BasicTypeEnum<'ctx>>,
    primitives: &PrimitiveStore,
    ty: Type,
) -> BasicTypeEnum<'ctx> {
    // If the type is used in the definition of a function, return `i1` instead of `i8` for ABI
    // consistency.
    if unifier.unioned(ty, primitives.bool) {
        ctx.i1.into()
    } else {
        get_llvm_type(ctx, unifier, top_level, type_cache, ty)
    }
}

/// Returns the [`BasicTypeEnum`] representing a `va_list` struct for variadic arguments.
#[allow(dead_code)]
fn get_llvm_valist_type<'ctx>(ctx: ContextRef<'ctx>, triple: &TargetTriple) -> BasicTypeEnum<'ctx> {
    let triple = TargetMachine::normalize_triple(triple);
    let triple = triple.as_str().to_str().unwrap();
    let arch = triple.split('-').next().unwrap();

    let llvm_pi8 = ctx.i8_type().ptr_type(AddressSpace::default());

    // Referenced from parseArch() in llvm/lib/Support/Triple.cpp
    match arch {
        "i386" | "i486" | "i586" | "i686" | "riscv32" => {
            ctx.i8_type().ptr_type(AddressSpace::default()).into()
        }
        "amd64" | "x86_64" | "x86_64h" => {
            let llvm_i32 = ctx.i32_type();

            let va_list_tag = ctx.opaque_struct_type("struct.__va_list_tag");
            va_list_tag.set_body(
                &[llvm_i32.into(), llvm_i32.into(), llvm_pi8.into(), llvm_pi8.into()],
                false,
            );
            va_list_tag.into()
        }
        "armv7" => {
            let va_list = ctx.opaque_struct_type("struct.__va_list");
            va_list.set_body(&[llvm_pi8.into()], false);
            va_list.into()
        }
        triple => {
            todo!("Unsupported platform for varargs: {triple}")
        }
    }
}

/// A recursive function to fold a [`BasicTypeEnum`] using a provided function `f`.
/// `Try` is not stable yet, so `ControlFlow` is used to allow for short-circuiting
/// during the folding process.
///
/// See [`basic_type_all`] for an example of how to use this function.
pub fn try_fold_basic_type<B, F>(init: B, value: &BasicTypeEnum, f: &F) -> ControlFlow<B, B>
where
    F: Fn(B, &BasicTypeEnum) -> ControlFlow<B, B>,
{
    use BasicTypeEnum::*;
    // Recusively fold the type using the provided function `f`.
    let folded = f(init, value);
    let ControlFlow::Continue(new_init) = folded else {
        return folded;
    };
    match value {
        ArrayType(ty) => try_fold_basic_type(new_init, &ty.get_element_type(), f),
        FloatType(_) | IntType(_) => ControlFlow::Continue(new_init),
        PointerType(ty) => {
            if let Ok(ty) = ty.get_element_type().try_into() {
                try_fold_basic_type(new_init, &ty, f)
            } else {
                ControlFlow::Continue(new_init)
            }
        }
        StructType(ty) => {
            // fold all fields of the struct
            ty.get_field_types()
                .iter()
                .try_fold(new_init, |acc, field| try_fold_basic_type(acc, field, f))
        }
        ScalableVectorType(ty) => try_fold_basic_type(new_init, &ty.get_element_type(), f),
        VectorType(ty) => try_fold_basic_type(new_init, &ty.get_element_type(), f),
    }
}

/// Function to check that all subtypes of a [`BasicTypeEnum`] hold for a given prpedicate `f`,
/// with support for short-circuiting when an instance of an element not satisfying the predicate
/// is found.
pub fn basic_type_all<F>(value: &BasicTypeEnum, f: &F) -> bool
where
    F: Fn(&BasicTypeEnum) -> bool,
{
    try_fold_basic_type((), value, &|(), v| {
        if f(v) { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
    })
    .is_continue()
}

/// Implementation for generating LLVM IR for a function.
#[allow(
    clippy::too_many_arguments,
    reason = "inherent complexity before CodeGenContext is available"
)]
pub fn gen_func_impl<
    'ctx,
    G: CodeGenerator,
    F: FnOnce(&mut G, &mut CodeGenContext) -> Result<(), String>,
>(
    mut ctx: ModuleContext<'ctx>,
    builder: Builder<'ctx>,
    generator: &mut G,
    registry: &WorkerRegistry,
    task: CodeGenTask,
    codegen_function: F,
) -> (ModuleContext<'ctx>, Builder<'ctx>, Result<FunctionValue<'ctx>, String>) {
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
        (primitives.int32, ctx.i32.into()),
        (primitives.int64, ctx.i64.into()),
        (primitives.uint32, ctx.i32.into()),
        (primitives.uint64, ctx.i64.into()),
        (primitives.float, ctx.f64.into()),
        (primitives.bool, ctx.i8.into()),
        (primitives.str, { StringType::new(&ctx).as_abi_type().into() }),
        (primitives.range, RangeType::new(&ctx).as_abi_type().into()),
        (primitives.exception, { ExceptionType::new(&ctx).as_abi_type().into() }),
    ]
    .iter()
    .copied()
    .collect();
    // NOTE: special handling of option cannot use this type cache since it contains type var,
    // handled inside get_llvm_type instead

    let ConcreteTypeEnum::TFunc { args, ret, .. } = task.store.get(task.signature) else {
        unreachable!()
    };

    // TODO(ivan): ConcreteType is conceptually more tightly linked to native LLVM types.
    // We should not be converting back into typechecking/unifier types and then turning that into
    // native LLVM types.

    let ret = task.store.to_unifier_type(&mut unifier, &primitives, *ret, &mut cache);
    let ret_type = if unifier.unioned(ret, primitives.none) {
        None
    } else {
        Some(get_llvm_abi_type(
            &ctx,
            &mut unifier,
            top_level_ctx.as_ref(),
            &mut type_cache,
            &primitives,
            ret,
        ))
    };

    let params = args
        .iter()
        .map(|arg| FuncArg {
            name: arg.name,
            ty: task.store.to_unifier_type(&mut unifier, &primitives, arg.ty, &mut cache),
            default_value: arg.default_value.clone(),
            is_vararg: arg.is_vararg,
        })
        .collect_vec();
    let params_type = params
        .iter()
        .map(|arg| {
            get_llvm_abi_type(
                &ctx,
                &mut unifier,
                top_level_ctx.as_ref(),
                &mut type_cache,
                &primitives,
                arg.ty,
            )
            .into()
        })
        .collect_vec();

    let symbol = &task.symbol_name;
    // This module is independent from the module spawning this codegen task,
    // so we must redefine the function from scratch.
    let (_, fn_val) = ctx.declare_internal(symbol, ret_type, &params_type, task.export_symbol);

    if let Some(personality) = get_personality(&top_level_ctx, &ctx) {
        fn_val.set_personality_function(personality);
    }

    let init_bb = ctx.ctx.append_basic_block(fn_val, "init");
    builder.position_at_end(init_bb);
    let body_bb = ctx.ctx.append_basic_block(fn_val, "body");

    // Store non-vararg argument values into local variables
    let mut var_assignment = HashMap::new();

    for (n, arg) in params.iter().enumerate().filter(|(_, arg)| !arg.is_vararg) {
        let param = fn_val.get_nth_param(n as u32).unwrap();
        let local_type =
            get_llvm_type(&ctx, &mut unifier, top_level_ctx.as_ref(), &mut type_cache, arg.ty);
        let alloca =
            builder.build_alloca(local_type, &format!("{}.addr", &arg.name.to_string())).unwrap();

        // Remap boolean parameters into i8
        let param = if local_type.is_int_type() && param.is_int_value() {
            let expected_ty = local_type.into_int_type();
            let param_val = param.into_int_value();

            if expected_ty.get_bit_width() == 8 && param_val.get_type().get_bit_width() == 1 {
                bool_to_int_type(&builder, param_val, ctx.i8)
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

    // TODO: Save vararg parameters as list

    let return_buffer = ret_type.map(|v| builder.build_alloca(v, "$ret").unwrap());

    let static_values = {
        let store = registry.static_value_store.lock();
        store.store[task.id].clone()
    };
    for (k, v) in static_values {
        let (_, static_val, _) = var_assignment.get_mut(&params[k].name).unwrap();
        *static_val = Some(v);
    }

    builder.build_unconditional_branch(body_bb).unwrap();
    builder.position_at_end(body_bb);

    let is_optimized = registry.codegen_options.opt_level != "0";

    let (dibuilder, compile_unit) = ctx.module.create_debug_info_builder(
        /* allow_unresolved */ true,
        /* language */ inkwell::debug_info::DWARFSourceLanguage::Python,
        /* filename */
        &task
            .body
            .first()
            .map_or_else(|| "<nac3_internal>".to_string(), |f| f.location.file.0.to_string()),
        /* directory */ "",
        /* producer */ "NAC3",
        /* is_optimized */ is_optimized,
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
        /* is_optimized */ is_optimized,
    );
    fn_val.set_subprogram(func_scope);

    let debug_info = (dibuilder, compile_unit, func_scope.as_debug_info_scope());
    let loc = debug_info.0.create_debug_location(
        ctx.ctx,
        row as u32,
        col as u32,
        func_scope.as_debug_info_scope(),
        None,
    );
    builder.set_current_debug_location(loc);

    let mut code_gen_context = CodeGenContext {
        inner: ctx,
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
        unifier,
        static_value_store,
        current_loc: Location::default(),
        debug_info,
    };

    let result = codegen_function(generator, &mut code_gen_context).map(|()| fn_val);

    // after static analysis, only void functions can have no return at the end.
    if !code_gen_context.is_terminated() {
        code_gen_context.builder.build_return(None).unwrap();
    }

    code_gen_context.builder.unset_current_debug_location();
    code_gen_context.debug_info.0.finalize();

    let CodeGenContext { inner, builder, .. } = code_gen_context;
    (inner, builder, result)
}

/// Generates LLVM IR for a function.
///
/// * `context` - The [`CoreContext`] we are inserting into.
/// * `builder` - The [`Builder`] used for generating LLVM IR.
/// * `generator` - The [`CodeGenerator`] for generating various program constructs.
/// * `registry` - The [`WorkerRegistry`] responsible for monitoring this function generation task.
/// * `task` - The [`CodeGenTask`] associated with this function generation task.
pub fn gen_func<'ctx, G: CodeGenerator>(
    context: ModuleContext<'ctx>,
    builder: Builder<'ctx>,
    generator: &mut G,
    registry: &WorkerRegistry,
    task: CodeGenTask,
) -> (ModuleContext<'ctx>, Builder<'ctx>, Result<FunctionValue<'ctx>, String>) {
    let body = task.body.clone();
    gen_func_impl(context, builder, generator, registry, task, |generator, ctx| {
        generator.gen_block(ctx, body.iter())
    })
}

/// Converts the value of a boolean-like value `value` into an arbitrary [`IntType`].
///
/// This has the same semantics as `(ty)(value != 0)` in C.
///
/// The returned value is guaranteed to either be `0` or `1`, except for `ty == i1` where only the
/// least-significant bit would be guaranteed to be `0` or `1`.
fn bool_to_int_type<'ctx>(
    builder: &Builder<'ctx>,
    value: IntValue<'ctx>,
    ty: IntType<'ctx>,
) -> IntValue<'ctx> {
    // i1 -> i1    : %value                                     ; no-op
    // i1 -> i<N>  : zext i1 %value to i<N>                     ; guaranteed to be 0 or 1 - see docs
    // i<M> -> i<N>: zext i1 (icmp eq i<M> %value, 0) to i<N>   ; same as i<M> -> i1 -> i<N>
    match (value.get_type().get_bit_width(), ty.get_bit_width()) {
        (1, 1) => value,
        (1, _) => builder.build_int_z_extend(value, ty, "frombool").unwrap(),
        _ => bool_to_int_type(
            builder,
            builder
                .build_int_compare(IntPredicate::NE, value, value.get_type().const_zero(), "tobool")
                .unwrap(),
            ty,
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
    let sign =
        ctx.builder.build_int_compare(IntPredicate::SGT, step, ctx.i32.const_zero(), "").unwrap();
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

/// Returns the alignment of the type.
///
/// This is necessary as `get_alignment` is not implemented as part of [`BasicType`].
pub fn get_type_alignment<'ctx>(ty: impl Into<BasicTypeEnum<'ctx>>) -> IntValue<'ctx> {
    match ty.into() {
        BasicTypeEnum::ArrayType(ty) => ty.get_alignment(),
        BasicTypeEnum::FloatType(ty) => ty.get_alignment(),
        BasicTypeEnum::IntType(ty) => ty.get_alignment(),
        BasicTypeEnum::PointerType(ty) => ty.get_alignment(),
        BasicTypeEnum::StructType(ty) => ty.get_alignment(),
        BasicTypeEnum::VectorType(ty) => ty.get_alignment(),
        BasicTypeEnum::ScalableVectorType(ty) => ty.get_alignment(),
    }
}

/// Inserts an `alloca` instruction with allocation `size` given in bytes and the alignment of the
/// given type.
///
/// The returned [`PointerValue`] will have a type of `i8*`, a size of at least `size`, and will be
/// aligned with the alignment of `align_ty`.
pub fn type_aligned_alloca<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    align_ty: impl Into<BasicTypeEnum<'ctx>>,
    size: IntValue<'ctx>,
    name: Option<&str>,
) -> PointerValue<'ctx> {
    /// Round `val` up to its modulo `power_of_two`.
    fn round_up<'ctx>(
        ctx: &CodeGenContext<'ctx, '_>,
        val: IntValue<'ctx>,
        power_of_two: IntValue<'ctx>,
    ) -> IntValue<'ctx> {
        debug_assert_eq!(
            val.get_type().get_bit_width(),
            power_of_two.get_type().get_bit_width(),
            "`val` ({}) and `power_of_two` ({}) must be the same type",
            val.get_type(),
            power_of_two.get_type(),
        );

        let llvm_val_t = val.get_type();

        let max_rem =
            ctx.builder.build_int_sub(power_of_two, llvm_val_t.const_int(1, false), "").unwrap();
        ctx.builder
            .build_and(
                ctx.builder.build_int_add(val, max_rem, "").unwrap(),
                ctx.builder.build_not(max_rem, "").unwrap(),
                "",
            )
            .unwrap()
    }

    let llvm_pi8 = ctx.ptr;
    let llvm_usize = ctx.size_t;
    let align_ty = align_ty.into();

    let size = ctx.builder.build_int_truncate_or_bit_cast(size, llvm_usize, "").unwrap();

    debug_assert_eq!(
        size.get_type().get_bit_width(),
        llvm_usize.get_bit_width(),
        "Expected size_t ({}) for parameter `size` of `aligned_alloca`, got {}",
        llvm_usize,
        size.get_type(),
    );

    let alignment = get_type_alignment(align_ty);
    let alignment = ctx.builder.build_int_truncate_or_bit_cast(alignment, llvm_usize, "").unwrap();

    if ctx.registry.codegen_options.debug {
        let alignment_bitcount = llvm_intrinsics::call_int_ctpop(ctx, alignment, None);

        ctx.make_assert(
            generator,
            ctx.builder
                .build_int_compare(
                    IntPredicate::EQ,
                    alignment_bitcount,
                    alignment_bitcount.get_type().const_int(1, false),
                    "",
                )
                .unwrap(),
            "0:AssertionError",
            "Expected power-of-two alignment for aligned_alloca, got {0}",
            [Some(alignment), None, None],
            ctx.current_loc,
        );
    }

    let buffer_size = round_up(ctx, size, alignment);
    let aligned_slices = ctx.builder.build_int_unsigned_div(buffer_size, alignment, "").unwrap();

    // Just to be absolutely sure, alloca in [i8 x alignment] slices
    let buffer = ctx.builder.build_array_alloca(align_ty, aligned_slices, "").unwrap();

    ctx.builder
        .build_bit_cast(buffer, llvm_pi8, name.unwrap_or_default())
        .map(BasicValueEnum::into_pointer_value)
        .unwrap()
}

/// Contains all global LLVM state that is attached to an LLVM [`Module`] and independent
/// from Python.
pub struct ModuleContext<'ctx> {
    /// The associated LLVM context.
    pub ctx: ContextRef<'ctx>,
    /// The LLVM module that we are generating into.
    pub module: Module<'ctx>,
    /// The `TargetMachine` that we are compiling for.
    pub target: TargetMachine,

    /// Wrapped function declarations.
    ///
    /// Belongs here because it needs to capture all function declarations to the module;
    /// wrapping a new `FunctionStore` around a non-empty `Module` is a logical error.
    fn_store: FunctionStore<'ctx>,

    /// The `usize`/`size_t` integer type. Pointer-sized on all supported platforms.
    pub size_t: IntType<'ctx>,
    /// The 1-bit integer, or LLVM-boolean, type.
    pub i1: IntType<'ctx>,
    /// The 8-bit integer, or byte, type.
    pub i8: IntType<'ctx>,
    /// The 32-bit integer type.
    pub i32: IntType<'ctx>,
    /// The 64-bit integer type.
    pub i64: IntType<'ctx>,
    /// The 64-bit floating-point type.
    pub f64: FloatType<'ctx>,
    /// The "canonical" pointer type. Currently implemented as i8*.
    pub ptr: PointerType<'ctx>,
}

impl<'ctx> ModuleContext<'ctx> {
    /// Constructs a [`CoreContext`].
    #[must_use]
    pub fn new(ctx: ContextRef<'ctx>, module_name: &str, options: &TargetMachineOptions) -> Self {
        let module = ctx.create_module(module_name);
        let target = options.create_target_machine();
        let size_t = ctx.ptr_sized_int_type(&target.get_target_data(), None);
        let i1 = ctx.bool_type();
        let i8 = ctx.i8_type();
        let i32 = ctx.i32_type();
        let i64 = ctx.i64_type();
        let f64 = ctx.f64_type();
        let ptr = i8.ptr_type(AddressSpace::default());
        let fn_store = FunctionStore::new(options);

        module.set_data_layout(&target.get_target_data().get_data_layout());
        module.set_triple(&target.get_triple());
        module.add_basic_value_flag(
            "Debug Info Version",
            inkwell::module::FlagBehavior::Warning,
            i32.const_int(3, false),
        );
        module.add_basic_value_flag(
            "Dwarf Version",
            inkwell::module::FlagBehavior::Warning,
            i32.const_int(4, false),
        );

        Self { ctx, module, target, fn_store, size_t, i1, i8, i32, i64, f64, ptr }
    }
}

/// Constructs a [`ContextRef`].
///
/// This is a macro because it needs to declare and borrow from a local [`Context`].
///
/// # Example
///
/// ```
/// # use nac3core::codegen::context_ref;
/// // Constructs a ContextRef named `ctx`.
/// context_ref!(ctx);
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __codegen_context_ref {
    ($name:pat) => {
        let ctx = $crate::inkwell::context::Context::create();
        let $name = $crate::codegen::__make_context_ref(&ctx);
    };
}

// Enforces that the ContextRef borrows from the Context.
#[doc(hidden)]
#[must_use]
pub fn __make_context_ref(ctx: &Context) -> ContextRef<'_> {
    unsafe { ContextRef::new(inkwell::context::AsContextRef::as_ctx_ref(&ctx)) }
}

#[doc(inline)]
pub use __codegen_context_ref as context_ref;
