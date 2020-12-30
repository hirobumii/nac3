use super::typedef::{Type::*, *};
use std::collections::HashMap;
use std::rc::Rc;

fn find_subst(
    ctx: &GlobalContext,
    valuation: &Option<(VariableId, Rc<Type>)>,
    sub: &mut HashMap<VariableId, Rc<Type>>,
    mut a: Rc<Type>,
    mut b: Rc<Type>,
) -> Result<(), String> {
    // TODO: fix error messages later
    if let TypeVariable(id) = a.as_ref() {
        if let Some((assumption_id, t)) = valuation {
            if assumption_id == id {
                a = t.clone();
            }
        }
    }

    let mut substituted = false;
    if let TypeVariable(id) = b.as_ref() {
        if let Some(c) = sub.get(&id) {
            b = c.clone();
            substituted = true;
        }
    }

    match (a.as_ref(), b.as_ref()) {
        (BotType, _) => Ok(()),
        (TypeVariable(id_a), TypeVariable(id_b)) => {
            if substituted {
                return if id_a == id_b {
                    Ok(())
                } else {
                    Err("different variables".to_string())
                };
            }
            let v_a = ctx.get_variable(*id_a);
            let v_b = ctx.get_variable(*id_b);
            if v_b.bound.len() > 0 {
                if v_a.bound.len() == 0 {
                    return Err("unbounded a".to_string());
                } else {
                    let diff: Vec<_> = v_a
                        .bound
                        .iter()
                        .filter(|x| !v_b.bound.contains(x))
                        .collect();
                    if diff.len() > 0 {
                        return Err("different domain".to_string());
                    }
                }
            }
            sub.insert(*id_b, a.clone().into());
            Ok(())
        }
        (TypeVariable(id_a), _) => {
            let v_a = ctx.get_variable(*id_a);
            if v_a.bound.len() == 1 && v_a.bound[0].as_ref() == b.as_ref() {
                Ok(())
            } else {
                Err("different domain".to_string())
            }
        }
        (_, TypeVariable(id_b)) => {
            let v_b = ctx.get_variable(*id_b);
            if v_b.bound.len() == 0 || v_b.bound.contains(&a) {
                sub.insert(*id_b, a.clone().into());
                Ok(())
            } else {
                Err("different domain".to_string())
            }
        }
        (_, VirtualClassType(id_b)) => {
            let mut parents;
            match a.as_ref() {
                ClassType(id_a) => {
                    parents = [*id_a].to_vec();
                }
                VirtualClassType(id_a) => {
                    parents = [*id_a].to_vec();
                }
                _ => {
                    return Err("cannot substitute non-class type into virtual class".to_string());
                }
            };
            while !parents.is_empty() {
                if *id_b == parents[0] {
                    return Ok(());
                }
                let c = ctx.get_class(parents.remove(0));
                parents.extend_from_slice(&c.parents);
            }
            Err("not subtype".to_string())
        }
        (ParametricType(id_a, param_a), ParametricType(id_b, param_b)) => {
            if id_a != id_b || param_a.len() != param_b.len() {
                Err("different parametric types".to_string())
            } else {
                for (x, y) in param_a.iter().zip(param_b.iter()) {
                    find_subst(ctx, valuation, sub, x.clone(), y.clone())?;
                }
                Ok(())
            }
        }
        (_, _) => {
            if a == b {
                Ok(())
            } else {
                Err("not equal".to_string())
            }
        }
    }
}

fn resolve_call_rec(
    ctx: &GlobalContext,
    valuation: &Option<(VariableId, Rc<Type>)>,
    obj: Option<Rc<Type>>,
    func: &str,
    args: &[Rc<Type>],
) -> Result<Option<Rc<Type>>, String> {
    let mut subst = obj
        .as_ref()
        .map(|v| v.get_subst(ctx))
        .unwrap_or(HashMap::new());

    let fun = match &obj {
        Some(obj) => {
            let base = match obj.as_ref() {
                TypeVariable(id) => {
                    let v = ctx.get_variable(*id);
                    if v.bound.len() == 0 {
                        return Err("unbounded type var".to_string());
                    }
                    let results: Result<Vec<_>, String> = v
                        .bound
                        .iter()
                        .map(|ins| {
                            resolve_call_rec(
                                ctx,
                                &Some((*id, ins.clone())),
                                Some(ins.clone()),
                                func,
                                args.clone(),
                            )
                        })
                        .collect();
                    let results = results?;
                    if results.iter().all(|v| v == &results[0]) {
                        return Ok(results[0].clone());
                    }
                    let mut results = results.iter().zip(v.bound.iter()).map(|(r, ins)| {
                        r.as_ref()
                            .map(|v| v.inv_subst(&[(ins.clone(), obj.clone().into())]))
                    });
                    let first = results.next().unwrap();
                    if results.all(|v| v == first) {
                        return Ok(first);
                    } else {
                        return Err("divergent type after substitution".to_string());
                    }
                }
                PrimitiveType(id) => &ctx.get_primitive(*id),
                ClassType(id) | VirtualClassType(id) => &ctx.get_class(*id).base,
                ParametricType(id, _) => &ctx.get_parametric(*id).base,
                _ => return Err("not supported".to_string()),
            };
            base.methods.get(func)
        }
        None => ctx.get_fn(func),
    }
    .ok_or("no such function".to_string())?;

    if args.len() != fun.args.len() {
        return Err("incorrect parameter number".to_string());
    }
    for (a, b) in args.iter().zip(fun.args.iter()) {
        find_subst(ctx, valuation, &mut subst, a.clone(), b.clone())?;
    }
    let result = fun.result.as_ref().map(|v| v.subst(&subst));
    Ok(result.map(|result| {
        if let SelfType = result {
            obj.unwrap()
        } else {
            result.into()
        }
    }))
}

pub fn resolve_call(
    ctx: &GlobalContext,
    obj: Option<Rc<Type>>,
    func: &str,
    args: &[Rc<Type>],
) -> Result<Option<Rc<Type>>, String> {
    resolve_call_rec(ctx, &None, obj, func, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::*;

    #[test]
    fn test_simple_generic() {
        let mut ctx = basic_ctx();

        assert_eq!(
            resolve_call(&ctx, None, "int32", &[PrimitiveType(FLOAT_TYPE).into()]),
            Ok(Some(PrimitiveType(INT32_TYPE).into()))
        );

        assert_eq!(
            resolve_call(&ctx, None, "int32", &[PrimitiveType(INT32_TYPE).into()],),
            Ok(Some(PrimitiveType(INT32_TYPE).into()))
        );

        assert_eq!(
            resolve_call(&ctx, None, "float", &[PrimitiveType(INT32_TYPE).into()]),
            Ok(Some(PrimitiveType(FLOAT_TYPE).into()))
        );

        assert_eq!(
            resolve_call(&ctx, None, "float", &[PrimitiveType(BOOL_TYPE).into()]),
            Err("different domain".to_string())
        );

        assert_eq!(
            resolve_call(&ctx, None, "float", &[]),
            Err("incorrect parameter number".to_string())
        );

        let v1 = ctx.add_variable(VarDef {
            name: "V1",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(FLOAT_TYPE).into(),
            ],
        });

        assert_eq!(
            resolve_call(&ctx, None, "float", &[TypeVariable(v1).into()]),
            Ok(Some(PrimitiveType(FLOAT_TYPE).into()))
        );

        let v2 = ctx.add_variable(VarDef {
            name: "V2",
            bound: vec![
                PrimitiveType(BOOL_TYPE).into(),
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(FLOAT_TYPE).into(),
            ],
        });

        assert_eq!(
            resolve_call(&ctx, None, "float", &[TypeVariable(v2).into()]),
            Err("different domain".to_string())
        );
    }

    #[test]
    fn test_methods() {
        let mut ctx = basic_ctx();

        let v0 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V0",
            bound: vec![],
        })));
        let v1 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V1",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(FLOAT_TYPE).into(),
            ],
        })));
        let v2 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V2",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(FLOAT_TYPE).into(),
            ],
        })));
        let v3 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V3",
            bound: vec![
                PrimitiveType(BOOL_TYPE).into(),
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(FLOAT_TYPE).into(),
            ],
        })));

        let int32 = Rc::new(PrimitiveType(INT32_TYPE));
        let int64 = Rc::new(PrimitiveType(INT64_TYPE));

        // simple cases
        assert_eq!(
            resolve_call(&ctx, Some(int32.clone()), "__add__", &[int32.clone()]),
            Ok(Some(int32.clone()))
        );

        assert_ne!(
            resolve_call(&ctx, Some(int32.clone()), "__add__", &[int32.clone()]),
            Ok(Some(int64.clone()))
        );

        assert_eq!(
            resolve_call(&ctx, Some(int32.clone()), "__add__", &[int64.clone()]),
            Err("not equal".to_string())
        );

        // with type variables
        assert_eq!(
            resolve_call(&ctx, Some(v1.clone()), "__add__", &[v1.clone()]),
            Ok(Some(v1.clone()))
        );
        assert_eq!(
            resolve_call(&ctx, Some(v0.clone()), "__add__", &[v2.clone()]),
            Err("unbounded type var".to_string())
        );
        assert_eq!(
            resolve_call(&ctx, Some(v1.clone()), "__add__", &[v0.clone()]),
            Err("different domain".to_string())
        );
        assert_eq!(
            resolve_call(&ctx, Some(v1.clone()), "__add__", &[v2.clone()]),
            Err("different domain".to_string())
        );
        assert_eq!(
            resolve_call(&ctx, Some(v1.clone()), "__add__", &[v3.clone()]),
            Err("different domain".to_string())
        );
        assert_eq!(
            resolve_call(&ctx, Some(v3.clone()), "__add__", &[v1.clone()]),
            Err("no such function".to_string())
        );
        assert_eq!(
            resolve_call(&ctx, Some(v3.clone()), "__add__", &[v3.clone()]),
            Err("no such function".to_string())
        );
    }

    #[test]
    fn test_multi_generic() {
        let mut ctx = basic_ctx();
        let v0 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V0",
            bound: vec![],
        })));
        let v1 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V1",
            bound: vec![],
        })));
        let v2 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V2",
            bound: vec![],
        })));
        let v3 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V3",
            bound: vec![],
        })));

        ctx.add_fn(
            "foo",
            FnDef {
                args: vec![v0.clone(), v0.clone(), v1.clone()],
                result: Some(v0.clone()),
            },
        );

        ctx.add_fn(
            "foo1",
            FnDef {
                args: vec![
                    ParametricType(TUPLE_TYPE, vec![v0.clone(), v0.clone(), v1.clone()]).into(),
                ],
                result: Some(v0.clone()),
            },
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo", &[v2.clone(), v2.clone(), v2.clone()]),
            Ok(Some(v2.clone()))
        );
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[v2.clone(), v2.clone(), v3.clone()]),
            Ok(Some(v2.clone()))
        );
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[v2.clone(), v3.clone(), v3.clone()]),
            Err("different variables".to_string())
        );

        assert_eq!(
            resolve_call(
                &ctx,
                None,
                "foo1",
                &[ParametricType(TUPLE_TYPE, vec![v2.clone(), v2.clone(), v2.clone()]).into()]
            ),
            Ok(Some(v2.clone()))
        );
        assert_eq!(
            resolve_call(
                &ctx,
                None,
                "foo1",
                &[ParametricType(TUPLE_TYPE, vec![v2.clone(), v2.clone(), v3.clone()]).into()]
            ),
            Ok(Some(v2.clone()))
        );
        assert_eq!(
            resolve_call(
                &ctx,
                None,
                "foo1",
                &[ParametricType(TUPLE_TYPE, vec![v2.clone(), v3.clone(), v3.clone()]).into()]
            ),
            Err("different variables".to_string())
        );
    }

    #[test]
    fn test_class_generics() {
        let mut ctx = basic_ctx();

        let list = ctx.get_parametric_mut(LIST_TYPE);
        let t = Rc::new(TypeVariable(list.params[0]));
        list.base.methods.insert(
            "head",
            FnDef {
                args: vec![],
                result: Some(t.clone()),
            },
        );
        list.base.methods.insert(
            "append",
            FnDef {
                args: vec![t.clone()],
                result: None,
            },
        );

        let v0 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V0",
            bound: vec![],
        })));
        let v1 = Rc::new(TypeVariable(ctx.add_variable(VarDef {
            name: "V1",
            bound: vec![],
        })));

        assert_eq!(
            resolve_call(
                &ctx,
                Some(ParametricType(LIST_TYPE, vec![v0.clone()]).into()),
                "head",
                &[]
            ),
            Ok(Some(v0.clone()))
        );
        assert_eq!(
            resolve_call(
                &ctx,
                Some(ParametricType(LIST_TYPE, vec![v0.clone()]).into()),
                "append",
                &[v0.clone()]
            ),
            Ok(None)
        );
        assert_eq!(
            resolve_call(
                &ctx,
                Some(ParametricType(LIST_TYPE, vec![v0.clone()]).into()),
                "append",
                &[v1.clone()]
            ),
            Err("different variables".to_string())
        );
    }

    #[test]
    fn test_virtual_class() {
        let mut ctx = basic_ctx();

        let foo = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Foo",
                methods: HashMap::new(),
                fields: HashMap::new(),
            },
            parents: vec![],
        });

        let foo1 = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Foo1",
                methods: HashMap::new(),
                fields: HashMap::new(),
            },
            parents: vec![foo],
        });

        let foo2 = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Foo2",
                methods: HashMap::new(),
                fields: HashMap::new(),
            },
            parents: vec![foo1],
        });

        let bar = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "bar",
                methods: HashMap::new(),
                fields: HashMap::new(),
            },
            parents: vec![],
        });

        ctx.add_fn(
            "foo",
            FnDef {
                args: vec![VirtualClassType(foo).into()],
                result: None,
            },
        );
        ctx.add_fn(
            "foo1",
            FnDef {
                args: vec![VirtualClassType(foo1).into()],
                result: None,
            },
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo", &[ClassType(foo).into()]),
            Ok(None)
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo", &[ClassType(foo1).into()]),
            Ok(None)
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo", &[ClassType(foo2).into()]),
            Ok(None)
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo", &[ClassType(bar).into()]),
            Err("not subtype".to_string())
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo1", &[ClassType(foo1).into()]),
            Ok(None)
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo1", &[ClassType(foo2).into()]),
            Ok(None)
        );

        assert_eq!(
            resolve_call(&ctx, None, "foo1", &[ClassType(foo).into()]),
            Err("not subtype".to_string())
        );

        // virtual class substitution
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[VirtualClassType(foo).into()]),
            Ok(None)
        );
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[VirtualClassType(foo1).into()]),
            Ok(None)
        );
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[VirtualClassType(foo2).into()]),
            Ok(None)
        );
        assert_eq!(
            resolve_call(&ctx, None, "foo", &[VirtualClassType(bar).into()]),
            Err("not subtype".to_string())
        );
    }
}
