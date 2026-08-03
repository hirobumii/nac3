use std::iter::zip;

use anyhow::anyhow;
use indexmap::IndexMap;
use indoc::indoc;
use nac3parser::{ast::FileName, parser::parse_program};
use parking_lot::RwLock;
use test_case::test_case;

use super::*;
use crate::{
    codegen::CodeGenContext,
    symbol_resolver::ValueEnum,
    toplevel::{
        DefinitionId, TopLevelDef,
        composer::{DefaultBuiltinRegistry, TopLevelComposer},
        helper::PrimDef,
    },
    typecheck::{magic_methods::with_fields, typedef::AttrKind},
};

struct Resolver {
    id_to_type: HashMap<StrRef, Type>,
    id_to_def: HashMap<StrRef, DefinitionId>,
}

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
        self.id_to_def.get(&id).copied().ok_or_else(|| vec![anyhow!("Unknown identifier")])
    }

    fn get_string_id(&self, _: &str) -> i32 {
        unimplemented!()
    }

    fn get_exception_id(&self, _tyid: usize) -> usize {
        unimplemented!()
    }
}

struct TestEnvironment {
    pub unifier: Unifier,
    pub function_data: FunctionData,
    pub primitives: PrimitiveStore,
    pub id_to_name: HashMap<usize, StrRef>,
    pub identifier_mapping: HashMap<StrRef, Type>,
    pub virtual_checks: Vec<(Type, Type, Location)>,
    pub calls: HashMap<CodeLocation, CallId>,
    pub top_level_defs: Vec<Arc<RwLock<TopLevelDef>>>,
}

impl TestEnvironment {
    fn new() -> Self {
        let mut identifier_mapping = HashMap::new();
        let (primitives, mut unifier) = TopLevelComposer::make_unifier(64);
        let mut top_level_defs: Vec<Arc<RwLock<TopLevelDef>>> = Vec::new();
        with_fields(&mut unifier, primitives.int32, |unifier, fields| {
            let add_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "other".into(),
                    ty: primitives.int32,
                    default_value: None,
                    is_vararg: false,
                }],
                ret: primitives.int32,
                vars: VarMap::new(),
            }));
            fields.insert("__add__".into(), (add_ty, AttrKind::Method));
        });
        identifier_mapping.insert("None".into(), primitives.none);
        let primitive_names = [
            "int32",
            "int64",
            "float",
            "bool",
            "none",
            "range",
            "enumerate",
            "str",
            "Exception",
            "uint32",
            "uint64",
            "Option",
            "list",
            "ndarray",
        ];
        for (i, name) in primitive_names.iter().enumerate() {
            top_level_defs.push(
                RwLock::new(TopLevelDef::Class {
                    name: (*name).into(),
                    simple_name: (*name).to_string(),
                    object_id: DefinitionId(i),
                    type_vars: Vec::default(),
                    fields: Vec::default(),
                    attributes: Vec::default(),
                    methods: Vec::default(),
                    ancestors: Vec::default(),
                    resolver: None,
                    constructor: None,
                    loc: None,
                })
                .into(),
            );
        }
        let defs = primitive_names.len();

        let tvar = unifier.get_dummy_var();

        let foo_ty = unifier.add_ty(TypeEnum::TObj {
            obj_id: DefinitionId(defs),
            fields: [("a".into(), (tvar.ty, AttrKind::Field { mutable: true }))].into(),
            params: into_var_map([tvar]),
        });
        top_level_defs.push(
            RwLock::new(TopLevelDef::Class {
                name: "Foo".into(),
                simple_name: "Foo".to_string(),
                object_id: DefinitionId(defs),
                type_vars: vec![tvar.ty],
                fields: [("a".into(), tvar.ty, true)].into(),
                attributes: Vec::default(),
                methods: Vec::default(),
                ancestors: Vec::default(),
                resolver: None,
                constructor: None,
                loc: None,
            })
            .into(),
        );

        identifier_mapping.insert(
            "Foo".into(),
            unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![],
                ret: foo_ty,
                vars: into_var_map([tvar]),
            })),
        );

        let fun = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: primitives.int32,
            vars: IndexMap::default(),
        }));
        let bar = unifier.add_ty(TypeEnum::TObj {
            obj_id: DefinitionId(defs + 1),
            fields: [
                ("a".into(), (primitives.int32, AttrKind::Field { mutable: true })),
                ("b".into(), (fun, AttrKind::Method)),
            ]
            .into(),
            params: IndexMap::default(),
        });
        top_level_defs.push(
            RwLock::new(TopLevelDef::Class {
                name: "Bar".into(),
                simple_name: "Bar".to_string(),
                object_id: DefinitionId(defs + 1),
                type_vars: Vec::default(),
                fields: [("a".into(), primitives.int32, true), ("b".into(), fun, true)].into(),
                attributes: Vec::default(),
                methods: Vec::default(),
                ancestors: Vec::default(),
                resolver: None,
                constructor: None,
                loc: None,
            })
            .into(),
        );
        identifier_mapping.insert(
            "Bar".into(),
            unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![],
                ret: bar,
                vars: IndexMap::default(),
            })),
        );

        let bar2 = unifier.add_ty(TypeEnum::TObj {
            obj_id: DefinitionId(defs + 2),
            fields: [
                ("a".into(), (primitives.bool, AttrKind::Field { mutable: true })),
                ("b".into(), (fun, AttrKind::Method)),
            ]
            .into(),
            params: IndexMap::default(),
        });
        top_level_defs.push(
            RwLock::new(TopLevelDef::Class {
                name: "Bar2".into(),
                simple_name: "Bar2".to_string(),
                object_id: DefinitionId(defs + 2),
                type_vars: Vec::default(),
                fields: [("a".into(), primitives.bool, true), ("b".into(), fun, false)].into(),
                attributes: Vec::default(),
                methods: Vec::default(),
                ancestors: Vec::default(),
                resolver: None,
                constructor: None,
                loc: None,
            })
            .into(),
        );
        identifier_mapping.insert(
            "Bar2".into(),
            unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![],
                ret: bar2,
                vars: IndexMap::default(),
            })),
        );

        let id_to_name: HashMap<_, _> = [
            (PrimDef::Int32.id().0, "int32".into()),
            (PrimDef::Int64.id().0, "int64".into()),
            (PrimDef::Float.id().0, "float".into()),
            (PrimDef::Bool.id().0, "bool".into()),
            (PrimDef::None.id().0, "none".into()),
            (PrimDef::Range.id().0, "range".into()),
            (PrimDef::Enumerate.id().0, "enumerate".into()),
            (PrimDef::Str.id().0, "str".into()),
            (PrimDef::Exception.id().0, "exception".into()),
            (PrimDef::UInt32.id().0, "uint32".into()),
            (PrimDef::UInt64.id().0, "uint64".into()),
            (PrimDef::Option.id().0, "option".into()),
            (PrimDef::List.id().0, "list".into()),
            (PrimDef::NDArray.id().0, "ndarray".into()),
            (defs, "Foo".into()),
            (defs + 1, "Bar".into()),
            (defs + 2, "Bar2".into()),
        ]
        .into();

        let resolver = Arc::new(Resolver {
            id_to_type: identifier_mapping.clone(),
            id_to_def: [
                ("Foo".into(), DefinitionId(defs)),
                ("Bar".into(), DefinitionId(defs + 1)),
                ("Bar2".into(), DefinitionId(defs + 2)),
            ]
            .into(),
        }) as Arc<dyn SymbolResolver + Send + Sync>;

        Self {
            top_level_defs,
            unifier,
            function_data: FunctionData {
                resolver,
                bound_variables: Vec::new(),
                return_type: None,
            },
            primitives,
            id_to_name,
            identifier_mapping,
            virtual_checks: Vec::new(),
            calls: HashMap::new(),
        }
    }

    fn get_inferencer(&mut self) -> Inferencer<'_> {
        Inferencer {
            top_level_defs: &self.top_level_defs,
            builtin_registry: &DefaultBuiltinRegistry,
            function_data: &mut self.function_data,
            unifier: &mut self.unifier,
            variable_mapping: HashMap::default(),
            primitives: &mut self.primitives,
            virtual_checks: &mut self.virtual_checks,
            calls: &mut self.calls,
            defined_identifiers: HashSet::default(),
            in_handler: false,
        }
    }
}

#[test_case(indoc! {"
        a = 1234
        b = int64(2147483648)
        c = 1.234
        d = True
    "},
    &[("a", "int32"), ("b", "int64"), ("c", "float"), ("d", "bool")].into(),
    &[]
    ; "primitives test")]
#[test_case(indoc! {"
        a = lambda x, y: x
        b = lambda x: a(x, x)
        c = 1.234
        d = b(c)
    "},
    &[("a", "fn[[x:float, y:float], float]"), ("b", "fn[[x:float], float]"), ("c", "float"), ("d", "float")].into(),
    &[]
    ; "lambda test")]
#[test_case(indoc! {"
        a = lambda x: x + x
        b = lambda x: a(x) + x
        a = b
        c = b(1)
    "},
    &[("a", "fn[[x:int32], int32]"), ("b", "fn[[x:int32], int32]"), ("c", "int32")].into(),
    &[]
    ; "lambda test 2")]
#[test_case(indoc! {"
        a = lambda x: x
        b = lambda x: x

        foo1 = Foo()
        foo2 = Foo()
        c = a(foo1.a)
        d = b(foo2.a)

        a(True)
        b(123)

    "},
    &[("a", "fn[[x:bool], bool]"), ("b", "fn[[x:int32], int32]"), ("c", "bool"),
     ("d", "int32"), ("foo1", "Foo[bool]"), ("foo2", "Foo[int32]")].into(),
    &[]
    ; "obj test")]
#[test_case(indoc! {"
        a = [1, 2, 3]
        b = [x + x for x in a]
    "},
    &[("a", "list[int32]"), ("b", "list[int32]")].into(),
    &[]
    ; "listcomp test")]
#[test_case(indoc! {"
        a = virtual(Bar(), Bar)
        b = a.b()
        a = virtual(Bar2())
    "},
    &[("a", "virtual[Bar]"), ("b", "int32")].into(),
    &[("Bar", "Bar"), ("Bar2", "Bar")]
    ; "virtual test")]
#[test_case(indoc! {"
        a = [virtual(Bar(), Bar), virtual(Bar2())]
        b = [x.b() for x in a]
    "},
    &[("a", "list[virtual[Bar]]"), ("b", "list[int32]")].into(),
    &[("Bar", "Bar"), ("Bar2", "Bar")]
    ; "virtual list test")]
fn test_basic(source: &str, mapping: &HashMap<&str, &str>, virtuals: &[(&str, &str)]) {
    println!("source:\n{source}");
    let mut env = TestEnvironment::new();
    let id_to_name = std::mem::take(&mut env.id_to_name);
    let mut defined_identifiers: HashSet<_, _> = env.identifier_mapping.keys().copied().collect();
    defined_identifiers.insert("virtual".into());
    let mut inferencer = env.get_inferencer();
    inferencer.defined_identifiers.clone_from(&defined_identifiers);
    let statements = parse_program(source, FileName::default()).unwrap();
    let statements = statements
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    inferencer.check_block(&statements, &mut defined_identifiers).unwrap();

    for (k, v) in &inferencer.variable_mapping {
        let name = inferencer.unifier.internal_stringify(
            *v,
            &mut |v| id_to_name[&v].into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );
        println!("{k}: {name}");
    }
    for (k, v) in mapping {
        let ty = inferencer.variable_mapping[&(*k).into()];
        let name = inferencer.unifier.internal_stringify(
            ty,
            &mut |v| id_to_name[&v].into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );
        assert_eq!(format!("{k}: {v}"), format!("{k}: {name}"));
    }
    assert_eq!(inferencer.virtual_checks.len(), virtuals.len());
    for ((a, b, _), (x, y)) in zip(inferencer.virtual_checks.iter(), virtuals) {
        let a = inferencer.unifier.internal_stringify(
            *a,
            &mut |v| id_to_name[&v].into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );
        let b = inferencer.unifier.internal_stringify(
            *b,
            &mut |v| id_to_name[&v].into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );

        assert_eq!(&a, x);
        assert_eq!(&b, y);
    }
}

#[test_case(indoc! {"
        a = 2
        b = 2
        c = a + b
        d = a - b
        e = a * b
        f = a / b
        g = a // b
        h = a % b
    "},
    &[("a", "int32"),
    ("b", "int32"),
    ("c", "int32"),
    ("d", "int32"),
    ("e", "int32"),
    ("f", "float"),
    ("g", "int32"),
    ("h", "int32")].into()
    ; "int32")]
#[test_case(
    indoc! {"
        a = 2.4
        b = 3.6
        c = a + b
        d = a - b
        e = a * b
        f = a / b
        g = a // b
        h = a % b
        i = a ** b
        ii = 3
        j = a ** b
    "},
    &[("a", "float"),
    ("b", "float"),
    ("c", "float"),
    ("d", "float"),
    ("e", "float"),
    ("f", "float"),
    ("g", "float"),
    ("h", "float"),
    ("i", "float"),
    ("ii", "int32"),
    ("j", "float")].into()
    ; "float"
)]
#[test_case(
    indoc! {"
        a = int64(12312312312)
        b = int64(24242424424)
        c = a + b
        d = a - b
        e = a * b
        f = a / b
        g = a // b
        h = a % b
        i = a == b
        j = a > b
        k = a < b
        l = a != b
    "},
    &[("a", "int64"),
    ("b", "int64"),
    ("c", "int64"),
    ("d", "int64"),
    ("e", "int64"),
    ("f", "float"),
    ("g", "int64"),
    ("h", "int64"),
    ("i", "bool"),
    ("j", "bool"),
    ("k", "bool"),
    ("l", "bool")].into()
    ; "int64"
)]
#[test_case(
    indoc! {"
        a = True
        b = False
        c = a == b
        d = not a
        e = a != b
    "},
    &[("a", "bool"),
    ("b", "bool"),
    ("c", "bool"),
    ("d", "bool"),
    ("e", "bool")].into()
    ; "boolean"
)]
fn test_primitive_magic_methods(source: &str, mapping: &HashMap<&str, &str>) {
    println!("source:\n{source}");
    let mut env = TestEnvironment::new();
    let id_to_name = std::mem::take(&mut env.id_to_name);
    let mut defined_identifiers: HashSet<_, _> = env.identifier_mapping.keys().copied().collect();
    defined_identifiers.insert("virtual".into());
    let mut inferencer = env.get_inferencer();
    inferencer.defined_identifiers.clone_from(&defined_identifiers);
    let statements = parse_program(source, FileName::default()).unwrap();
    let statements = statements
        .into_iter()
        .map(|v| inferencer.fold_stmt(v))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    inferencer.check_block(&statements, &mut defined_identifiers).unwrap();

    for (k, v) in &inferencer.variable_mapping {
        let name = inferencer.unifier.internal_stringify(
            *v,
            &mut |v| (id_to_name[&v]).into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );
        println!("{k}: {name}");
    }
    for (k, v) in mapping {
        let ty = inferencer.variable_mapping[&(*k).into()];
        let name = inferencer.unifier.internal_stringify(
            ty,
            &mut |v| (id_to_name[&v]).into(),
            &mut |v| format!("v{v}"),
            &mut None,
        );
        assert_eq!(format!("{k}: {v}"), format!("{k}: {name}"));
    }
}
