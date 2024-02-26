use std::convert::TryInto;

use crate::symbol_resolver::SymbolValue;
use nac3parser::ast::{Constant, Location};

use super::*;

/// Structure storing [`DefinitionId`] for primitive types.
#[derive(Clone, Copy)]
pub struct PrimitiveDefinitionIds {
    pub int32: DefinitionId,
    pub int64: DefinitionId,
    pub uint32: DefinitionId,
    pub uint64: DefinitionId,
    pub float: DefinitionId,
    pub bool: DefinitionId,
    pub none: DefinitionId,
    pub range: DefinitionId,
    pub str: DefinitionId,
    pub exception: DefinitionId,
    pub option: DefinitionId,
    pub ndarray: DefinitionId,
}

impl PrimitiveDefinitionIds {
    /// Returns all [`DefinitionId`] of primitives as a [`Vec`].
    ///
    /// There are no guarantees on ordering of the IDs.
    #[must_use]
    fn as_vec(&self) -> Vec<DefinitionId> {
        vec![
            self.int32,
            self.int64,
            self.uint32,
            self.uint64,
            self.float,
            self.bool,
            self.none,
            self.range,
            self.str,
            self.exception,
            self.option,
            self.ndarray,
        ]
    }

    /// Returns the primitive with the largest [`DefinitionId`].
    #[must_use]
    pub fn max_id(&self) -> DefinitionId {
        self.as_vec().into_iter().max().unwrap()
    }
}

/// The [definition IDs][DefinitionId] for primitive types.
pub const PRIMITIVE_DEF_IDS: PrimitiveDefinitionIds = PrimitiveDefinitionIds {
    int32: DefinitionId(0),
    int64: DefinitionId(1),
    uint32: DefinitionId(8),
    uint64: DefinitionId(9),
    float: DefinitionId(2),
    bool: DefinitionId(3),
    none: DefinitionId(4),
    range: DefinitionId(5),
    str: DefinitionId(6),
    exception: DefinitionId(7),
    option: DefinitionId(10),
    ndarray: DefinitionId(14),
};

impl TopLevelDef {
    pub fn to_string(&self, unifier: &mut Unifier) -> String {
        match self {
            TopLevelDef::Class { name, ancestors, fields, methods, type_vars, .. } => {
                let fields_str = fields
                    .iter()
                    .map(|(n, ty, _)| (n.to_string(), unifier.stringify(*ty)))
                    .collect_vec();

                let methods_str = methods
                    .iter()
                    .map(|(n, ty, id)| (n.to_string(), unifier.stringify(*ty), *id))
                    .collect_vec();
                format!(
                    "Class {{\nname: {:?},\nancestors: {:?},\nfields: {:?},\nmethods: {:?},\ntype_vars: {:?}\n}}",
                    name,
                    ancestors.iter().map(|ancestor| ancestor.stringify(unifier)).collect_vec(),
                    fields_str.iter().map(|(a, _)| a).collect_vec(),
                    methods_str.iter().map(|(a, b, _)| (a, b)).collect_vec(),
                    type_vars.iter().map(|id| unifier.stringify(*id)).collect_vec(),
                )
            }
            TopLevelDef::Function { name, signature, var_id, .. } => format!(
                "Function {{\nname: {:?},\nsig: {:?},\nvar_id: {:?}\n}}",
                name,
                unifier.stringify(*signature),
                {
                    // preserve the order for debug output and test
                    let mut r = var_id.clone();
                    r.sort_unstable();
                    r
                }
            ),
        }
    }
}

impl TopLevelComposer {
    #[must_use]
    pub fn make_primitives(size_t: u32) -> (PrimitiveStore, Unifier) {
        let mut unifier = Unifier::new();
        let int32 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.int32,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let int64 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.int64,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let float = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.float,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let bool = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.bool,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let none = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.none,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let range = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.range,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let str = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.str,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let exception = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.exception,
            fields: vec![
                ("__name__".into(), (int32, true)),
                ("__file__".into(), (str, true)),
                ("__line__".into(), (int32, true)),
                ("__col__".into(), (int32, true)),
                ("__func__".into(), (str, true)),
                ("__message__".into(), (str, true)),
                ("__param0__".into(), (int64, true)),
                ("__param1__".into(), (int64, true)),
                ("__param2__".into(), (int64, true)),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            params: HashMap::new(),
        });
        let uint32 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.uint32,
            fields: HashMap::new(),
            params: HashMap::new(),
        });
        let uint64 = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.uint64,
            fields: HashMap::new(),
            params: HashMap::new(),
        });

        let option_type_var = unifier.get_fresh_var(Some("option_type_var".into()), None);
        let is_some_type_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: bool,
            vars: HashMap::from([(option_type_var.1, option_type_var.0)]),
        }));
        let unwrap_fun_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![],
            ret: option_type_var.0,
            vars: HashMap::from([(option_type_var.1, option_type_var.0)]),
        }));
        let option = unifier.add_ty(TypeEnum::TObj {
            obj_id: PRIMITIVE_DEF_IDS.option,
            fields: vec![
                ("is_some".into(), (is_some_type_fun_ty, true)),
                ("is_none".into(), (is_some_type_fun_ty, true)),
                ("unwrap".into(), (unwrap_fun_ty, true)),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            params: HashMap::from([(option_type_var.1, option_type_var.0)]),
        });

        let primitives = PrimitiveStore {
            int32,
            int64,
            uint32,
            uint64,
            float,
            bool,
            none,
            range,
            str,
            exception,
            option,
            size_t,
        };
        unifier.put_primitive_store(&primitives);
        crate::typecheck::magic_methods::set_primitives_magic_methods(&primitives, &mut unifier);
        (primitives, unifier)
    }

    /// already include the `definition_id` of itself inside the ancestors vector
    /// when first registering, the `type_vars`, fields, methods, ancestors are invalid
    #[must_use]
    pub fn make_top_level_class_def(
        obj_id: DefinitionId,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        name: StrRef,
        constructor: Option<Type>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Class {
            name,
            object_id: obj_id,
            type_vars: Vec::default(),
            fields: Vec::default(),
            methods: Vec::default(),
            ancestors: Vec::default(),
            constructor,
            resolver,
            loc,
        }
    }

    /// when first registering, the type is a invalid value
    #[must_use]
    pub fn make_top_level_function_def(
        name: String,
        simple_name: StrRef,
        ty: Type,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        loc: Option<Location>,
    ) -> TopLevelDef {
        TopLevelDef::Function {
            name,
            simple_name,
            signature: ty,
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver,
            codegen_callback: None,
            loc,
        }
    }

    #[must_use]
    pub fn make_class_method_name(mut class_name: String, method_name: &str) -> String {
        class_name.push('.');
        class_name.push_str(method_name);
        class_name
    }

    pub fn get_class_method_def_info(
        class_methods_def: &[(StrRef, Type, DefinitionId)],
        method_name: StrRef,
    ) -> Result<(Type, DefinitionId), HashSet<String>> {
        for (name, ty, def_id) in class_methods_def {
            if name == &method_name {
                return Ok((*ty, *def_id));
            }
        }
        Err(HashSet::from([format!("no method {method_name} in the current class")]))
    }

    /// get all base class def id of a class, excluding itself. \
    /// this function should called only after the direct parent is set
    /// and before all the ancestors are set
    /// and when we allow single inheritance \
    /// the order of the returned list is from the child to the deepest ancestor
    pub fn get_all_ancestors_helper(
        child: &TypeAnnotation,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
    ) -> Result<Vec<TypeAnnotation>, HashSet<String>> {
        let mut result: Vec<TypeAnnotation> = Vec::new();
        let mut parent = Self::get_parent(child, temp_def_list);
        while let Some(p) = parent {
            parent = Self::get_parent(&p, temp_def_list);
            let p_id = if let TypeAnnotation::CustomClass { id, .. } = &p {
                *id
            } else {
                unreachable!("must be class kind annotation")
            };
            // check cycle
            let no_cycle = result.iter().all(|x| {
                let TypeAnnotation::CustomClass { id, .. } = x else {
                    unreachable!("must be class kind annotation")
                };

                id.0 != p_id.0
            });
            if no_cycle {
                result.push(p);
            } else {
                return Err(HashSet::from(["cyclic inheritance detected".into()]));
            }
        }
        Ok(result)
    }

    /// should only be called when finding all ancestors, so panic when wrong
    fn get_parent(
        child: &TypeAnnotation,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
    ) -> Option<TypeAnnotation> {
        let child_id = if let TypeAnnotation::CustomClass { id, .. } = child {
            *id
        } else {
            unreachable!("should be class type annotation")
        };
        let child_def = temp_def_list.get(child_id.0).unwrap();
        let child_def = child_def.read();
        let TopLevelDef::Class { ancestors, .. } = &*child_def else {
            unreachable!("child must be top level class def")
        };

        if ancestors.is_empty() {
            None
        } else {
            Some(ancestors[0].clone())
        }
    }

    /// get the `var_id` of a given `TVar` type
    pub fn get_var_id(var_ty: Type, unifier: &mut Unifier) -> Result<u32, HashSet<String>> {
        if let TypeEnum::TVar { id, .. } = unifier.get_ty(var_ty).as_ref() {
            Ok(*id)
        } else {
            Err(HashSet::from([
                "not type var".to_string(),
            ]))
        }
    }

    pub fn check_overload_function_type(
        this: Type,
        other: Type,
        unifier: &mut Unifier,
        type_var_to_concrete_def: &HashMap<Type, TypeAnnotation>,
    ) -> bool {
        let this = unifier.get_ty(this);
        let this = this.as_ref();
        let other = unifier.get_ty(other);
        let other = other.as_ref();
        let (
            TypeEnum::TFunc(FunSignature { args: this_args, ret: this_ret, .. }),
            TypeEnum::TFunc(FunSignature { args: other_args, ret: other_ret, .. }),
        ) = (this, other) else {
            unreachable!("this function must be called with function type")
        };

        // check args
        let args_ok = this_args
            .iter()
            .map(|FuncArg { name, ty, .. }| (name, type_var_to_concrete_def.get(ty).unwrap()))
            .zip(other_args.iter().map(|FuncArg { name, ty, .. }| {
                (name, type_var_to_concrete_def.get(ty).unwrap())
            }))
            .all(|(this, other)| {
                if this.0 == &"self".into() && this.0 == other.0 {
                    true
                } else {
                    this.0 == other.0
                        && check_overload_type_annotation_compatible(this.1, other.1, unifier)
                }
            });

        // check rets
        let ret_ok = check_overload_type_annotation_compatible(
            type_var_to_concrete_def.get(this_ret).unwrap(),
            type_var_to_concrete_def.get(other_ret).unwrap(),
            unifier,
        );

        // return
        args_ok && ret_ok
    }

    pub fn check_overload_field_type(
        this: Type,
        other: Type,
        unifier: &mut Unifier,
        type_var_to_concrete_def: &HashMap<Type, TypeAnnotation>,
    ) -> bool {
        check_overload_type_annotation_compatible(
            type_var_to_concrete_def.get(&this).unwrap(),
            type_var_to_concrete_def.get(&other).unwrap(),
            unifier,
        )
    }

    pub fn get_all_assigned_field(stmts: &[Stmt<()>]) -> Result<HashSet<StrRef>, HashSet<String>> {
        let mut result = HashSet::new();
        for s in stmts {
            match &s.node {
                ast::StmtKind::AnnAssign { target, .. }
                    if {
                        if let ast::ExprKind::Attribute { value, .. } = &target.node {
                            if let ast::ExprKind::Name { id, .. } = &value.node {
                                id == &"self".into()
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } =>
                {
                    return Err(HashSet::from([
                        format!(
                            "redundant type annotation for class fields at {}",
                            s.location
                        ),
                    ]))
                }
                ast::StmtKind::Assign { targets, .. } => {
                    for t in targets {
                        if let ast::ExprKind::Attribute { value, attr, .. } = &t.node {
                            if let ast::ExprKind::Name { id, .. } = &value.node {
                                if id == &"self".into() {
                                    result.insert(*attr);
                                }
                            }
                        }
                    }
                }
                // TODO: do not check for For and While?
                ast::StmtKind::For { body, orelse, .. }
                | ast::StmtKind::While { body, orelse, .. } => {
                    result.extend(Self::get_all_assigned_field(body.as_slice())?);
                    result.extend(Self::get_all_assigned_field(orelse.as_slice())?);
                }
                ast::StmtKind::If { body, orelse, .. } => {
                    let inited_for_sure = Self::get_all_assigned_field(body.as_slice())?
                        .intersection(&Self::get_all_assigned_field(orelse.as_slice())?)
                        .copied()
                        .collect::<HashSet<_>>();
                    result.extend(inited_for_sure);
                }
                ast::StmtKind::Try { body, orelse, finalbody, .. } => {
                    let inited_for_sure = Self::get_all_assigned_field(body.as_slice())?
                        .intersection(&Self::get_all_assigned_field(orelse.as_slice())?)
                        .copied()
                        .collect::<HashSet<_>>();
                    result.extend(inited_for_sure);
                    result.extend(Self::get_all_assigned_field(finalbody.as_slice())?);
                }
                ast::StmtKind::With { body, .. } => {
                    result.extend(Self::get_all_assigned_field(body.as_slice())?);
                }
                ast::StmtKind::Pass { .. }
                | ast::StmtKind::Assert { .. }
                | ast::StmtKind::Expr { .. } => {}

                _ => {
                    unimplemented!()
                }
            }
        }
        Ok(result)
    }

    pub fn parse_parameter_default_value(
        default: &ast::Expr,
        resolver: &(dyn SymbolResolver + Send + Sync),
    ) -> Result<SymbolValue, HashSet<String>> {
        parse_parameter_default_value(default, resolver)
    }

    pub fn check_default_param_type(
        val: &SymbolValue,
        ty: &TypeAnnotation,
        primitive: &PrimitiveStore,
        unifier: &mut Unifier,
    ) -> Result<(), String> {
        fn is_compatible(
            found: &TypeAnnotation,
            expect: &TypeAnnotation,
            unifier: &mut Unifier,
            primitive: &PrimitiveStore,
        ) -> bool {
            match (found, expect) {
                (TypeAnnotation::Primitive(f), TypeAnnotation::Primitive(e)) => {
                    unifier.unioned(*f, *e)
                }
                (
                    TypeAnnotation::CustomClass { id: f_id, params: f_param },
                    TypeAnnotation::CustomClass { id: e_id, params: e_param },
                ) => {
                    *f_id == *e_id
                        && *f_id == primitive.option.get_obj_id(unifier)
                        && (f_param.is_empty()
                            || (f_param.len() == 1
                                && e_param.len() == 1
                                && is_compatible(&f_param[0], &e_param[0], unifier, primitive)))
                }
                (TypeAnnotation::Tuple(f), TypeAnnotation::Tuple(e)) => {
                    f.len() == e.len()
                        && f.iter()
                            .zip(e.iter())
                            .all(|(f, e)| is_compatible(f, e, unifier, primitive))
                }
                _ => false,
            }
        }

        let found = val.get_type_annotation(primitive, unifier);
        if is_compatible(&found, ty, unifier, primitive) {
            Ok(())
        } else {
            Err(format!(
                "incompatible default parameter type, expect {}, found {}",
                ty.stringify(unifier),
                found.stringify(unifier),
            ))
        }
    }
}

pub fn parse_parameter_default_value(
    default: &ast::Expr,
    resolver: &(dyn SymbolResolver + Send + Sync),
) -> Result<SymbolValue, HashSet<String>> {
    fn handle_constant(val: &Constant, loc: &Location) -> Result<SymbolValue, HashSet<String>> {
        match val {
            Constant::Int(v) => {
                if let Ok(v) = (*v).try_into() {
                    Ok(SymbolValue::I32(v))
                } else {
                    Err(HashSet::from([format!("integer value out of range at {loc}")]))
                }
            }
            Constant::Float(v) => Ok(SymbolValue::Double(*v)),
            Constant::Bool(v) => Ok(SymbolValue::Bool(*v)),
            Constant::Tuple(tuple) => Ok(SymbolValue::Tuple(
                tuple.iter().map(|x| handle_constant(x, loc)).collect::<Result<Vec<_>, _>>()?,
            )),
            Constant::None => Err(HashSet::from([
                format!(
                    "`None` is not supported, use `none` for option type instead ({loc})"
                ),
            ])),
            _ => unimplemented!("this constant is not supported at {}", loc),
        }
    }
    match &default.node {
        ast::ExprKind::Constant { value, .. } => handle_constant(value, &default.location),
        ast::ExprKind::Call { func, args, .. } if args.len() == 1 => {
            match &func.node {
                ast::ExprKind::Name { id, .. } if *id == "int64".into() => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        let v: Result<i64, _> = (*v).try_into();
                        match v {
                            Ok(v) => Ok(SymbolValue::I64(v)),
                            _ => Err(HashSet::from([
                                format!("default param value out of range at {}", default.location)
                            ])),
                        }
                    }
                    _ => Err(HashSet::from([
                        format!("only allow constant integer here at {}", default.location),
                    ]))
                }
                ast::ExprKind::Name { id, .. } if *id == "uint32".into() => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        let v: Result<u32, _> = (*v).try_into();
                        match v {
                            Ok(v) => Ok(SymbolValue::U32(v)),
                            _ => Err(HashSet::from([
                                format!("default param value out of range at {}", default.location),
                            ])),
                        }
                    }
                    _ => Err(HashSet::from([
                        format!("only allow constant integer here at {}", default.location),
                    ]))
                }
                ast::ExprKind::Name { id, .. } if *id == "uint64".into() => match &args[0].node {
                    ast::ExprKind::Constant { value: Constant::Int(v), .. } => {
                        let v: Result<u64, _> = (*v).try_into();
                        match v {
                            Ok(v) => Ok(SymbolValue::U64(v)),
                            _ => Err(HashSet::from([
                                format!("default param value out of range at {}", default.location),
                            ])),
                        }
                    }
                    _ => Err(HashSet::from([
                        format!("only allow constant integer here at {}", default.location),
                    ]))
                }
                ast::ExprKind::Name { id, .. } if *id == "Some".into() => Ok(
                    SymbolValue::OptionSome(
                        Box::new(parse_parameter_default_value(&args[0], resolver)?)
                    )
                ),
                _ => Err(HashSet::from([
                    format!("unsupported default parameter at {}", default.location),
                ])),
            }
        }
        ast::ExprKind::Tuple { elts, .. } => Ok(SymbolValue::Tuple(elts
            .iter()
            .map(|x| parse_parameter_default_value(x, resolver))
            .collect::<Result<Vec<_>, _>>()?
        )),
        ast::ExprKind::Name { id, .. } if id == &"none".into() => Ok(SymbolValue::OptionNone),
        ast::ExprKind::Name { id, .. } => {
            resolver.get_default_param_value(default).ok_or_else(
                || HashSet::from([
                    format!(
                        "`{}` cannot be used as a default parameter at {} \
                        (not primitive type, option or tuple / not defined?)",
                        id,
                        default.location
                    ),
                ])
            )
        }
        _ => Err(HashSet::from([
            format!(
                "unsupported default parameter (not primitive type, option or tuple) at {}",
                default.location
            ),
        ]))
    }
}
