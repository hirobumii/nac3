use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::anyhow;
use function_name::named;
use indexmap::IndexMap;
use indoc::indoc;
use inkwell::{
    OptimizationLevel,
    targets::{InitializationConfig, Target},
};
use nac3parser::{
    ast::{FileName, StrRef, fold::Fold},
    parser::parse_program,
};
use parking_lot::RwLock;

use crate::{
    codegen::{
        CodeGenContext, CodeGenOptions, CodeGenTask, DefaultCodeGenerator, TargetMachineOptions,
        WithCall, WorkerRegistry, concrete_type::ConcreteTypeStore,
    },
    symbol_resolver::{SymbolResolver, ValueEnum},
    toplevel::{
        DefinitionId, FunInstance, TopLevelContext, TopLevelDef,
        composer::{DefaultBuiltinRegistry, TopLevelComposer},
    },
    typecheck::{
        type_inferencer::{FunctionData, Inferencer, PrimitiveStore},
        typedef::{FunSignature, FuncArg, Type, TypeEnum, Unifier, VarMap},
    },
};

struct Resolver {
    id_to_type: HashMap<StrRef, Type>,
    id_to_def: RwLock<HashMap<StrRef, DefinitionId>>,
}

impl Resolver {
    pub fn add_id_def(&self, id: StrRef, def: DefinitionId) {
        self.id_to_def.write().insert(id, def);
    }
}

impl SymbolResolver for Resolver {
    fn get_default_param_value(
        &self,
        _: &nac3parser::ast::Expr,
    ) -> anyhow::Result<Option<crate::symbol_resolver::SymbolValue>> {
        unimplemented!()
    }

    fn get_symbol_type(
        &self,
        _: &mut Unifier,
        _: &[Arc<RwLock<TopLevelDef>>],
        _: &PrimitiveStore,
        str: StrRef,
    ) -> anyhow::Result<Type> {
        self.id_to_type.get(&str).copied().ok_or_else(|| anyhow!("cannot find symbol `{str}`"))
    }

    fn get_symbol_value<'ctx>(
        &self,
        _: StrRef,
        _: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<Option<ValueEnum<'ctx>>> {
        unimplemented!()
    }

    fn get_identifier_def(&self, id: StrRef) -> Result<DefinitionId, Vec<anyhow::Error>> {
        self.id_to_def
            .read()
            .get(&id)
            .copied()
            .ok_or_else(|| vec![anyhow!("cannot find symbol `{id}`")])
    }

    fn get_string_id(&self, _: &str) -> i32 {
        // Stub
        0
    }

    fn get_exception_id(&self, _tyid: usize) -> usize {
        unimplemented!()
    }
}

fn codegen_options() -> CodeGenOptions {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    // We want things like debug assertions, but we otherwise want to run on optimized code.
    CodeGenOptions {
        opt_level: String::from("2"),
        debug: true,
        target: TargetMachineOptions::from_host_triple(OptimizationLevel::Default),
    }
}

#[test]
#[named]
fn test_primitives() {
    let source = indoc! { "
        c = a + b
        d = a if c == 1 else 0
        return d
        "};
    let statements = parse_program(source, FileName::default()).unwrap();

    let composer =
        TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 64).0;
    let mut unifier = composer.unifier.clone();
    let primitives = composer.primitives_ty;
    let top_level = composer.make_top_level_context();

    let resolver =
        Arc::new(Resolver { id_to_type: HashMap::new(), id_to_def: RwLock::new(HashMap::new()) })
            as Arc<dyn SymbolResolver + Send + Sync>;

    let threads = vec![DefaultCodeGenerator::new("test".into()).into()];
    let signature = FunSignature {
        args: vec![
            FuncArg {
                name: "a".into(),
                ty: primitives.int32,
                default_value: None,
                is_vararg: false,
            },
            FuncArg {
                name: "b".into(),
                ty: primitives.int32,
                default_value: None,
                is_vararg: false,
            },
        ],
        ret: primitives.int32,
        vars: VarMap::new(),
    };

    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();
    let signature = store.from_signature(&mut unifier, &primitives, &signature, &mut cache);
    let signature = store.add_cty(signature);

    let mut function_data = FunctionData {
        resolver: resolver.clone(),
        bound_variables: Vec::new(),
        return_type: Some(primitives.int32),
    };
    let mut virtual_checks = Vec::new();
    let mut calls = HashMap::new();
    let mut identifiers: HashSet<_, _> = ["a".into(), "b".into()].into();
    let mut inferencer = Inferencer {
        builtin_registry: &*top_level.builtin_registry,
        top_level_defs: &top_level.definitions,
        function_data: &mut function_data,
        unifier: &mut unifier,
        variable_mapping: HashMap::default(),
        primitives: &primitives,
        virtual_checks: &mut virtual_checks,
        calls: &mut calls,
        defined_identifiers: identifiers.clone(),
        in_handler: false,
    };
    inferencer.variable_mapping.insert("a".into(), inferencer.primitives.int32);
    inferencer.variable_mapping.insert("b".into(), inferencer.primitives.int32);

    let statements = statements
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    inferencer.check_block(&statements, &mut identifiers).unwrap();
    let top_level = Arc::new(TopLevelContext {
        definitions: top_level.definitions,
        unifiers: (unifier.get_shared_unifier(), primitives),
        personality_symbol: None,
        builtin_registry: top_level.builtin_registry,
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".into(),
        export_symbol: true,
        location: statements.first().unwrap().location,
        body: Arc::new(statements),
        calls: Arc::new(calls),
        resolver,
        store,
        signature,
        id: 0,
    };
    let f = Arc::new(WithCall::new(Box::new(|module| {
        insta::assert_snapshot!(
            function_name!(),
            module.print_to_string().to_str().map(str::trim).unwrap()
        );
    })));

    let (registry, handles) =
        WorkerRegistry::create_workers(threads, top_level, &codegen_options(), &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);
}

#[test]
#[named]
fn test_simple_call() {
    let source_1 = indoc! { "
        a = foo(a)
        return a * 2
        "};
    let statements_1 = parse_program(source_1, FileName::default()).unwrap();

    let source_2 = indoc! { "
        return a + 1
        "};
    let statements_2 = parse_program(source_2, FileName::default()).unwrap();
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);

    let composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry.clone(), 64).0;
    let mut unifier = composer.unifier.clone();
    let primitives = composer.primitives_ty;
    let mut top_level = composer.make_top_level_context();

    let signature = FunSignature {
        args: vec![FuncArg {
            name: "a".into(),
            ty: primitives.int32,
            default_value: None,
            is_vararg: false,
        }],
        ret: primitives.int32,
        vars: VarMap::new(),
    };
    let fun_ty = unifier.add_ty(TypeEnum::TFunc(signature.clone()));
    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();
    let signature = store.from_signature(&mut unifier, &primitives, &signature, &mut cache);
    let signature = store.add_cty(signature);

    let foo_id = top_level.definitions.len();
    top_level.definitions.push(Arc::new(RwLock::new(TopLevelDef::Function {
        name: "foo".to_string(),
        simple_name: "foo".into(),
        signature: fun_ty,
        var_id: vec![],
        attributes: Vec::default(),
        instance_to_stmt: HashMap::new(),
        instance_to_symbol: HashMap::new(),
        resolver: None,
        codegen_callback: None,
        loc: None,
    })));

    let resolver = Resolver { id_to_type: HashMap::new(), id_to_def: RwLock::new(HashMap::new()) };
    resolver.add_id_def("foo".into(), DefinitionId(foo_id));
    let resolver = Arc::new(resolver) as Arc<dyn SymbolResolver + Send + Sync>;

    if let TopLevelDef::Function { resolver: r, .. } = &mut *top_level.definitions[foo_id].write() {
        *r = Some(resolver.clone());
    } else {
        unreachable!()
    }

    let threads = vec![DefaultCodeGenerator::new("test".into()).into()];
    let mut function_data = FunctionData {
        resolver: resolver.clone(),
        bound_variables: Vec::new(),
        return_type: Some(primitives.int32),
    };
    let mut virtual_checks = Vec::new();
    let mut calls = HashMap::new();
    let mut identifiers: HashSet<_, _> = ["a".into(), "foo".into()].into();
    let mut inferencer = Inferencer {
        builtin_registry: &*top_level.builtin_registry,
        top_level_defs: &top_level.definitions,
        function_data: &mut function_data,
        unifier: &mut unifier,
        variable_mapping: HashMap::default(),
        primitives: &primitives,
        virtual_checks: &mut virtual_checks,
        calls: &mut calls,
        defined_identifiers: identifiers.clone(),
        in_handler: false,
    };
    inferencer.variable_mapping.insert("a".into(), inferencer.primitives.int32);
    inferencer.variable_mapping.insert("foo".into(), fun_ty);

    let statements_1 = statements_1
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let calls1 = inferencer.calls.clone();
    inferencer.calls.clear();

    let statements_2 = statements_2
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    if let TopLevelDef::Function { instance_to_stmt, .. } =
        &mut *top_level.definitions[foo_id].write()
    {
        instance_to_stmt.insert(
            String::new(),
            FunInstance {
                body: Arc::new(statements_2),
                calls: Arc::new(inferencer.calls.clone()),
                subst: IndexMap::default(),
            },
        );
    } else {
        unreachable!()
    }

    inferencer.check_block(&statements_1, &mut identifiers).unwrap();
    let top_level = Arc::new(TopLevelContext {
        definitions: top_level.definitions,
        unifiers: (unifier.get_shared_unifier(), primitives),
        personality_symbol: None,
        builtin_registry,
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".to_string(),
        export_symbol: true,
        location: statements_1.first().unwrap().location,
        body: Arc::new(statements_1),
        calls: Arc::new(calls1),
        resolver,
        signature,
        store,
        id: 0,
    };
    let f = Arc::new(WithCall::new(Box::new(|module| {
        insta::assert_snapshot!(
            function_name!(),
            module.print_to_string().to_str().map(str::trim).unwrap()
        );
    })));

    let (registry, handles) =
        WorkerRegistry::create_workers(threads, top_level, &codegen_options(), &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);
}

/// Regression test for list-repetition refcounting (issue #814).
///
/// Ensure that the generated IR for a list repetition expression `[L] * n` contains a single
/// `@__nac3_refcount_incr_by(elem, n)` call over a loop bounded by the *source* length `len(L)`,
/// rather than a single `@__nac3_refcount_incr(elem)` call a loop over all copied slots.
// The snapshot pins the emitted allocation calls, which depend on which allocator entry point is
// compiled in, so it is only asserted for the default (CTRC) configuration.
#[cfg(feature = "ctrc")]
#[test]
#[named]
fn test_list_mul_refcount() {
    let source = indoc! { "
        l = [[0]] * 15
        return 0
        "};
    let statements = parse_program(source, FileName::default()).unwrap();
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);

    let composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry.clone(), 64).0;
    let mut unifier = composer.unifier.clone();
    let primitives = composer.primitives_ty;
    let top_level = composer.make_top_level_context();

    let resolver =
        Arc::new(Resolver { id_to_type: HashMap::new(), id_to_def: RwLock::new(HashMap::new()) })
            as Arc<dyn SymbolResolver + Send + Sync>;

    let threads = vec![DefaultCodeGenerator::new("test".into()).into()];
    let signature = FunSignature { args: vec![], ret: primitives.int32, vars: VarMap::new() };

    let mut store = ConcreteTypeStore::new();
    let mut cache = HashMap::new();
    let signature = store.from_signature(&mut unifier, &primitives, &signature, &mut cache);
    let signature = store.add_cty(signature);

    let mut function_data = FunctionData {
        resolver: resolver.clone(),
        bound_variables: Vec::new(),
        return_type: Some(primitives.int32),
    };
    let mut virtual_checks = Vec::new();
    let mut calls = HashMap::new();
    let mut identifiers: HashSet<_, _> = HashSet::new();
    let mut inferencer = Inferencer {
        builtin_registry: &*top_level.builtin_registry,
        top_level_defs: &top_level.definitions,
        function_data: &mut function_data,
        unifier: &mut unifier,
        variable_mapping: HashMap::default(),
        primitives: &primitives,
        virtual_checks: &mut virtual_checks,
        calls: &mut calls,
        defined_identifiers: identifiers.clone(),
        in_handler: false,
    };

    let statements = statements
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    inferencer.check_block(&statements, &mut identifiers).unwrap();
    let top_level = Arc::new(TopLevelContext {
        definitions: top_level.definitions,
        unifiers: (unifier.get_shared_unifier(), primitives),
        personality_symbol: None,
        builtin_registry,
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".into(),
        export_symbol: true,
        location: statements.first().unwrap().location,
        body: Arc::new(statements),
        calls: Arc::new(calls),
        resolver,
        store,
        signature,
        id: 0,
    };
    let f = Arc::new(WithCall::new(Box::new(|module| {
        insta::assert_snapshot!(
            function_name!(),
            module.print_to_string().to_str().map(str::trim).unwrap()
        );
    })));

    let (registry, handles) =
        WorkerRegistry::create_workers(threads, top_level, &codegen_options(), &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);
}

// ---------------------------------------------------------------------------
// Type layout tests — assert LLVM struct layouts for refcounted types
// ---------------------------------------------------------------------------

mod layout {
    use std::{collections::HashMap, sync::Arc};

    use indexmap::IndexMap;
    use inkwell::{
        OptimizationLevel,
        debug_info::{AsDIScope, DWARFEmissionKind, DWARFSourceLanguage},
        targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetData},
        types::{BasicTypeEnum, StructType},
    };
    #[cfg(feature = "malloc")]
    use inkwell::{passes::PassBuilderOptions, values::AnyValue};
    use nac3parser::ast::Location;
    use parking_lot::RwLock;

    #[cfg(feature = "malloc")]
    use crate::codegen::{allocator::AllocationScope, type_aligned_allocate};
    use crate::{
        codegen::{
            CodeGenContext, CodeGenOptions, DefaultCodeGenerator, ModuleContext,
            TargetMachineOptions, WithCall, WorkerRegistry, context_ref,
            types::{
                ClassType, ExceptionType, NDArrayType, ObjectHeaderType, OptionSomeType, ProxyType,
                RefCountedArrayType, RefType, StringType, TupleType, TypeinfoType,
            },
        },
        symbol_resolver::SymbolResolver,
        toplevel::{
            DefinitionId,
            composer::{DefaultBuiltinRegistry, TopLevelComposer},
        },
        typecheck::typedef::{AttrKind, TypeEnum},
    };

    fn codegen_options() -> CodeGenOptions {
        Target::initialize_native(&InitializationConfig::default()).unwrap();
        // We don't really care about the options since we are just inspect the generated layouts
        CodeGenOptions {
            opt_level: String::from("0"),
            debug: true,
            target: TargetMachineOptions::from_host_triple(OptimizationLevel::None),
        }
    }

    /// Formats an LLVM struct layout: type string, ABI size/alignment, and per-field offsets.
    fn format_layout(ctx: &ModuleContext<'_>, ty: BasicTypeEnum<'_>, label: &str) -> String {
        let dl = ctx.target.get_target_data();
        let mut lines = Vec::new();

        lines.push(format!("{label}:"));
        lines.push(format!("  llvm_type: {}", ty.print_to_string().to_string_lossy()));
        lines.push(format!("  abi_size: {}", dl.get_abi_size(&ty)));
        lines.push(format!("  abi_alignment: {}", dl.get_abi_alignment(&ty)));

        if let BasicTypeEnum::StructType(st) = ty {
            lines.push(format!("  fields ({}):", st.count_fields()));
            for i in 0..st.count_fields() {
                let field_ty = unsafe { st.get_field_type_at_index_unchecked(i) };
                let offset = dl.offset_of_element(&st, i).unwrap();
                lines.push(format!(
                    "    [{i}] offset={offset}, type={}",
                    field_ty.print_to_string().to_string_lossy()
                ));
            }
        }

        lines.join("\n")
    }

    /// Formats a struct and its immediate sub-structs.
    fn format_layout_recursive(
        ctx: &ModuleContext<'_>,
        ty: BasicTypeEnum<'_>,
        label: &str,
    ) -> String {
        let mut parts = vec![format_layout(ctx, ty, label)];

        if let BasicTypeEnum::StructType(st) = ty {
            for i in 0..st.count_fields() {
                let field_ty = unsafe { st.get_field_type_at_index_unchecked(i) };
                if let BasicTypeEnum::StructType(_) = field_ty {
                    parts.push(format_layout(ctx, field_ty, &format!("{label}.field[{i}]")));
                }
            }
        }

        parts.join("\n")
    }

    fn create_module_context_64(ctx_ref: inkwell::context::ContextRef<'_>) -> ModuleContext<'_> {
        Target::initialize_x86(&InitializationConfig::default());
        let options = TargetMachineOptions {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            cpu: String::new(),
            features: String::new(),
            reloc_mode: inkwell::targets::RelocMode::Default,
            code_model: inkwell::targets::CodeModel::Default,
            target_opt_level: OptimizationLevel::None,
        };
        ModuleContext::new(ctx_ref, "test_layout_64", &options)
    }

    fn create_module_context_32(ctx_ref: inkwell::context::ContextRef<'_>) -> ModuleContext<'_> {
        Target::initialize_x86(&InitializationConfig::default());
        let options = TargetMachineOptions {
            triple: "i686-unknown-linux-gnu".to_string(),
            cpu: String::new(),
            features: String::new(),
            reloc_mode: inkwell::targets::RelocMode::Default,
            code_model: inkwell::targets::CodeModel::Default,
            target_opt_level: OptimizationLevel::None,
        };
        ModuleContext::new(ctx_ref, "test_layout_32", &options)
    }

    // TODO(ivan): CodeGenContext holds too many invariants to be meaningfully constructed for unit tests
    //             Consider refactoring a subset of fields and functions out for unit testing purposes
    fn create_codegen_context<'ctx, 'a>(
        ctx: &'a mut ModuleContext<'ctx>,
        registry: &'a WorkerRegistry,
        composer: &TopLevelComposer,
    ) -> CodeGenContext<'ctx, 'a> {
        let (dibuilder, compile_unit) = ctx.module.create_debug_info_builder(
            true,
            DWARFSourceLanguage::Python,
            "<dummy>",
            "",
            "NAC3",
            false,
            "",
            0,
            "",
            DWARFEmissionKind::Full,
            0,
            true,
            false,
            "",
            "",
        );
        let scope = compile_unit.as_debug_info_scope();
        let resolver = Arc::new(super::Resolver {
            id_to_type: HashMap::new(),
            id_to_def: RwLock::new(HashMap::new()),
        }) as Arc<dyn SymbolResolver + Send + Sync>;
        let (_, fn_val) = ctx.declare_internal("dummy", None, &[], false);

        let init_bb = ctx.ctx.append_basic_block(fn_val, "entry");
        let init_builder = ctx.ctx.create_builder(); /* dummy */
        let builder = ctx.ctx.create_builder();
        builder.position_at_end(init_bb);

        let exception_val = ctx.ptr.const_null();

        CodeGenContext {
            inner: ctx,
            builder,
            debug_info: (dibuilder, compile_unit, scope),
            top_level: &registry.top_level_ctx,
            unifier: composer.unifier.clone(),
            resolver,
            static_value_store: registry.static_value_store.clone(),
            var_assignment: HashMap::new(),
            type_cache: HashMap::new(),
            alloca_type_cache: HashMap::new(),
            primitives: composer.primitives_ty,
            calls: Arc::new(HashMap::new()),
            registry,
            const_strings: HashMap::new(),
            init_builder,
            exception_val,
            loop_target: None,
            unwind_target: None,
            return_target: None,
            return_buffer: None,
            return_buffer_type: None,
            outer_catch_clauses: None,
            current_loc: Location::default(),
        }
    }

    /// Generates the layout snapshot string for all refcounted types.
    fn generate_layouts(ctx: &mut CodeGenContext<'_, '_>) -> String {
        let mut sections = Vec::new();

        // ObjectHeader
        let header_ty = ObjectHeaderType::new(ctx);
        sections.push(format_layout(ctx, header_ty.alloca_ty(ctx), "ObjectHeader"));

        // Typeinfo
        let typeinfo_ty = TypeinfoType::new(ctx);
        sections.push(format_layout(ctx, typeinfo_ty.llvm_ty(ctx), "Typeinfo"));

        // RefCountedArray<i32> (e.g., list[int32] data backing)
        let rc_array_i32 = RefCountedArrayType::new(ctx, ctx.i32, Some(0));
        sections.push(format_layout_recursive(
            ctx,
            rc_array_i32.alloca_ty(ctx),
            "RefCountedArray<i32>",
        ));

        // List: { ObjectHeader, { ptr items, size_t len } }
        // Built manually since ListType::create requires a unifier Type.
        let list_inner = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.size_t.into()], false);
        let list_header = header_ty.alloca_ty(ctx).into_struct_type();
        let list_outer = ctx.ctx.struct_type(&[list_header.into(), list_inner.into()], false);
        sections.push(format_layout_recursive(ctx, list_outer.into(), "List"));

        // NDArray<i32, ndims=1>
        let ndarray_ty = NDArrayType::create(ctx, ctx.i32.into(), 1);
        sections.push(format_layout_recursive(ctx, ndarray_ty.alloca_ty(ctx), "NDArray<i32, 1>"));

        // OptionSome<i32>
        let option_some_i32 = OptionSomeType::new(ctx, ctx.i32.into());
        sections.push(format_layout_recursive(
            ctx,
            option_some_i32.alloca_ty(ctx),
            "OptionSome<i32>",
        ));

        // OptionSome<ptr> (refcounted element)
        let option_some_ptr = OptionSomeType::new(ctx, ctx.ptr.into());
        sections.push(format_layout_recursive(
            ctx,
            option_some_ptr.alloca_ty(ctx),
            "OptionSome<ptr>",
        ));

        // Tuple<i32, i32>
        let tuple_i32_i32 = TupleType::new(ctx, &[ctx.i32.into(), ctx.i32.into()]);
        sections.push(format_layout_recursive(ctx, tuple_i32_i32.llvm_ty(ctx), "Tuple<i32, i32>"));

        // Tuple<ptr, i32> (one refcounted field)
        let tuple_ptr_i32 = TupleType::new(ctx, &[ctx.ptr.into(), ctx.i32.into()]);
        sections.push(format_layout_recursive(ctx, tuple_ptr_i32.llvm_ty(ctx), "Tuple<ptr, i32>"));

        // Class with two i32 fields (like Point)
        let point_inner = ctx.ctx.struct_type(&[ctx.i32.into(), ctx.i32.into()], false);
        let point_ty = ctx.unifier.add_ty(TypeEnum::TObj {
            obj_id: DefinitionId(100),
            fields: HashMap::from([
                ("x".into(), (ctx.primitives.int32, AttrKind::Field { mutable: true })),
                ("y".into(), (ctx.primitives.int32, AttrKind::Field { mutable: true })),
            ]),
            params: IndexMap::new(),
        });
        let point_class = ClassType::create(ctx, point_inner, point_ty, "Class<i32, i32>".into());
        sections.push(format_layout_recursive(ctx, point_class.alloca_ty(ctx), "Class<i32, i32>"));

        // Class with two ptr fields (like Container)
        let container_inner = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.ptr.into()], false);
        let container_ty = ctx.unifier.add_ty(TypeEnum::TObj {
            obj_id: DefinitionId(101),
            fields: HashMap::from([
                ("l1".into(), (ctx.primitives.list, AttrKind::Field { mutable: true })),
                ("l2".into(), (ctx.primitives.list, AttrKind::Field { mutable: true })),
            ]),
            params: IndexMap::new(),
        });
        let container_class =
            ClassType::create(ctx, container_inner, container_ty, "Class<ptr, ptr>".into());
        sections.push(format_layout_recursive(
            ctx,
            container_class.alloca_ty(ctx),
            "Class<ptr, ptr>",
        ));

        sections.join("\n\n")
    }

    #[test]
    fn test_type_layouts_64bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_64(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 64).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let mut ctx = create_codegen_context(&mut ctx, &registry, &composer);
        insta::assert_snapshot!(generate_layouts(&mut ctx));
    }

    #[test]
    fn test_type_layouts_32bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_32(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 32).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let mut ctx = create_codegen_context(&mut ctx, &registry, &composer);
        insta::assert_snapshot!(generate_layouts(&mut ctx));
    }

    #[test]
    fn test_shared_type_layouts_are_datalayout_invariant_64bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_64(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 64).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let ctx = create_codegen_context(&mut ctx, &registry, &composer);
        let layouts = collect_datalayouts(IRRT_DATALAYOUT_64, TARGET_TRIPLES_64);
        for (label, ty) in shared_irrt_types(&ctx) {
            assert_layout_invariant(&label, ty, &layouts);
        }
    }

    #[test]
    fn test_shared_type_layouts_are_datalayout_invariant_32bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_32(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 32).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let ctx = create_codegen_context(&mut ctx, &registry, &composer);
        let layouts = collect_datalayouts(IRRT_DATALAYOUT_32, TARGET_TRIPLES_32);
        for (label, ty) in shared_irrt_types(&ctx) {
            assert_layout_invariant(&label, ty, &layouts);
        }
    }

    // The datalayout IRRT is compiled with.
    const IRRT_DATALAYOUT_32: &str = include_str!(concat!(env!("OUT_DIR"), "/irrt32.datalayout"));
    const IRRT_DATALAYOUT_64: &str = include_str!(concat!(env!("OUT_DIR"), "/irrt64.datalayout"));

    /// 32-bit targets NAC3 emits code for.
    ///
    /// Note that `i686` is the odd one out in terms of alignment: its datalayout gives `i64` and
    /// `f64` an ABI alignment of 4, where wasm32, armv7 and riscv32 all use 8.
    const TARGET_TRIPLES_32: &[&str] =
        &["i686-unknown-linux-gnu", "armv7-unknown-linux-gnueabihf", "riscv32-unknown-none-elf"];

    /// 64-bit targets NAC3 emits code for.
    const TARGET_TRIPLES_64: &[&str] = &["x86_64-unknown-linux-gnu", "riscv64-unknown-none-elf"];

    /// Collects the datalayout IRRT was compiled with, plus the datalayout of each given target.
    fn collect_datalayouts(irrt_datalayout: &str, triples: &[&str]) -> Vec<(String, TargetData)> {
        let config = InitializationConfig::default();
        Target::initialize_x86(&config);
        Target::initialize_arm(&config);
        Target::initialize_riscv(&config);

        // `TargetData::create` parses a datalayout string directly, so the IRRT (wasm) datalayout
        // is usable even though `llvm-nac3` here is not built with the Wasm backend.
        let mut layouts = vec![("IRRT".to_string(), TargetData::create(irrt_datalayout.trim()))];
        layouts.extend(triples.iter().map(|triple| {
            let options = TargetMachineOptions {
                triple: (*triple).to_string(),
                cpu: String::new(),
                features: String::new(),
                reloc_mode: RelocMode::Default,
                code_model: CodeModel::Default,
                target_opt_level: OptimizationLevel::None,
            };
            ((*triple).to_string(), options.create_target_machine().get_target_data())
        }));

        layouts
    }

    /// Asserts that `ty` has the same ABI size and the same field offsets under every datalayout.
    ///
    /// ABI *alignment* is deliberately not compared: it may legitimately differ (e.g. a struct
    /// holding a `double` is 4-aligned on i686 and 8-aligned on armv7) without moving any field.
    fn assert_layout_invariant(
        label: &str,
        struct_ty: StructType<'_>,
        layouts: &[(String, TargetData)],
    ) {
        let describe = |dl: &TargetData| {
            let offsets: Vec<_> = (0..struct_ty.count_fields())
                .map(|i| dl.offset_of_element(&struct_ty, i).unwrap())
                .collect();
            (dl.get_abi_size(&struct_ty), offsets)
        };

        let (reference_name, reference_dl) = &layouts[0];
        let reference = describe(reference_dl);

        for (name, dl) in &layouts[1..] {
            assert_eq!(
                describe(dl),
                reference,
                "layout of `{label}` differs between {reference_name} and {name} \
                 (abi_size, field offsets); a struct shared with IRRT must lay out identically \
                 under every datalayout - see `Exception` in nac3core/irrt/irrt/exception.hpp"
            );
        }
    }

    /// Returns the types that are shared between IRRT and codegen, i.e. those with a C++
    /// counterpart in `nac3core/irrt`.
    ///
    /// Types without an IRRT counterpart (tuples, classes, `Option`) are deliberately excluded:
    /// IRRT only ever reaches their fields through offsets computed by codegen at runtime, so they
    /// are free to lay out differently per target.
    fn shared_irrt_types<'ctx>(ctx: &CodeGenContext<'ctx, '_>) -> Vec<(String, StructType<'ctx>)> {
        vec![
            (
                "ObjectHeader".to_string(),
                ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type(),
            ),
            ("Typeinfo".to_string(), TypeinfoType::new(ctx).alloca_ty(ctx).into_struct_type()),
            // `String` must be built before `Exception`, which refers to the `str` named struct.
            ("String".to_string(), StringType::new(ctx).llvm_ty(ctx).into_struct_type()),
            ("Exception".to_string(), ExceptionType::new(ctx).alloca_ty(ctx).into_struct_type()),
            // `RefCountedArray<f64>` is the #797 case: an 8-byte element type on a 32-bit target,
            // which only stays in agreement because of the explicit count padding.
            (
                "RefCountedArray<i32>".to_string(),
                RefCountedArrayType::new(ctx, ctx.i32, Some(0)).alloca_ty(ctx).into_struct_type(),
            ),
            (
                "RefCountedArray<f64>".to_string(),
                RefCountedArrayType::new(ctx, ctx.f64, Some(0)).alloca_ty(ctx).into_struct_type(),
            ),
            (
                "NDArray<i32, 1>".to_string(),
                NDArrayType::create(ctx, ctx.i32.into(), 1).alloca_ty(ctx).into_struct_type(),
            ),
        ]
    }

    /// Extracts the size argument of the (single) allocation call in `ir`.
    ///
    /// The allocator entry point depends on the `ctrc` feature: `@__nac3_alloc(size, align)` for
    /// the CTRC slab, or `@malloc(size)` otherwise. Only the leading `size` argument is read, so
    /// both forms are handled by the same extraction.
    ///
    /// Panics if the call is missing, or if its size argument has not been folded to an integer
    /// literal.
    #[cfg(feature = "malloc")]
    fn parse_alloc_size(ir: &str) -> u64 {
        #[cfg(feature = "ctrc")]
        const ALLOC_FN: &str = "@__nac3_alloc";
        #[cfg(not(feature = "ctrc"))]
        const ALLOC_FN: &str = "@malloc";

        let args = ir
            .split_once(&format!("{ALLOC_FN}("))
            .unwrap_or_else(|| panic!("expected a `{ALLOC_FN}` call in the generated IR"))
            .1
            .split_once(')')
            .unwrap_or_else(|| panic!("malformed `{ALLOC_FN}` call"))
            .0;
        let size_arg = args.split(',').next().unwrap();
        let size_val = size_arg.split_whitespace().last().unwrap();
        size_val.parse().unwrap_or_else(|_| {
            panic!("expected the `{ALLOC_FN}` size to be a folded integer literal")
        })
    }

    /// Emits a single [`type_aligned_allocate`] for a `RefCountedArray<i32>` backing buffer - whose
    /// ABI size (16 bytes) differs from its alignment - and returns the pre-optimization IR of the
    /// emitting function together with the `(actual, expected)` allocation byte counts.
    ///
    /// `actual` is read back from the `malloc` call after constant-folding; `expected` is the
    /// correct `ceil(size / sizeof) * sizeof`.
    #[cfg(feature = "malloc")]
    fn run_type_aligned_allocate(ctx: &mut CodeGenContext<'_, '_>) -> (String, u64, u64) {
        const BUFFER_SIZE: u64 = 1024;

        let align_ty =
            RefCountedArrayType::new(ctx, ctx.i32, Some(0)).alloca_ty(ctx).into_struct_type();
        let sizeof_align = ctx.sizeof(align_ty);

        // `size_val` mirrors the real backing-buffer request for `[int32(0)] * BUFFER_SIZE`
        let sizeof_alloc = ctx.sizeof(align_ty) + ctx.sizeof(ctx.i32) * BUFFER_SIZE;

        let size = ctx.size_t.const_int(sizeof_alloc, false);
        let slice =
            type_aligned_allocate(ctx, AllocationScope::Heap, align_ty, size, None).unwrap();
        // Store the pointer into a global so the `malloc` call is not eliminated as dead code.
        let keep = ctx.module.add_global(ctx.ptr, None, "keep_alloc");
        ctx.builder.build_store(keep.as_pointer_value(), slice.value.0).unwrap();
        ctx.builder.build_return(None).unwrap();

        let fn_val = ctx.builder.get_insert_block().unwrap().get_parent().unwrap();
        let ir = fn_val.print_to_string().to_string();

        // Constant-fold the (target-data-dependent) allocation size so it can be read back.
        ctx.module.run_passes("instcombine", &ctx.target, PassBuilderOptions::create()).unwrap();
        let actual = parse_alloc_size(&fn_val.print_to_string().to_string());

        let expected = sizeof_alloc.div_ceil(sizeof_align) * sizeof_align;

        (ir, actual, expected)
    }

    #[test]
    #[cfg(feature = "malloc")]
    fn test_type_aligned_allocate_64bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_64(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 64).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let mut ctx = create_codegen_context(&mut ctx, &registry, &composer);
        let (ir, actual, expected) = run_type_aligned_allocate(&mut ctx);

        assert_eq!(actual, expected);

        // The emitted IR depends on which allocator entry point is compiled in, so it is only
        // pinned for the default (CTRC) configuration; the size assertion above covers both.
        #[cfg(feature = "ctrc")]
        insta::assert_snapshot!(ir);
        #[cfg(not(feature = "ctrc"))]
        drop(ir);
    }

    #[test]
    #[cfg(feature = "malloc")]
    fn test_type_aligned_allocate_32bit() {
        context_ref!(ctx_ref);

        let mut ctx = create_module_context_32(ctx_ref);
        let composer =
            TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(DefaultBuiltinRegistry), 32).0;
        let top_level = Arc::new(composer.make_top_level_context());
        let registry = WorkerRegistry::create_workers(
            Vec::<Box<DefaultCodeGenerator>>::new(),
            top_level,
            &codegen_options(),
            &Arc::new(WithCall::new(Box::new(|_| {}))),
        )
        .0;

        let mut ctx = create_codegen_context(&mut ctx, &registry, &composer);
        let (ir, actual, expected) = run_type_aligned_allocate(&mut ctx);

        assert_eq!(actual, expected);

        // The emitted IR depends on which allocator entry point is compiled in, so it is only
        // pinned for the default (CTRC) configuration; the size assertion above covers both.
        #[cfg(feature = "ctrc")]
        insta::assert_snapshot!(ir);
        #[cfg(not(feature = "ctrc"))]
        drop(ir);
    }
}
