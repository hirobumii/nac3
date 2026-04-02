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
        unimplemented!()
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
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);

    let composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry.clone(), 64).0;
    let mut unifier = composer.unifier.clone();
    let primitives = composer.primitives_ty;
    let top_level = Arc::new(composer.make_top_level_context());
    unifier.top_level = Some(top_level.clone());

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
        top_level: &top_level,
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
        definitions: Arc::new(RwLock::new(std::mem::take(&mut *top_level.definitions.write()))),
        unifiers: Arc::new(RwLock::new(vec![(unifier.get_shared_unifier(), primitives)])),
        personality_symbol: None,
        builtin_registry,
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".into(),
        export_symbol: true,
        body: Arc::new(statements),
        unifier_index: 0,
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
    let top_level = Arc::new(composer.make_top_level_context());
    unifier.top_level = Some(top_level.clone());

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

    let foo_id = top_level.definitions.read().len();
    top_level.definitions.write().push(Arc::new(RwLock::new(TopLevelDef::Function {
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

    if let TopLevelDef::Function { resolver: r, .. } =
        &mut *top_level.definitions.read()[foo_id].write()
    {
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
        top_level: &top_level,
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
        &mut *top_level.definitions.read()[foo_id].write()
    {
        instance_to_stmt.insert(
            String::new(),
            FunInstance {
                body: Arc::new(statements_2),
                calls: Arc::new(inferencer.calls.clone()),
                subst: IndexMap::default(),
                unifier_id: 0,
            },
        );
    } else {
        unreachable!()
    }

    inferencer.check_block(&statements_1, &mut identifiers).unwrap();
    let top_level = Arc::new(TopLevelContext {
        definitions: Arc::new(RwLock::new(std::mem::take(&mut *top_level.definitions.write()))),
        unifiers: Arc::new(RwLock::new(vec![(unifier.get_shared_unifier(), primitives)])),
        personality_symbol: None,
        builtin_registry,
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".to_string(),
        export_symbol: true,
        body: Arc::new(statements_1),
        calls: Arc::new(calls1),
        unifier_index: 0,
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

// ---------------------------------------------------------------------------
// Type layout tests — assert LLVM struct layouts for refcounted types
// ---------------------------------------------------------------------------

mod layout {
    use inkwell::{
        OptimizationLevel,
        targets::{InitializationConfig, Target},
        types::BasicTypeEnum,
    };

    use crate::{
        codegen::{
            ModuleContext, TargetMachineOptions,
            types::{
                ClassType, NDArrayType, ObjectHeaderType, OptionSomeType, ProxyType,
                RefCountedArrayType, RefType, TupleType, TypeinfoType,
            },
        },
        toplevel::DefinitionId,
    };

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

    /// Generates the layout snapshot string for all refcounted types.
    fn generate_layouts(ctx: &ModuleContext<'_>) -> String {
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
        sections
            .push(format_layout_recursive(ctx, ndarray_ty.alloca_ty(ctx), "NDArray<i32, 1>"));

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
        sections.push(format_layout_recursive(
            ctx,
            tuple_i32_i32.llvm_ty(ctx),
            "Tuple<i32, i32>",
        ));

        // Tuple<ptr, i32> (one refcounted field)
        let tuple_ptr_i32 = TupleType::new(ctx, &[ctx.ptr.into(), ctx.i32.into()]);
        sections.push(format_layout_recursive(
            ctx,
            tuple_ptr_i32.llvm_ty(ctx),
            "Tuple<ptr, i32>",
        ));

        // Class with two i32 fields (like Point)
        let point_inner = ctx.ctx.struct_type(&[ctx.i32.into(), ctx.i32.into()], false);
        let point_class = ClassType::create(ctx, point_inner, DefinitionId(100), 0);
        sections.push(format_layout_recursive(
            ctx,
            point_class.alloca_ty(ctx),
            "Class<i32, i32>",
        ));

        // Class with two ptr fields (like Container)
        let container_inner = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.ptr.into()], false);
        let container_class = ClassType::create(ctx, container_inner, DefinitionId(101), 0b11);
        sections.push(format_layout_recursive(
            ctx,
            container_class.alloca_ty(ctx),
            "Class<ptr, ptr>",
        ));

        sections.join("\n\n")
    }

    #[test]
    fn test_type_layouts_64bit() {
        crate::codegen::context_ref!(ctx_ref);
        let ctx = create_module_context_64(ctx_ref);
        insta::assert_snapshot!(generate_layouts(&ctx));
    }

    #[test]
    fn test_type_layouts_32bit() {
        crate::codegen::context_ref!(ctx_ref);
        let ctx = create_module_context_32(ctx_ref);
        insta::assert_snapshot!(generate_layouts(&ctx));
    }
}
