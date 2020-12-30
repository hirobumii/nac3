use crate::inference::resolve_call;
use crate::operators::*;
use crate::primitives::*;
use crate::typedef::{GlobalContext, Type, Type::*};
use rustpython_parser::ast::{
    Comparison, Comprehension, ComprehensionKind, Expression, ExpressionType, Operator,
    UnaryOperator,
};
use std::collections::HashMap;
use std::convert::TryInto;
use std::rc::Rc;

type SymTable<'a> = HashMap<&'a str, Rc<Type>>;
type ParserResult = Result<Option<Rc<Type>>, String>;

pub fn parse_expr(ctx: &GlobalContext, sym_table: &SymTable, expr: &Expression) -> ParserResult {
    match &expr.node {
        ExpressionType::Number { value } => parse_constant(ctx, sym_table, value),
        ExpressionType::Identifier { name } => parse_identifier(ctx, sym_table, name),
        ExpressionType::List { elements } => parse_list(ctx, sym_table, elements),
        ExpressionType::Tuple { elements } => parse_tuple(ctx, sym_table, elements),
        ExpressionType::Attribute { value, name } => parse_attribute(ctx, sym_table, value, name),
        ExpressionType::BoolOp { values, .. } => parse_bool_ops(ctx, sym_table, values),
        ExpressionType::Binop { a, b, op } => parse_bin_ops(ctx, sym_table, op, a, b),
        ExpressionType::Unop { op, a } => parse_unary_ops(ctx, sym_table, op, a),
        ExpressionType::Compare { vals, ops } => parse_compare(ctx, sym_table, vals, ops),
        ExpressionType::Call {
            args,
            function,
            keywords,
        } => {
            if keywords.len() > 0 {
                Err("keyword is not supported".into())
            } else {
                parse_call(ctx, sym_table, &args, &function)
            }
        }
        ExpressionType::Subscript { a, b } => parse_subscript(ctx, sym_table, a, b),
        ExpressionType::IfExpression { test, body, orelse } => {
            parse_if_expr(ctx, sym_table, &test, &body, orelse)
        }
        ExpressionType::Comprehension { kind, generators } => match kind.as_ref() {
            ComprehensionKind::List { element } => {
                if generators.len() == 1 {
                    parse_list_comprehension(ctx, sym_table, element, &generators[0])
                } else {
                    Err("only 1 generator statement is supported".into())
                }
            }
            _ => Err("only list comprehension is supported".into()),
        },
        ExpressionType::True | ExpressionType::False => Ok(Some(PrimitiveType(BOOL_TYPE).into())),
        _ => Err("not supported".into()),
    }
}

fn parse_constant(
    _: &GlobalContext,
    _: &SymTable,
    value: &rustpython_parser::ast::Number,
) -> ParserResult {
    use rustpython_parser::ast::Number;
    match value {
        Number::Integer { value } => {
            let int32: Result<i32, _> = value.try_into();
            if int32.is_ok() {
                Ok(Some(PrimitiveType(INT32_TYPE).into()))
            } else {
                let int64: Result<i64, _> = value.try_into();
                if int64.is_ok() {
                    Ok(Some(PrimitiveType(INT64_TYPE).into()))
                } else {
                    Err("integer out of range".into())
                }
            }
        }
        Number::Float { .. } => Ok(Some(PrimitiveType(FLOAT_TYPE).into())),
        _ => Err("not supported".into()),
    }
}

fn parse_identifier(_: &GlobalContext, sym_table: &SymTable, name: &str) -> ParserResult {
    match sym_table.get(name) {
        Some(v) => Ok(Some(v.clone())),
        None => Err("unbounded variable".into()),
    }
}

fn parse_list(ctx: &GlobalContext, sym_table: &SymTable, elements: &[Expression]) -> ParserResult {
    if elements.len() == 0 {
        return Ok(Some(ParametricType(LIST_TYPE, vec![BotType.into()]).into()));
    }

    let mut types = elements.iter().map(|v| parse_expr(&ctx, sym_table, v));

    let head = types.next().unwrap()?;
    if head.is_none() {
        return Err("list elements must have some type".into());
    }
    for v in types {
        if v? != head {
            return Err("inhomogeneous list is not allowed".into());
        }
    }
    Ok(Some(ParametricType(LIST_TYPE, vec![head.unwrap()]).into()))
}

fn parse_tuple(ctx: &GlobalContext, sym_table: &SymTable, elements: &[Expression]) -> ParserResult {
    let types: Result<Option<Vec<_>>, String> = elements
        .iter()
        .map(|v| parse_expr(&ctx, sym_table, v))
        .collect();
    if let Some(t) = types? {
        Ok(Some(ParametricType(TUPLE_TYPE, t).into()))
    } else {
        Err("tuple elements must have some type".into())
    }
}

fn parse_attribute(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    value: &Expression,
    name: &String,
) -> ParserResult {
    let value = parse_expr(ctx, sym_table, value)?.ok_or("no value".to_string())?;
    if let TypeVariable(id) = value.as_ref() {
        let v = ctx.get_variable(*id);
        if v.bound.len() == 0 {
            return Err("no fields on unbounded type variable".into());
        }
        let ty = v.bound[0]
            .get_base(ctx)
            .and_then(|v| v.fields.get(name.as_str()));
        if ty.is_none() {
            return Err("unknown field".into());
        }
        for x in v.bound[1..].iter() {
            let ty1 = x.get_base(ctx).and_then(|v| v.fields.get(name.as_str()));
            if ty1 != ty {
                return Err("unknown field (type mismatch between variants)".into());
            }
        }
        return Ok(Some(ty.unwrap().clone()));
    }

    match value.get_base(ctx) {
        Some(b) => match b.fields.get(name.as_str()) {
            Some(t) => Ok(Some(t.clone())),
            None => Err("no such field".into()),
        },
        None => Err("this object has no fields".into()),
    }
}

fn parse_bool_ops(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    values: &[Expression],
) -> ParserResult {
    assert_eq!(values.len(), 2);
    let left = parse_expr(ctx, sym_table, &values[0])?.ok_or("no value".to_string())?;
    let right = parse_expr(ctx, sym_table, &values[1])?.ok_or("no value".to_string())?;

    let b = PrimitiveType(BOOL_TYPE);
    if left.as_ref() == &b && right.as_ref() == &b {
        Ok(Some(b.into()))
    } else {
        Err("bool operands must be bool".into())
    }
}

fn parse_bin_ops(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    op: &Operator,
    left: &Expression,
    right: &Expression,
) -> ParserResult {
    let left = parse_expr(ctx, sym_table, left)?.ok_or("no value".to_string())?;
    let right = parse_expr(ctx, sym_table, right)?.ok_or("no value".to_string())?;
    let fun = binop_name(op);
    resolve_call(ctx, Some(left), fun, &[right])
}

fn parse_unary_ops(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    op: &UnaryOperator,
    obj: &Expression,
) -> ParserResult {
    let ty = parse_expr(ctx, sym_table, obj)?.ok_or("no value".to_string())?;
    if let UnaryOperator::Not = op {
        if ty.as_ref() == &PrimitiveType(BOOL_TYPE) {
            Ok(Some(ty))
        } else {
            Err("logical not must be applied to bool".into())
        }
    } else {
        resolve_call(ctx, Some(ty), unaryop_name(op), &[])
    }
}

fn parse_compare(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    vals: &[Expression],
    ops: &[Comparison],
) -> ParserResult {
    let types: Result<Option<Vec<_>>, _> =
        vals.iter().map(|v| parse_expr(ctx, sym_table, v)).collect();
    let types = types?;
    if types.is_none() {
        return Err("comparison operands must have type".into());
    }
    let types = types.unwrap();
    let boolean = PrimitiveType(BOOL_TYPE);
    let left = &types[..types.len() - 1];
    let right = &types[1..];

    for ((a, b), op) in left.iter().zip(right.iter()).zip(ops.iter()) {
        let fun = comparison_name(op).ok_or("unsupported comparison".to_string())?;
        let ty = resolve_call(ctx, Some(a.clone()), fun, &[b.clone()])?;
        if ty.is_none() || ty.unwrap().as_ref() != &boolean {
            return Err("comparison result must be boolean".into());
        }
    }
    Ok(Some(boolean.into()))
}

fn parse_call(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    args: &[Expression],
    function: &Expression,
) -> ParserResult {
    let types: Result<Option<Vec<_>>, _> =
        args.iter().map(|v| parse_expr(ctx, sym_table, v)).collect();
    let types = types?;
    if types.is_none() {
        return Err("function params must have type".into());
    }

    let (obj, fun) = match &function.node {
        ExpressionType::Identifier { name } => (None, name),
        ExpressionType::Attribute { value, name } => (
            Some(parse_expr(ctx, sym_table, &value)?.ok_or("no value".to_string())?),
            name,
        ),
        _ => return Err("not supported".into()),
    };
    resolve_call(ctx, obj, fun.as_str(), &types.unwrap())
}

fn parse_subscript(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    a: &Expression,
    b: &Expression,
) -> ParserResult {
    let a = parse_expr(ctx, sym_table, a)?.ok_or("no value".to_string())?;
    let t = if let ParametricType(LIST_TYPE, ls) = a.as_ref() {
        ls[0].clone()
    } else {
        return Err("subscript is not supported for types other than list".into());
    };

    match &b.node {
        ExpressionType::Slice { elements } => {
            let int32 = Rc::new(PrimitiveType(INT32_TYPE));
            let types: Result<Option<Vec<_>>, _> = elements
                .iter()
                .map(|v| {
                    if let ExpressionType::None = v.node {
                        Ok(Some(int32.clone()))
                    } else {
                        parse_expr(ctx, sym_table, v)
                    }
                })
                .collect();
            let types = types?.ok_or("slice must have type".to_string())?;
            if types.iter().all(|v| v == &int32) {
                Ok(Some(a))
            } else {
                Err("slice must be int32 type".into())
            }
        }
        _ => {
            let b = parse_expr(ctx, sym_table, b)?.ok_or("no value".to_string())?;
            if b.as_ref() == &PrimitiveType(INT32_TYPE) {
                Ok(Some(t))
            } else {
                Err("index must be either slice or int32".into())
            }
        }
    }
}

fn parse_if_expr(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    test: &Expression,
    body: &Expression,
    orelse: &Expression,
) -> ParserResult {
    let test = parse_expr(ctx, sym_table, test)?.ok_or("no value".to_string())?;
    if test.as_ref() != &PrimitiveType(BOOL_TYPE) {
        return Err("test should be bool".into());
    }

    let body = parse_expr(ctx, sym_table, body)?;
    let orelse = parse_expr(ctx, sym_table, orelse)?;
    if body.as_ref() == orelse.as_ref() {
        Ok(body)
    } else {
        Err("divergent type".into())
    }
}

fn parse_simple_binding<'a: 'b, 'b>(
    sym_table: &mut SymTable<'b>,
    name: &'a Expression,
    ty: Rc<Type>,
) -> Result<(), String> {
    match &name.node {
        ExpressionType::Identifier { name } => {
            if name == "_" {
                Ok(())
            } else if sym_table.get(name.as_str()).is_some() {
                Err("duplicated naming".into())
            } else {
                sym_table.insert(name.as_str(), ty);
                Ok(())
            }
        }
        ExpressionType::Tuple { elements } => {
            if let ParametricType(TUPLE_TYPE, ls) = ty.as_ref() {
                if elements.len() == ls.len() {
                    for (a, b) in elements.iter().zip(ls.iter()) {
                        parse_simple_binding(sym_table, a, b.clone())?;
                    }
                    Ok(())
                } else {
                    Err("different length".into())
                }
            } else {
                Err("not supported".into())
            }
        }
        _ => Err("not supported".into()),
    }
}

fn parse_list_comprehension(
    ctx: &GlobalContext,
    sym_table: &SymTable,
    element: &Expression,
    comprehension: &Comprehension,
) -> ParserResult {
    if comprehension.is_async {
        return Err("async is not supported".into());
    }

    // TODO: it may be more efficient to use multi-level table
    // but it would better done in a whole program level
    let iter = parse_expr(ctx, sym_table, &comprehension.iter)?.ok_or("no value".to_string())?;
    if let ParametricType(LIST_TYPE, ls) = iter.as_ref() {
        let mut local_sym = sym_table.clone();
        parse_simple_binding(&mut local_sym, &comprehension.target, ls[0].clone())?;

        let boolean = PrimitiveType(BOOL_TYPE);
        for test in comprehension.ifs.iter() {
            let result =
                parse_expr(ctx, &local_sym, test)?.ok_or("no value in test".to_string())?;
            if result.as_ref() != &boolean {
                return Err("test must be bool".into());
            }
        }
        let result = parse_expr(ctx, &local_sym, element)?.ok_or("no value")?;
        Ok(Some(ParametricType(LIST_TYPE, vec![result]).into()))
    } else {
        Err("iteration is supported for list only".into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::typedef::*;
    use rustpython_parser::parser::parse_expression;

    #[test]
    fn test_constants() {
        let ctx = basic_ctx();
        let sym_table = HashMap::new();

        let result = parse_expr(&ctx, &sym_table, &parse_expression("123").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("2147483647").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("2147483648").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT64_TYPE).into());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("9223372036854775807").unwrap(),
        );
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT64_TYPE).into());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("9223372036854775808").unwrap(),
        );
        assert_eq!(result, Err("integer out of range".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("123.456").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(FLOAT_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("True").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(BOOL_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("False").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(BOOL_TYPE).into());
    }

    #[test]
    fn test_identifier() {
        let ctx = basic_ctx();
        let mut sym_table = HashMap::new();
        sym_table.insert("abc", Rc::new(PrimitiveType(INT32_TYPE)));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("abc").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("ab").unwrap());
        assert_eq!(result, Err("unbounded variable".into()));
    }

    #[test]
    fn test_list() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "foo",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let mut sym_table = HashMap::new();
        sym_table.insert("abc", Rc::new(PrimitiveType(INT32_TYPE)));
        // def is reserved...
        sym_table.insert("efg", Rc::new(PrimitiveType(INT32_TYPE)));
        sym_table.insert("xyz", Rc::new(PrimitiveType(FLOAT_TYPE)));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[]").unwrap());
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(LIST_TYPE, vec![BotType.into()]).into()
        );

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[abc]").unwrap());
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(LIST_TYPE, vec![PrimitiveType(INT32_TYPE).into()]).into()
        );

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[abc, efg]").unwrap());
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(LIST_TYPE, vec![PrimitiveType(INT32_TYPE).into()]).into()
        );

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[abc, efg, xyz]").unwrap(),
        );
        assert_eq!(result, Err("inhomogeneous list is not allowed".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[foo()]").unwrap());
        assert_eq!(result, Err("list elements must have some type".into()));
    }

    #[test]
    fn test_tuple() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "foo",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let mut sym_table = HashMap::new();
        sym_table.insert("abc", Rc::new(PrimitiveType(INT32_TYPE)));
        sym_table.insert("efg", Rc::new(PrimitiveType(FLOAT_TYPE)));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("(abc, efg)").unwrap());
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(
                TUPLE_TYPE,
                vec![
                    PrimitiveType(INT32_TYPE).into(),
                    PrimitiveType(FLOAT_TYPE).into()
                ]
            )
            .into()
        );

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("(abc, efg, foo())").unwrap(),
        );
        assert_eq!(result, Err("tuple elements must have some type".into()));
    }

    #[test]
    fn test_attribute() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );

        let foo = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Foo",
                fields: HashMap::new(),
                methods: HashMap::new(),
            },
            parents: vec![],
        });
        let foo_def = ctx.get_class_mut(foo);
        foo_def
            .base
            .fields
            .insert("a", PrimitiveType(INT32_TYPE).into());
        foo_def.base.fields.insert("b", ClassType(foo).into());
        foo_def
            .base
            .fields
            .insert("c", PrimitiveType(INT32_TYPE).into());

        let bar = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Bar",
                fields: HashMap::new(),
                methods: HashMap::new(),
            },
            parents: vec![],
        });
        let bar_def = ctx.get_class_mut(bar);
        bar_def
            .base
            .fields
            .insert("a", PrimitiveType(INT32_TYPE).into());
        bar_def.base.fields.insert("b", ClassType(bar).into());
        bar_def
            .base
            .fields
            .insert("c", PrimitiveType(FLOAT_TYPE).into());

        let v0 = ctx.add_variable(VarDef {
            name: "v0",
            bound: vec![],
        });

        let v1 = ctx.add_variable(VarDef {
            name: "v1",
            bound: vec![ClassType(foo).into(), ClassType(bar).into()],
        });

        let mut sym_table = HashMap::new();
        sym_table.insert("foo", Rc::new(ClassType(foo)));
        sym_table.insert("bar", Rc::new(ClassType(bar)));
        sym_table.insert("foobar", Rc::new(VirtualClassType(foo)));
        sym_table.insert("v0", Rc::new(TypeVariable(v0)));
        sym_table.insert("v1", Rc::new(TypeVariable(v1)));
        sym_table.insert("bot", Rc::new(BotType));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("foo.a").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("foo.d").unwrap());
        assert_eq!(result, Err("no such field".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("foobar.a").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v0.a").unwrap());
        assert_eq!(result, Err("no fields on unbounded type variable".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v1.a").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        // shall we support this?
        let result = parse_expr(&ctx, &sym_table, &parse_expression("v1.b").unwrap());
        assert_eq!(
            result,
            Err("unknown field (type mismatch between variants)".into())
        );
        // assert_eq!(result.unwrap().unwrap(), TypeVariable(v1).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v1.c").unwrap());
        assert_eq!(
            result,
            Err("unknown field (type mismatch between variants)".into())
        );

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v1.d").unwrap());
        assert_eq!(result, Err("unknown field".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("none().a").unwrap());
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("bot.a").unwrap());
        assert_eq!(result, Err("this object has no fields".into()));
    }

    #[test]
    fn test_bool_ops() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let sym_table = HashMap::new();

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("True and False").unwrap(),
        );
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(BOOL_TYPE).into());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("True and none()").unwrap(),
        );
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("True and 123").unwrap());
        assert_eq!(result, Err("bool operands must be bool".into()));
    }

    #[test]
    fn test_bin_ops() {
        let mut ctx = basic_ctx();
        let v0 = ctx.add_variable(VarDef {
            name: "v0",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(INT64_TYPE).into(),
            ],
        });
        let mut sym_table = HashMap::new();
        sym_table.insert("a", TypeVariable(v0).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("1 + 2 + 3").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("a + a + a").unwrap());
        assert_eq!(result.unwrap().unwrap(), TypeVariable(v0).into());
    }

    #[test]
    fn test_unary_ops() {
        let mut ctx = basic_ctx();
        let v0 = ctx.add_variable(VarDef {
            name: "v0",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(INT64_TYPE).into(),
            ],
        });
        let mut sym_table = HashMap::new();
        sym_table.insert("a", TypeVariable(v0).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("-(123)").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("-a").unwrap());
        assert_eq!(result.unwrap().unwrap(), TypeVariable(v0).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("not True").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(BOOL_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("not (1)").unwrap());
        assert_eq!(result, Err("logical not must be applied to bool".into()));
    }

    #[test]
    fn test_compare() {
        let mut ctx = basic_ctx();
        let v0 = ctx.add_variable(VarDef {
            name: "v0",
            bound: vec![
                PrimitiveType(INT32_TYPE).into(),
                PrimitiveType(INT64_TYPE).into(),
            ],
        });
        let mut sym_table = HashMap::new();
        sym_table.insert("a", TypeVariable(v0).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("a == a == a").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(BOOL_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("a == a == 1").unwrap());
        assert_eq!(result, Err("not equal".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("True > False").unwrap());
        assert_eq!(result, Err("no such function".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("True in False").unwrap(),
        );
        assert_eq!(result, Err("unsupported comparison".into()));
    }

    #[test]
    fn test_call() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );

        let foo = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Foo",
                fields: HashMap::new(),
                methods: HashMap::new(),
            },
            parents: vec![],
        });
        let foo_def = ctx.get_class_mut(foo);
        foo_def.base.methods.insert(
            "a",
            FnDef {
                args: vec![],
                result: Some(Rc::new(ClassType(foo))),
            },
        );

        let bar = ctx.add_class(ClassDef {
            base: TypeDef {
                name: "Bar",
                fields: HashMap::new(),
                methods: HashMap::new(),
            },
            parents: vec![],
        });
        let bar_def = ctx.get_class_mut(bar);
        bar_def.base.methods.insert(
            "a",
            FnDef {
                args: vec![],
                result: Some(Rc::new(ClassType(bar))),
            },
        );

        let v0 = ctx.add_variable(VarDef {
            name: "v0",
            bound: vec![],
        });
        let v1 = ctx.add_variable(VarDef {
            name: "v1",
            bound: vec![ClassType(foo).into(), ClassType(bar).into()],
        });
        let v2 = ctx.add_variable(VarDef {
            name: "v2",
            bound: vec![
                ClassType(foo).into(),
                ClassType(bar).into(),
                PrimitiveType(INT32_TYPE).into(),
            ],
        });
        let mut sym_table = HashMap::new();
        sym_table.insert("foo", Rc::new(ClassType(foo)));
        sym_table.insert("bar", Rc::new(ClassType(bar)));
        sym_table.insert("foobar", Rc::new(VirtualClassType(foo)));
        sym_table.insert("v0", Rc::new(TypeVariable(v0)));
        sym_table.insert("v1", Rc::new(TypeVariable(v1)));
        sym_table.insert("v2", Rc::new(TypeVariable(v2)));
        sym_table.insert("bot", Rc::new(BotType));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("foo.a()").unwrap());
        assert_eq!(result.unwrap().unwrap(), ClassType(foo).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v1.a()").unwrap());
        assert_eq!(result.unwrap().unwrap(), TypeVariable(v1).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("foobar.a()").unwrap());
        assert_eq!(result.unwrap().unwrap(), ClassType(foo).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("none().a()").unwrap());
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("bot.a()").unwrap());
        assert_eq!(result, Err("not supported".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[][0].a()").unwrap());
        assert_eq!(result, Err("not supported".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v0.a()").unwrap());
        assert_eq!(result, Err("unbounded type var".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("v2.a()").unwrap());
        assert_eq!(result, Err("no such function".into()));
    }

    #[test]
    fn parse_subscript() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let sym_table = HashMap::new();

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[1, 2, 3][0]").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(&ctx, &sym_table, &parse_expression("[[1]][0][0]").unwrap());
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][1:2]").unwrap(),
        );
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(LIST_TYPE, vec![PrimitiveType(INT32_TYPE).into()]).into()
        );

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][1:2:2]").unwrap(),
        );
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(LIST_TYPE, vec![PrimitiveType(INT32_TYPE).into()]).into()
        );

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][1:1.2]").unwrap(),
        );
        assert_eq!(result, Err("slice must be int32 type".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][1:none()]").unwrap(),
        );
        assert_eq!(result, Err("slice must have type".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][1.2]").unwrap(),
        );
        assert_eq!(result, Err("index must be either slice or int32".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[1, 2, 3][none()]").unwrap(),
        );
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("none()[1.2]").unwrap());
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(&ctx, &sym_table, &parse_expression("123[1]").unwrap());
        assert_eq!(
            result,
            Err("subscript is not supported for types other than list".into())
        );
    }

    #[test]
    fn test_if_expr() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let sym_table = HashMap::new();

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("1 if True else 0").unwrap(),
        );
        assert_eq!(result.unwrap().unwrap(), PrimitiveType(INT32_TYPE).into());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("none() if True else none()").unwrap(),
        );
        assert_eq!(result.unwrap(), None);

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("none() if 1 else none()").unwrap(),
        );
        assert_eq!(result, Err("test should be bool".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("1 if True else none()").unwrap(),
        );
        assert_eq!(result, Err("divergent type".into()));
    }

    #[test]
    fn test_list_comp() {
        let mut ctx = basic_ctx();
        ctx.add_fn(
            "none",
            FnDef {
                args: vec![],
                result: None,
            },
        );
        let int32 = Rc::new(PrimitiveType(INT32_TYPE));
        let mut sym_table = HashMap::new();
        sym_table.insert("z", int32.clone());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[x for x in [(1, 2), (2, 3), (3, 4)]][0]").unwrap(),
        );
        assert_eq!(
            result.unwrap().unwrap(),
            ParametricType(TUPLE_TYPE, vec![int32.clone(), int32.clone()]).into()
        );

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[x for (x, y) in [(1, 2), (2, 3), (3, 4)]][0]").unwrap(),
        );
        assert_eq!(result.unwrap().unwrap(), int32.clone());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[x for (x, y) in [(1, 2), (2, 3), (3, 4)] if x > 0][0]").unwrap(),
        );
        assert_eq!(result.unwrap().unwrap(), int32.clone());

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[x for (x, y) in [(1, 2), (2, 3), (3, 4)] if x][0]").unwrap(),
        );
        assert_eq!(result, Err("test must be bool".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[y for x in []][0]").unwrap(),
        );
        assert_eq!(result, Err("unbounded variable".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[none() for x in []][0]").unwrap(),
        );
        assert_eq!(result, Err("no value".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[z for z in []][0]").unwrap(),
        );
        assert_eq!(result, Err("duplicated naming".into()));

        let result = parse_expr(
            &ctx,
            &sym_table,
            &parse_expression("[x for x in [] for y in []]").unwrap(),
        );
        assert_eq!(
            result,
            Err("only 1 generator statement is supported".into())
        );
    }
}
