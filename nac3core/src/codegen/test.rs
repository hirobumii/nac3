use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use function_name::named;
use indexmap::IndexMap;
use indoc::indoc;
use inkwell::{
    OptimizationLevel,
    targets::{InitializationConfig, Target},
};
use parking_lot::RwLock;

use nac3parser::{
    ast::{FileName, StrRef, fold::Fold},
    parser::parse_program,
};

use super::{
    CodeGenContext, CodeGenLLVMOptions, CodeGenTargetMachineOptions, CodeGenTask, CodeGenerator,
    DefaultCodeGenerator, WithCall, WorkerRegistry,
    concrete_type::ConcreteTypeStore,
    types::{ListType, ProxyType, RangeType, ndarray::NDArrayType},
};
use crate::{
    symbol_resolver::{SymbolResolver, ValueEnum},
    toplevel::{
        DefinitionId, FunInstance, TopLevelContext, TopLevelDef,
        composer::{ComposerConfig, TopLevelComposer},
    },
    typecheck::{
        type_inferencer::{FunctionData, IdentifierInfo, Inferencer, PrimitiveStore},
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
    ) -> Option<crate::symbol_resolver::SymbolValue> {
        unimplemented!()
    }

    fn get_symbol_type(
        &self,
        _: &mut Unifier,
        _: &[Arc<RwLock<TopLevelDef>>],
        _: &PrimitiveStore,
        str: StrRef,
    ) -> Result<Type, String> {
        self.id_to_type.get(&str).copied().ok_or_else(|| format!("cannot find symbol `{str}`"))
    }

    fn get_symbol_value<'ctx>(
        &self,
        _: StrRef,
        _: &mut CodeGenContext<'ctx, '_>,
        _: &mut dyn CodeGenerator,
    ) -> Option<ValueEnum<'ctx>> {
        unimplemented!()
    }

    fn get_identifier_def(&self, id: StrRef) -> Result<DefinitionId, HashSet<String>> {
        self.id_to_def
            .read()
            .get(&id)
            .copied()
            .ok_or_else(|| HashSet::from([format!("cannot find symbol `{id}`")]))
    }

    fn get_string_id(&self, _: &str) -> i32 {
        unimplemented!()
    }

    fn get_exception_id(&self, _tyid: usize) -> usize {
        unimplemented!()
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

    let context = inkwell::context::Context::create();
    let composer = TopLevelComposer::new(Vec::new(), Vec::new(), ComposerConfig::default(), 64).0;
    let mut unifier = composer.unifier.clone();
    let primitives = composer.primitives_ty;
    let top_level = Arc::new(composer.make_top_level_context());
    unifier.top_level = Some(top_level.clone());

    let resolver =
        Arc::new(Resolver { id_to_type: HashMap::new(), id_to_def: RwLock::new(HashMap::new()) })
            as Arc<dyn SymbolResolver + Send + Sync>;

    let threads = vec![DefaultCodeGenerator::new("test".into(), context.i64_type()).into()];
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
    let mut identifiers: HashMap<_, _> =
        ["a".into(), "b".into()].map(|id| (id, IdentifierInfo::default())).into();
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
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".into(),
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

    Target::initialize_all(&InitializationConfig::default());

    let llvm_options = CodeGenLLVMOptions {
        opt_level: OptimizationLevel::Default,
        target: CodeGenTargetMachineOptions::from_host_triple(),
    };
    let (registry, handles) = WorkerRegistry::create_workers(threads, top_level, &llvm_options, &f);
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

    let context = inkwell::context::Context::create();
    let composer = TopLevelComposer::new(Vec::new(), Vec::new(), ComposerConfig::default(), 64).0;
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

    let threads = vec![DefaultCodeGenerator::new("test".into(), context.i64_type()).into()];
    let mut function_data = FunctionData {
        resolver: resolver.clone(),
        bound_variables: Vec::new(),
        return_type: Some(primitives.int32),
    };
    let mut virtual_checks = Vec::new();
    let mut calls = HashMap::new();
    let mut identifiers: HashMap<_, _> =
        ["a".into(), "foo".into()].map(|id| (id, IdentifierInfo::default())).into();
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
    });

    let task = CodeGenTask {
        subst: Vec::default(),
        symbol_name: "testing".to_string(),
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

    Target::initialize_all(&InitializationConfig::default());

    let llvm_options = CodeGenLLVMOptions {
        opt_level: OptimizationLevel::Default,
        target: CodeGenTargetMachineOptions::from_host_triple(),
    };
    let (registry, handles) = WorkerRegistry::create_workers(threads, top_level, &llvm_options, &f);
    registry.add_task(task);
    registry.wait_tasks_complete(handles);
}

#[test]
fn test_classes_list_type_new() {
    let ctx = inkwell::context::Context::create();
    let generator = DefaultCodeGenerator::new(String::new(), ctx.i64_type());

    let llvm_i32 = ctx.i32_type();
    let llvm_usize = generator.get_size_type(&ctx);

    let llvm_list = ListType::new_with_generator(&generator, &ctx, llvm_i32.into());
    assert!(ListType::is_representable(llvm_list.as_abi_type(), llvm_usize).is_ok());
}

#[test]
fn test_classes_range_type_new() {
    let ctx = inkwell::context::Context::create();
    let generator = DefaultCodeGenerator::new(String::new(), ctx.i64_type());

    let llvm_usize = generator.get_size_type(&ctx);

    let llvm_range = RangeType::new_with_generator(&generator, &ctx);
    assert!(RangeType::is_representable(llvm_range.as_abi_type(), llvm_usize).is_ok());
}

#[test]
fn test_classes_ndarray_type_new() {
    let ctx = inkwell::context::Context::create();
    let generator = DefaultCodeGenerator::new(String::new(), ctx.i64_type());

    let llvm_i32 = ctx.i32_type();
    let llvm_usize = generator.get_size_type(&ctx);

    let llvm_ndarray = NDArrayType::new_with_generator(&generator, &ctx, llvm_i32.into(), 2);
    assert!(NDArrayType::is_representable(llvm_ndarray.as_abi_type(), llvm_usize).is_ok());
}
