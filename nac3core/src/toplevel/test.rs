use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use indoc::indoc;
use itertools::Itertools as _;
use nac3parser::{
    ast::{self, FileName, StrRef, fold::Fold},
    parser::{parse_expression, parse_program},
};
use parking_lot::{Mutex, RwLock};
use test_case::test_case;

use crate::{
    codegen::CodeGenContext,
    symbol_resolver::{SymbolResolver, ValueEnum},
    toplevel::{
        DefinitionId, TopLevelDef,
        composer::{
            BuiltinRegistry, CatSeqBuiltinRegistry, DefaultBuiltinRegistry, SourceProfile,
            TopLevelComposer,
        },
        helper::PrimDef,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{AttrKind, FunSignature, Type, TypeEnum, Unifier, VarMap},
    },
};

struct ResolverInternal {
    id_to_type: Mutex<HashMap<StrRef, Type>>,
    id_to_def: Mutex<HashMap<StrRef, DefinitionId>>,
    auto_field_types: Mutex<HashMap<(StrRef, StrRef), Type>>,
    deferred_unifications: Mutex<Vec<(Type, Type)>>,
}

impl ResolverInternal {
    fn add_id_def(&self, id: StrRef, def: DefinitionId) {
        self.id_to_def.lock().insert(id, def);
    }

    fn add_id_type(&self, id: StrRef, ty: Type) {
        self.id_to_type.lock().insert(id, ty);
    }

    fn add_auto_field_type(&self, class_name: StrRef, field_name: StrRef, ty: Type) {
        self.auto_field_types.lock().insert((class_name, field_name), ty);
    }
}

struct Resolver(Arc<ResolverInternal>);

impl SymbolResolver for Resolver {
    fn get_default_param_value(
        &self,
        _: &ast::Expr,
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
        self.0
            .id_to_type
            .lock()
            .get(&str)
            .copied()
            .ok_or_else(|| anyhow!("cannot find symbol `{str}`"))
    }

    fn get_symbol_value<'ctx>(
        &self,
        _: StrRef,
        _: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<Option<ValueEnum<'ctx>>> {
        unimplemented!()
    }

    fn get_identifier_def(&self, id: StrRef) -> Result<DefinitionId, Vec<anyhow::Error>> {
        self.0.id_to_def.lock().get(&id).copied().ok_or_else(|| vec![anyhow!("Unknown identifier")])
    }

    fn get_string_id(&self, _: &str) -> i32 {
        unimplemented!()
    }

    fn get_exception_id(&self, _tyid: usize) -> usize {
        unimplemented!()
    }

    fn resolve_auto_field_type(
        &self,
        class_name: StrRef,
        field_name: StrRef,
        _: &mut Unifier,
        _: &[Arc<RwLock<TopLevelDef>>],
        _: &PrimitiveStore,
    ) -> anyhow::Result<Option<Type>> {
        Ok(self.0.auto_field_types.lock().get(&(class_name, field_name)).copied())
    }

    fn handle_deferred_eval(
        &self,
        unifier: &mut Unifier,
        _: &[Arc<RwLock<TopLevelDef>>],
        _: &PrimitiveStore,
    ) -> anyhow::Result<()> {
        for (actual, expected) in self.0.deferred_unifications.lock().iter() {
            unifier
                .unify(*actual, *expected)
                .map_err(|error| anyhow!("{}", error.to_display(unifier)))?;
        }
        Ok(())
    }
}

#[test_case(
    vec![
        indoc! {"
            def fun(a: int32) -> int32:
                return a
        "},
        indoc! {"
            class A:
                def __init__(self):
                    self.a: int32 = 3
        "},
        indoc! {"
            class B:
                def __init__(self):
                    self.b: float = 4.3

                def fun(self):
                    self.b = self.b + 3.0
        "},
        indoc! {"
            def foo(a: float):
                a + 1.0
        "},
        indoc! {"
            class C(B):
                def __init__(self):
                    self.c: int32 = 4
                    self.a: bool = True
        "},
    ];
    "register"
)]
fn test_simple_register(source: Vec<&str>) {
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);
    let mut composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry, 64).0;

    for s in source {
        let ast = parse_program(s, FileName::default()).unwrap();
        let ast = ast[0].clone();

        composer.register_top_level(ast, None, "", false).unwrap();
    }
}

#[test_case(
    indoc! {"
        class A:
            def foo(self):
                pass
        a = A()
    "};
    "register"
)]
fn test_simple_register_without_constructor(source: &str) {
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);
    let mut composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry, 64).0;
    let ast = parse_program(source, FileName::default()).unwrap();
    let ast = ast[0].clone();
    composer.register_top_level(ast, None, "", true).unwrap();
}

#[test_case(
    &[
        indoc! {"
            def fun(a: int32) -> int32:
                return a
        "},
        indoc! {"
            def foo(a: float):
                a + 1.0
        "},
        indoc! {"
            def f(b: int64) -> int32:
                return 3
        "},
    ],
    &[
        "fn[[a:35], 35]",
        "fn[[a:0], 25]",
        "fn[[b:36], 35]",
    ],
    &[
        "fun",
        "foo",
        "f"
    ];
    "function compose"
)]
fn test_simple_function_analyze(source: &[&str], tys: &[&str], names: &[&str]) {
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);
    let mut composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry, 64).0;

    let internal_resolver = Arc::new(ResolverInternal {
        id_to_def: Mutex::default(),
        id_to_type: Mutex::default(),
        auto_field_types: Mutex::default(),
        deferred_unifications: Mutex::default(),
    });
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;

    for s in source {
        let ast = parse_program(s, FileName::default()).unwrap();
        let ast = ast[0].clone();

        let (id, def_id, ty) =
            composer.register_top_level(ast, Some(resolver.clone()), "", false).unwrap();
        internal_resolver.add_id_def(id, def_id);
        if let Some(ty) = ty {
            internal_resolver.add_id_type(id, ty);
        }
    }

    composer.start_analysis(true).unwrap();

    for (i, (def, _)) in composer.definition_ast_list.iter().skip(composer.builtin_num).enumerate()
    {
        let def = &*def.read();
        if let TopLevelDef::Function { signature, name, .. } = def {
            let ty_str = composer.unifier.internal_stringify(
                *signature,
                &mut |id| id.to_string(),
                &mut |id| id.to_string(),
                &mut None,
            );
            assert_eq!(ty_str, tys[i]);
            assert_eq!(name, names[i]);
        }
    }
}

fn new_catseq_composer()
-> (TopLevelComposer, Arc<ResolverInternal>, Arc<dyn SymbolResolver + Send + Sync>) {
    let composer =
        TopLevelComposer::new(Vec::new(), Vec::new(), Arc::new(CatSeqBuiltinRegistry), 64).0;
    let internal_resolver = Arc::new(ResolverInternal {
        id_to_def: Mutex::default(),
        id_to_type: Mutex::default(),
        auto_field_types: Mutex::default(),
        deferred_unifications: Mutex::default(),
    });
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;
    (composer, internal_resolver, resolver)
}

fn analyze_catseq_function(source: &str) -> Result<(TopLevelComposer, DefinitionId), String> {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let ast = parse_program(source, FileName::default()).unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true)?;
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());
    composer
        .start_analysis(true)
        .map_err(|errors| errors.into_iter().map(|error| error.to_string()).join("\n"))?;
    Ok((composer, definition_id))
}

fn analyze_catseq_with_late_bound_value(source: &str) -> Result<(), String> {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let value_type = composer.unifier.get_fresh_var_with_range(
        &[composer.primitives_ty.int32, composer.primitives_ty.float],
        Some("T".into()),
        None,
    );
    internal_resolver.add_id_type("x".into(), value_type.ty);
    let consume_float = composer.unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: vec![crate::typecheck::typedef::FuncArg {
            name: "value".into(),
            ty: composer.primitives_ty.float,
            default_value: None,
            is_vararg: false,
        }],
        ret: composer.primitives_ty.none,
        vars: VarMap::new(),
    }));
    internal_resolver.add_id_type("consume_float".into(), consume_float);
    let consume_pair = composer.unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: vec![
            crate::typecheck::typedef::FuncArg {
                name: "quotient".into(),
                ty: composer.primitives_ty.float,
                default_value: None,
                is_vararg: false,
            },
            crate::typecheck::typedef::FuncArg {
                name: "value".into(),
                ty: composer.primitives_ty.int32,
                default_value: None,
                is_vararg: false,
            },
        ],
        ret: composer.primitives_ty.none,
        vars: VarMap::new(),
    }));
    internal_resolver.add_id_type("consume_pair".into(), consume_pair);
    let consume_int_pair = composer.unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: vec![
            crate::typecheck::typedef::FuncArg {
                name: "quotient".into(),
                ty: composer.primitives_ty.int32,
                default_value: None,
                is_vararg: false,
            },
            crate::typecheck::typedef::FuncArg {
                name: "value".into(),
                ty: composer.primitives_ty.int32,
                default_value: None,
                is_vararg: false,
            },
        ],
        ret: composer.primitives_ty.none,
        vars: VarMap::new(),
    }));
    internal_resolver.add_id_type("consume_int_pair".into(), consume_int_pair);
    let ast = parse_program(source, FileName::default()).unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true)?;
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());
    composer
        .start_analysis(true)
        .map_err(|errors| errors.into_iter().map(|error| error.to_string()).join("\n"))
}

#[test]
fn catseq_source_profile_has_explicit_identity_without_changing_the_default() {
    assert_eq!(DefaultBuiltinRegistry.source_profile(), SourceProfile::Default);
    assert_eq!(CatSeqBuiltinRegistry.source_profile(), SourceProfile::CatSeqInt32);
    assert_eq!(SourceProfile::Default.abi_tag(), None);
    assert_eq!(SourceProfile::CatSeqInt32.abi_tag(), Some("catseq-int32-v1"));

    let int = parse_expression("int").unwrap();
    let int32 = parse_expression("int32").unwrap();
    let int64 = parse_expression("int64").unwrap();
    assert_eq!(DefaultBuiltinRegistry.match_builtin(&int), None);
    assert_eq!(CatSeqBuiltinRegistry.match_builtin(&int), Some(PrimDef::Int32));
    assert_eq!(CatSeqBuiltinRegistry.match_builtin(&int32), Some(PrimDef::Int32));
    assert_eq!(CatSeqBuiltinRegistry.match_builtin(&int64), None);
}

#[test]
fn catseq_source_profile_rejects_float_literals() {
    let result = analyze_catseq_function(indoc! {"
        def compute(value: int) -> int:
            temporary = 1.0
            return value
    "});
    let Err(error) = result else {
        panic!("floating-point literals must not type-check in the CatSeq source profile")
    };

    assert!(error.contains("floating-point literals are not supported by CatSeqInt32"), "{error}");
    assert!(error.contains("at unknown:2:17"), "{error}");
}

#[test]
fn catseq_source_profile_rejects_true_division() {
    let result = analyze_catseq_function(indoc! {"
        def compute(value: int) -> int:
            temporary = value / 2
            return value
    "});
    let Err(error) = result else {
        panic!("true division must not type-check in the CatSeq source profile")
    };

    assert!(error.contains("operator `/` is not supported by CatSeqInt32"), "{error}");
    assert!(error.contains("at unknown:2:23"), "{error}");
}

#[test]
fn catseq_source_profile_rejects_float_results_from_builtins() {
    let result = analyze_catseq_function(indoc! {"
        def compute(value: int) -> int:
            temporary = np_arctan2(value, value)
            return value
    "});
    let Err(error) = result else {
        panic!("floating-point builtin results must not type-check in the CatSeq source profile")
    };

    assert!(error.contains("floating-point values are not supported by CatSeqInt32"), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test]
fn catseq_source_profile_rejects_forbidden_numeric_types_nested_in_values() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let class_ast = parse_program(
        indoc! {"
            class ExternalRecord:
                pass
        "},
        FileName::default(),
    )
    .unwrap();
    let (class_name, class_id, class_ty) = composer
        .register_top_level(class_ast[0].clone(), Some(resolver.clone()), "", true)
        .unwrap();
    internal_resolver.add_id_def(class_name, class_id);
    internal_resolver.add_id_type(class_name, class_ty.unwrap());
    let external_record = composer.unifier.add_ty(TypeEnum::TObj {
        obj_id: class_id,
        fields: HashMap::from([(
            "sample".into(),
            (composer.primitives_ty.float, AttrKind::Field { mutable: false }),
        )]),
        params: VarMap::new(),
    });
    let external_result = composer.unifier.add_ty(TypeEnum::TTuple {
        ty: vec![composer.primitives_ty.int32, external_record],
        is_vararg_ctx: false,
    });
    let external_function = composer.unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: Vec::new(),
        ret: external_result,
        vars: VarMap::new(),
    }));
    internal_resolver.add_id_type("read_pair".into(), external_function);
    let ast = parse_program(
        indoc! {"
            def compute(value: int) -> int:
                temporary = read_pair()
                return value
        "},
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    let errors = composer
        .start_analysis(true)
        .expect_err("a record containing float must not type-check in the CatSeq source profile");
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("floating-point values are not supported"))
    );
}

#[test]
fn catseq_source_profile_accepts_int32_ndarray_rank_literals() {
    analyze_catseq_function(indoc! {"
        def compute(value: int) -> int:
            array = np_array([1, 2])
            filled = np_full((2,), 1)
            return value
    "})
    .unwrap_or_else(|error| {
        panic!("type-level ndarray rank literals must not be rejected as uint64 values: {error}")
    });
}

#[test]
fn catseq_source_profile_validates_instantiated_generic_builtins() {
    analyze_catseq_function(indoc! {"
        def compute(value: int) -> int:
            absolute = abs(value)
            minimum = min(value, 1)
            maximum = max(value, 2)
            return absolute + minimum + maximum
    "})
    .unwrap_or_else(|error| {
        panic!("generic builtins instantiated with Int32 must type-check: {error}")
    });
}

#[test]
fn catseq_source_profile_validates_non_builtin_generics_after_instantiation() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let generic = composer.unifier.get_fresh_var_with_range(
        &[composer.primitives_ty.int32, composer.primitives_ty.float],
        Some("T".into()),
        None,
    );
    let identity = composer.unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: vec![crate::typecheck::typedef::FuncArg {
            name: "value".into(),
            ty: generic.ty,
            default_value: None,
            is_vararg: false,
        }],
        ret: generic.ty,
        vars: VarMap::from([(generic.id, generic.ty)]),
    }));
    internal_resolver.add_id_type("identity".into(), identity);
    let ast = parse_program(
        indoc! {"
            def compute(value: int) -> int:
                return identity(value)
        "},
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    composer.start_analysis(true).unwrap_or_else(|errors| {
        panic!(
            "unused generic alternatives must not be treated as runtime values: {}",
            errors.iter().join("\n")
        )
    });
}

#[test]
fn catseq_source_profile_revalidates_values_after_call_unification() {
    let result = analyze_catseq_with_late_bound_value(indoc! {"
        def compute() -> int:
            consume_float(x)
            return 0
    "});
    let Err(error) = result else {
        panic!("a resolver value unified to float must not escape CatSeqInt32 validation")
    };

    assert!(error.contains("floating-point values are not supported by CatSeqInt32"), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test_case(
    indoc! {"
        def compute() -> int:
            consume_pair(x / 1, x)
            return 0
    "},
    "operator `/` is not supported";
    "true_division"
)]
#[test_case(
    indoc! {"
        def compute() -> int:
            consume_int_pair(x // 0, x)
            return 0
    "},
    "divisor for `//` must not be zero";
    "floor_division_by_zero"
)]
fn catseq_source_profile_revalidates_operators_after_type_unification(
    source: &str,
    expected: &str,
) {
    let result = analyze_catseq_with_late_bound_value(source);
    let Err(error) = result else {
        panic!("an Int32 operator selected by late type unification must be revalidated")
    };

    assert!(error.contains(expected), "{error}");
}

#[test]
fn catseq_source_profile_applies_integer_rules_only_to_integer_operators() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let ast = parse_program(
        indoc! {"
            class Number:
                def __init__(self):
                    pass

                def __floordiv__(self, other: int) -> int:
                    return 1

                def __mod__(self, other: int) -> int:
                    return 2

                def __truediv__(self, other: int) -> int:
                    return 3

            def compute(value: Number) -> int:
                floor_result: int = value // 0
                remainder: int = value % 0
                quotient: int = value / 0
                return floor_result + remainder + quotient
        "},
        FileName::default(),
    )
    .unwrap();
    for definition in ast {
        let (name, definition_id, ty) =
            composer.register_top_level(definition, Some(resolver.clone()), "", true).unwrap();
        internal_resolver.add_id_def(name, definition_id);
        if let Some(ty) = ty {
            internal_resolver.add_id_type(name, ty);
        }
    }

    composer.start_analysis(true).unwrap_or_else(|errors| {
        panic!(
            "Int32 operator restrictions must not reject user-defined overloads: {}",
            errors.iter().join("\n")
        )
    });
}

#[test_case("int64"; "int64")]
#[test_case("uint32"; "uint32")]
#[test_case("uint64"; "uint64")]
#[test_case("float"; "float")]
fn catseq_source_profile_rejects_non_int32_numeric_annotations(annotation: &str) {
    let source = format!("def compute(value: {annotation}) -> int:\n    return 0\n");
    let result = analyze_catseq_function(&source);
    let Err(error) = result else {
        panic!("`{annotation}` must not be accepted by the CatSeq source profile")
    };

    assert!(error.contains(&format!("`{annotation}` is not a valid type annotation")), "{error}");
    assert!(error.contains("at unknown:1:"), "{error}");
}

#[test_case("Literal[1.0]", "floating-point values"; "float_literal")]
#[test_case("Literal[2147483648]", "int64 values"; "wide_integer_literal")]
fn catseq_source_profile_rejects_forbidden_literal_annotations(annotation: &str, expected: &str) {
    let source = format!("def compute(value: {annotation}) -> int:\n    return 0\n");
    let result = analyze_catseq_function(&source);
    let Err(error) = result else {
        panic!("`{annotation}` must not be accepted by the CatSeq source profile")
    };

    assert!(error.contains(expected), "{error}");
    assert!(error.contains("at unknown:1:"), "{error}");
}

#[test_case(
    indoc! {"
        def compute(value: int) -> int:
            return value // 0
    "},
    "//";
    "floor_division"
)]
#[test_case(
    indoc! {"
        def compute(value: int) -> int:
            return value % 0
    "},
    "%";
    "modulo"
)]
#[test_case(
    indoc! {"
        def compute(value: int) -> int:
            return value // (1 - 1)
    "},
    "//";
    "constant_arithmetic"
)]
#[test_case(
    indoc! {"
        def compute(value: int) -> int:
            return value % (1 << 32)
    "},
    "%";
    "constant_saturating_shift"
)]
#[test_case(
    indoc! {"
        def compute(value: int) -> int:
            return value // int(0)
    "},
    "//";
    "explicit_int_conversion"
)]
fn catseq_source_profile_rejects_a_proven_zero_divisor(source: &str, operator: &str) {
    let result = analyze_catseq_function(source);
    let Err(error) = result else {
        panic!("a compile-time proven zero divisor for `{operator}` must be rejected")
    };

    assert!(error.contains(&format!("divisor for `{operator}` must not be zero")), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test]
fn catseq_source_profile_rejects_forbidden_class_constants() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let ast = parse_program(
        indoc! {"
            class Constants:
                sample: int = 1.0
        "},
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    let errors = composer
        .start_analysis(true)
        .expect_err("floating-point class constants must not be accepted by CatSeqInt32");
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains("floating-point values are not supported")),
        "{}",
        errors.iter().join("\n")
    );
}

#[test]
fn catseq_source_profile_normalizes_int_spellings_literals_and_range_to_int32() {
    let (mut composer, definition_id) = analyze_catseq_function(indoc! {"
        def compute(value: int, explicit: int32) -> int:
            result = value + explicit + 1
            for offset in range(4):
                result = result + offset
            return result
    "})
    .unwrap_or_else(|error| panic!("the CatSeq Int32 profile must type-check: {error}"));

    let signature = {
        let definition = composer.definition_ast_list[definition_id.0].0.read();
        let TopLevelDef::Function { signature, .. } = &*definition else {
            panic!("registered source must remain a function")
        };
        let signature = *signature;
        drop(definition);
        signature
    };
    let TypeEnum::TFunc(signature) = composer.unifier.get_ty(signature).as_ref().clone() else {
        panic!("registered function must retain its function signature")
    };
    assert_eq!(signature.args.len(), 2);
    assert!(composer.unifier.unioned(signature.args[0].ty, composer.primitives_ty.int32));
    assert!(composer.unifier.unioned(signature.args[1].ty, composer.primitives_ty.int32));
    assert!(composer.unifier.unioned(signature.ret, composer.primitives_ty.int32));
}

#[test]
fn catseq_source_profile_accepts_the_int32_minimum_literal() {
    analyze_catseq_function(indoc! {"
        def compute() -> int:
            return -2147483648
    "})
    .unwrap_or_else(|error| panic!("the Int32 minimum literal must type-check: {error}"));
}

#[test_case(
    indoc! {"
        def compute() -> int:
            total = 0
            for index, value in enumerate([1, 2]):
                total = total + index + value
            return total
    "};
    "enumerate"
)]
#[test_case(
    indoc! {"
        def compute(error: Exception) -> int:
            return 0
    "};
    "exception"
)]
fn catseq_source_profile_ignores_builtin_representation_fields(source: &str) {
    analyze_catseq_function(source).unwrap_or_else(|error| {
        panic!("builtin ABI storage must not be treated as a source numeric value: {error}")
    });
}

#[test]
fn catseq_source_profile_ignores_inherited_exception_representation_fields() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let ast = parse_program(
        indoc! {"
            class ComputeError(Exception):
                pass

            def compute(value: int) -> int:
                if value == 0:
                    raise ComputeError(\"zero\")
                return value
        "},
        FileName::default(),
    )
    .unwrap();
    for definition in ast {
        let (name, definition_id, ty) =
            composer.register_top_level(definition, Some(resolver.clone()), "", true).unwrap();
        internal_resolver.add_id_def(name, definition_id);
        if let Some(ty) = ty {
            internal_resolver.add_id_type(name, ty);
        }
    }

    composer.start_analysis(true).unwrap_or_else(|errors| {
        panic!(
            "inherited exception ABI storage must not be treated as source values: {}",
            errors.iter().join("\n")
        )
    });
}

#[test_case("Auto", false, "floating-point values"; "bare_auto")]
#[test_case("tuple[Auto]", true, "int64 values"; "nested_auto")]
fn catseq_source_profile_rejects_forbidden_resolved_auto_fields(
    annotation: &str,
    nested: bool,
    expected: &str,
) {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let resolved_ty = if nested {
        composer.unifier.add_ty(TypeEnum::TTuple {
            ty: vec![composer.primitives_ty.int64],
            is_vararg_ctx: false,
        })
    } else {
        composer.primitives_ty.float
    };
    internal_resolver.add_auto_field_type("ExternalRecord".into(), "sample".into(), resolved_ty);
    let ast = parse_program(
        &format!("class ExternalRecord:\n    sample: {annotation}\n"),
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    let errors = composer
        .start_analysis(true)
        .expect_err("a forbidden resolved Auto field must not pass CatSeqInt32 validation");
    let error = errors.iter().map(ToString::to_string).join("\n");
    assert!(error.contains(expected), "{error}");
    assert!(error.contains("in field `sample`"), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test]
fn catseq_source_profile_rejects_auto_field_concretized_during_deferred_eval() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    let deferred_ty = composer.unifier.get_dummy_var().ty;
    internal_resolver.add_auto_field_type("ExternalRecord".into(), "sample".into(), deferred_ty);
    internal_resolver
        .deferred_unifications
        .lock()
        .push((deferred_ty, composer.primitives_ty.float));
    let ast = parse_program(
        indoc! {"
            class ExternalRecord:
                sample: Auto
        "},
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    let errors = composer
        .start_analysis(true)
        .expect_err("a field resolved to float during deferred evaluation must be rejected");
    let error = errors.iter().map(ToString::to_string).join("\n");
    assert!(error.contains("floating-point values"), "{error}");
    assert!(error.contains("in field `sample`"), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test]
fn catseq_source_profile_accepts_int32_resolved_auto_fields() {
    let (mut composer, internal_resolver, resolver) = new_catseq_composer();
    internal_resolver.add_auto_field_type(
        "ExternalRecord".into(),
        "sample".into(),
        composer.primitives_ty.int32,
    );
    let ast = parse_program(
        indoc! {"
            class ExternalRecord:
                sample: Auto
        "},
        FileName::default(),
    )
    .unwrap();
    let (name, definition_id, ty) =
        composer.register_top_level(ast[0].clone(), Some(resolver), "", true).unwrap();
    internal_resolver.add_id_def(name, definition_id);
    internal_resolver.add_id_type(name, ty.unwrap());

    composer.start_analysis(true).unwrap_or_else(|errors| {
        panic!("an Auto field resolved to Int32 must remain valid: {}", errors.iter().join("\n"))
    });
}

#[test]
fn catseq_source_profile_retains_target_independent_integer_operations() {
    let (mut composer, definition_id) = analyze_catseq_function(indoc! {"
        def compute(value: int, divisor: int, shift: int) -> int:
            sum = value + 1
            difference = value - 1
            product = value * 2
            floor = value // divisor
            remainder = value % divisor
            negative_floor = -7 // 3
            negative_remainder = 7 % -3
            overflow = -2147483648 // -1
            exact = -2147483648 % -1
            dynamic_left = value << shift
            dynamic_right = value >> shift
            wide_left = value << 32
            wide_right = value >> 32
            negative_left = value << -1
            negative_right = value >> -1
            return sum + difference + product + floor + remainder + negative_floor + negative_remainder + overflow + exact + dynamic_left + dynamic_right + wide_left + wide_right + negative_left + negative_right
    "})
    .unwrap_or_else(|error| panic!("CatSeq integer operations must type-check: {error}"));

    let body = {
        let definition = composer.definition_ast_list[definition_id.0].0.read();
        let TopLevelDef::Function { instance_to_stmt, .. } = &*definition else {
            panic!("registered source must remain a function")
        };
        let instance = instance_to_stmt.values().exactly_one().unwrap();
        let body = Arc::clone(&instance.body);
        drop(definition);
        body
    };
    let operations = body
        .iter()
        .filter_map(|statement| {
            let ast::StmtKind::Assign { value, .. } = &statement.node else { return None };
            let ast::ExprKind::BinOp { op, .. } = &value.node else { return None };
            Some((*op, value.custom.unwrap()))
        })
        .collect_vec();
    let operators = operations.iter().map(|(operator, _)| *operator).collect_vec();

    assert_eq!(
        operators,
        [
            ast::Operator::Add,
            ast::Operator::Sub,
            ast::Operator::Mult,
            ast::Operator::FloorDiv,
            ast::Operator::Mod,
            ast::Operator::FloorDiv,
            ast::Operator::Mod,
            ast::Operator::FloorDiv,
            ast::Operator::Mod,
            ast::Operator::LShift,
            ast::Operator::RShift,
            ast::Operator::LShift,
            ast::Operator::RShift,
            ast::Operator::LShift,
            ast::Operator::RShift,
        ]
    );
    for (_, ty) in operations {
        assert!(composer.unifier.unioned(ty, composer.primitives_ty.int32));
    }
}

#[test_case("2147483648"; "above_maximum")]
#[test_case("-2147483649"; "below_minimum")]
fn catseq_source_profile_rejects_out_of_range_literals(literal: &str) {
    let source = format!("def compute() -> int:\n    return {literal}\n");
    let result = analyze_catseq_function(&source);
    let Err(error) = result else {
        panic!("an integer outside i32 must not type-check in the CatSeq source profile")
    };

    assert!(error.contains("Integer out of bound"), "{error}");
    assert!(error.contains("at unknown:2:"), "{error}");
}

#[test_case(
    &[
        indoc! {"
            class A():
                a: int32
                def __init__(self):
                    self.a = 3
                def fun(self, b: B):
                    pass
                def foo(self, a: T, b: V):
                    pass
        "},
        indoc! {"
            class C(A):
                def __init__(self):
                    pass
                def fun(self, b: B):
                    a = 1
                    pass
        "},
        indoc! {"
            class B(C):
                def __init__(self):
                    pass
        "},
        indoc! {"
            def foo(a: A):
                pass
        "},
        indoc! {"
            def ff(a: T) -> V:
                pass
        "}
    ],
    &[],
    "simple class compose";
    "simple class compose"
)]
#[test_case(
    &[
        indoc! {"
        class B:
            aa: bool
            def __init__(self):
                self.aa = False
            def foo(self, b: T):
                pass
        "},
        indoc! {"
            class Generic_A(Generic[V], B):
                a: int64
                def __init__(self):
                    self.a = 123123123123
                def fun(self, a: int32) -> V:
                    pass
        "}
    ],
    &[],
    "generic class";
    "generic class"
)]
#[test_case(
    &[
        indoc! {"
            def foo(a: list[int32], b: tuple[T, float]) -> A[B, bool]:
                pass
        "},
        indoc! {"
            class A(Generic[T, V]):
                a: T
                b: V
                def __init__(self, v: V):
                    self.a = 1
                    self.b = v
                def fun(self, a: T) -> V:
                    pass
        "},
        indoc! {"
            def gfun(a: A[list[float], int32]):
                pass
        "},
        indoc! {"
            class B:
                def __init__(self):
                    pass
        "}
    ],
    &[],
    "list tuple generic";
    "list tuple generic"
)]
#[test_case(
    &[
        indoc! {"
            class A(Generic[T, V]):
                a: A[float, bool]
                b: B
                def __init__(self, a: A[float, bool], b: B):
                    self.a = a
                    self.b = b
                def fun(self, a: A[float, bool]) -> A[bool, int32]:
                    pass
        "},
        indoc! {"
            class B(A[int64, bool]):
                def __init__(self):
                    pass
                def foo(self, b: B) -> B:
                    pass
                def bar(self, a: A[list[B], int32]) -> tuple[A[virtual[A[B, int32]], bool], B]:
                    pass
        "}
    ],
    &[],
    "self1";
    "self1"
)]
#[test_case(
    &[
        indoc! {"
            class A(Generic[T]):
                a: int32
                b: T
                c: A[int64]
                def __init__(self, t: T):
                    self.a = 3
                    self.b = T
                def fun(self, a: int32, b: T) -> list[virtual[B[bool]]]:
                    pass
                def foo(self, c: C):
                    pass
        "},
        indoc! {"
            class B(Generic[V], A[float]):
                d: C
                def __init__(self):
                    pass
                def fun(self, a: int32, b: T) -> list[virtual[B[bool]]]:
                    # override
                    pass
        "},
        indoc! {"
            class C(B[bool]):
                e: int64
                def __init__(self):
                    pass
        "}
    ],
    &[],
    "inheritance_override";
    "inheritance_override"
)]
#[test_case(
    &[
        indoc! {"
            class A(Generic[T]):
                def __init__(self):
                    pass
                def fun(self, a: A[T]) -> A[T]:
                    pass
        "}
    ],
    &[],
    "type var in generic app";
    "type_var_in_generic_app"
)]
#[test_case(
    &[
        indoc! {"
            class A(B):
                def __init__(self):
                    pass
        "},
        indoc! {"
            class B(A):
                def __init__(self):
                    pass
        "}
    ],
    &["NameError: name 'B' is not defined (at unknown:1:9)"],
    "cyclic1";
    "cyclic1"
)]
#[test_case(
    &[
        indoc! {"
        class B(Generic[V, T], C[int32]):
            def __init__(self):
                pass
        "},
        indoc! {"
            class A(B[bool, int64]):
                def __init__(self):
                    pass
        "},
        indoc! {"
            class C(Generic[T], A):
                def __init__(self):
                    pass
        "},
    ],
    &["NameError: name 'C' is not defined (at unknown:1:25)"],
    "cyclic2";
    "cyclic2"
)]
#[test_case(
    &[
        indoc! {"
            class A:
                pass
        "}
    ],
    &["5: Class {\nname: \"A\",\ndef_id: DefinitionId(5),\nancestors: [CustomClassKind { id: DefinitionId(5), params: [] }],\nfields: [],\nmethods: [],\ntype_vars: []\n}"],
    "simple pass in class";
    "simple pass in class"
)]
#[test_case(
    &[indoc! {"
        class A:
            def __init__():
                pass
    "}],
    &["__init__ method must have a `self` parameter (at unknown:2:5)"],
    "err no self_1";
    "err no self_1"
)]
#[test_case(
    &[
        indoc! {"
            class B:
                def __init__(self):
                    pass
        "},
        indoc! {"
            class C:
                def __init__(self):
                    pass
        "},
        indoc! {"
        class A(B, Generic[T], C):
            def __init__(self):
                pass
        "}

    ],
    &["a class definition can only have at most one base class declaration and one generic declaration (at unknown:1:24)"],
    "err multiple inheritance";
    "err multiple inheritance"
)]
#[test_case(
    &[
        indoc! {"
            class A(Generic[T]):
                a: int32
                b: T
                c: A[int64]
                def __init__(self, t: T):
                    self.a = 3
                    self.b = T
                def fun(self, a: int32, b: T) -> list[virtual[B[bool]]]:
                    pass
        "},
        indoc! {"
            class B(Generic[V], A[float]):
                def __init__(self):
                    pass
                def fun(self, a: int32, b: T) -> list[virtual[B[int32]]]:
                    # override
                    pass
        "}
    ],
    &["method fun has same name as ancestors' method, but incompatible type"],
    "err_incompatible_inheritance_method";
    "err_incompatible_inheritance_method"
)]
#[test_case(
    &[
        indoc! {"
            class A(Generic[T]):
                a: int32
                b: T
                c: A[int64]
                def __init__(self, t: T):
                    self.a = 3
                    self.b = T
                def fun(self, a: int32, b: T) -> list[virtual[B[bool]]]:
                    pass
        "},
        indoc! {"
            class B(Generic[V], A[float]):
                a: int32
                def __init__(self):
                    pass
                def fun(self, a: int32, b: T) -> list[virtual[B[bool]]]:
                    # override
                    pass
        "}
    ],
    &["field `a` has already declared in the ancestor classes"],
    "err_incompatible_inheritance_field";
    "err_incompatible_inheritance_field"
)]
#[test_case(
    &[
        indoc! {"
            class A:
                def __init__(self):
                    pass
        "},
        indoc! {"
            class A:
                a: int32
                def __init__(self):
                    pass
        "}
    ],
    &["duplicate definition of class `A` (at unknown:1:1)"],
    "class same name";
    "class same name"
)]
// case_name param is required for insta to distinguish different test_case
// See https://github.com/frondeus/test-case/issues/37
fn test_analyze(source: &[&str], res: &[&str], case_name: &str) {
    let print = false;
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);
    let mut composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry, 64).0;

    let internal_resolver = make_internal_resolver_with_tvar(
        vec![
            ("T".into(), vec![]),
            ("V".into(), vec![composer.primitives_ty.bool, composer.primitives_ty.int32]),
            ("G".into(), vec![composer.primitives_ty.bool, composer.primitives_ty.int64]),
        ],
        &mut composer.unifier,
        print,
    );
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;

    for s in source {
        let ast = parse_program(s, FileName::default()).unwrap();
        let ast = ast[0].clone();

        let (id, def_id, ty) = {
            match composer.register_top_level(ast, Some(resolver.clone()), "", false) {
                Ok(x) => x,
                Err(msg) => {
                    if print {
                        println!("{msg}");
                    } else {
                        assert_eq!(res[0], msg);
                    }
                    return;
                }
            }
        };
        internal_resolver.add_id_def(id, def_id);
        if let Some(ty) = ty {
            internal_resolver.add_id_type(id, ty);
        }
    }

    if let Err(msg) = composer.start_analysis(false) {
        if print {
            println!(
                "{}",
                msg.iter()
                    .sorted_by(|lhs, rhs| Ord::cmp(&lhs.to_string(), &rhs.to_string()))
                    .join("\n----------\n")
            );
        } else {
            assert_eq!(res[0], msg.first().unwrap().to_string());
        }
    } else {
        // skip 5 to skip primitives
        let mut res_vec: Vec<String> = Vec::new();
        for (def, _) in composer.definition_ast_list.iter().skip(composer.builtin_num) {
            let def = &*def.read();
            res_vec.push(format!("{}\n", def.to_string(&mut composer.unifier)));
        }
        insta::with_settings!({ snapshot_suffix => case_name.replace(' ', "_") }, {
            insta::assert_debug_snapshot!(res_vec);
        });
    }
}

#[test_case(
    vec![
        indoc! {"
            def fun(a: int32, b: int32) -> int32:
                return a + b
        "},
        indoc! {"
            def fib(n: int32) -> int32:
                if n <= 2:
                    return 1
                a = fib(n - 1)
                b = fib(n - 2)
                return fib(n - 1)
        "}
    ],
    &[];
    "simple function"
)]
#[test_case(
    vec![
        indoc! {"
            class A:
                a: int32
                def __init__(self):
                    self.a = 3
                def fun(self) -> int32:
                    b = self.a + 3
                    return b * self.a
                def clone(self) -> A:
                    SELF = self
                    return SELF
                def sum(self) -> int32:
                    if self.a == 0:
                        return self.a
                    else:
                        a = self.a
                        self.a = self.a - 1
                        return a + self.sum()
                def fib(self, a: int32) -> int32:
                    if a <= 2:
                        return 1
                    return self.fib(a - 1) + self.fib(a - 2)
        "},
        indoc! {"
            def fun(a: A) -> int32:
                return a.fun() + 2
        "}
    ],
    &[];
    "simple class body"
)]
#[test_case(
    vec![
        indoc! {"
            def fun(a: V, c: G, t: T) -> V:
                b = a
                cc = c
                ret = fun(b, cc, t)
                return ret * ret
        "},
        indoc! {"
            def sum_three(l: list[V]) -> V:
                return l[0] + l[1] + l[2]
        "},
        indoc! {"
            def sum_sq_pair(p: tuple[V, V]) -> list[V]:
                a = p[0]
                b = p[1]
                a = a**a
                b = b**b
                return [a, b]
        "}
    ],
    &[];
    "type var fun"
)]
#[test_case(
    vec![
        indoc! {"
            class A(Generic[G]):
                a: G
                b: bool
                def __init__(self, aa: G):
                    self.a = aa
                    if 2 > 1:
                        self.b = True
                    else:
                        # self.b = False
                        pass
                def fun(self, a: G) -> list[G]:
                    ret = [a, self.a]
                    return ret if self.b else self.fun(self.a)
        "}
    ],
    &[];
    "type var class"
)]
#[test_case(
    vec![
        indoc! {"
            class A:
                def fun(self):
                    pass
        "},
        indoc!{"
            class B:
                a: int32
                b: bool
                def __init__(self):
                    # self.b = False
                    if 3 > 2:
                        self.a = 3
                        self.b = False
                    else:
                        self.a = 4
                        self.b = True
        "}
    ],
    &[];
    "no_init_inst_check"
)]
fn test_inference(source: Vec<&str>, res: &[&str]) {
    let print = true;
    let builtin_registry = Arc::new(DefaultBuiltinRegistry);
    let mut composer = TopLevelComposer::new(Vec::new(), Vec::new(), builtin_registry, 64).0;

    let internal_resolver = make_internal_resolver_with_tvar(
        vec![
            ("T".into(), vec![]),
            (
                "V".into(),
                vec![
                    composer.primitives_ty.float,
                    composer.primitives_ty.int32,
                    composer.primitives_ty.int64,
                ],
            ),
            ("G".into(), vec![composer.primitives_ty.bool, composer.primitives_ty.int64]),
        ],
        &mut composer.unifier,
        print,
    );
    let resolver =
        Arc::new(Resolver(internal_resolver.clone())) as Arc<dyn SymbolResolver + Send + Sync>;

    for s in source {
        let ast = parse_program(s, FileName::default()).unwrap();
        let ast = ast[0].clone();

        let (id, def_id, ty) = {
            match composer.register_top_level(ast, Some(resolver.clone()), "", false) {
                Ok(x) => x,
                Err(msg) => {
                    if print {
                        println!("{msg}");
                    } else {
                        assert_eq!(res[0], msg);
                    }
                    return;
                }
            }
        };
        internal_resolver.add_id_def(id, def_id);
        if let Some(ty) = ty {
            internal_resolver.add_id_type(id, ty);
        }
    }

    if let Err(msg) = composer.start_analysis(true) {
        if print {
            println!(
                "{}",
                msg.iter()
                    .sorted_by(|lhs, rhs| Ord::cmp(&lhs.to_string(), &rhs.to_string()))
                    .join("\n----------\n")
            );
        } else {
            assert_eq!(res[0], msg.first().unwrap().to_string());
        }
    } else {
        // skip 5 to skip primitives
        let mut stringify_folder = TypeToStringFolder { unifier: &mut composer.unifier };
        for (def, _) in composer.definition_ast_list.iter().skip(composer.builtin_num) {
            let def = &*def.read();

            if let TopLevelDef::Function { instance_to_stmt, name, .. } = def {
                println!(
                    "=========`{}`: number of instances: {}===========",
                    name,
                    instance_to_stmt.len()
                );
                for inst in instance_to_stmt {
                    let ast = &inst.1.body;
                    for b in ast.iter() {
                        println!("{:?}", stringify_folder.fold_stmt(b.clone()).unwrap());
                        println!("--------------------");
                    }
                    println!("\n");
                }
            }
        }
    }
}

fn make_internal_resolver_with_tvar(
    tvars: Vec<(StrRef, Vec<Type>)>,
    unifier: &mut Unifier,
    print: bool,
) -> Arc<ResolverInternal> {
    let res: Arc<ResolverInternal> = ResolverInternal {
        id_to_def: Mutex::new(HashMap::from([("list".into(), PrimDef::List.id())])),
        id_to_type: tvars
            .into_iter()
            .map(|(name, range)| {
                (name, {
                    let tvar = unifier.get_fresh_var_with_range(range.as_slice(), None, None);
                    if print {
                        println!("{}: {:?}, typevar{}", name, tvar.ty, tvar.id);
                    }
                    tvar.ty
                })
            })
            .collect::<HashMap<_, _>>()
            .into(),
        auto_field_types: Mutex::default(),
        deferred_unifications: Mutex::default(),
    }
    .into();
    if print {
        println!();
    }
    res
}

struct TypeToStringFolder<'a> {
    unifier: &'a mut Unifier,
}

impl Fold<Option<Type>> for TypeToStringFolder<'_> {
    type TargetU = String;
    type Error = String;
    fn map_user(&mut self, user: Option<Type>) -> Result<Self::TargetU, Self::Error> {
        Ok(if let Some(ty) = user {
            self.unifier.internal_stringify(
                ty,
                &mut |id| format!("class{id}"),
                &mut |id| format!("typevar{id}"),
                &mut None,
            )
        } else {
            "None".into()
        })
    }
}
