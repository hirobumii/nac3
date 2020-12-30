use super::typedef::{Type::*, *};
use std::collections::HashMap;
use std::rc::Rc;

pub const TUPLE_TYPE: ParamId = ParamId(0);
pub const LIST_TYPE: ParamId = ParamId(1);

pub const BOOL_TYPE: PrimitiveId = PrimitiveId(0);
pub const INT32_TYPE: PrimitiveId = PrimitiveId(1);
pub const INT64_TYPE: PrimitiveId = PrimitiveId(2);
pub const FLOAT_TYPE: PrimitiveId = PrimitiveId(3);

fn impl_math(def: &mut TypeDef, ty: &Rc<Type>) {
    let result = Some(ty.clone());
    let fun = FnDef {
        args: vec![ty.clone()],
        result: result.clone(),
    };
    def.methods.insert("__add__", fun.clone());
    def.methods.insert("__sub__", fun.clone());
    def.methods.insert("__mul__", fun.clone());
    def.methods.insert("__neg__", FnDef {
        args: vec![],
        result
    });
    def.methods.insert(
        "__truediv__",
        FnDef {
            args: vec![ty.clone()],
            result: Some(PrimitiveType(FLOAT_TYPE).into()),
        },
    );
    def.methods.insert("__floordiv__", fun.clone());
    def.methods.insert("__mod__", fun.clone());
    def.methods.insert("__pow__", fun.clone());
}

fn impl_bits(def: &mut TypeDef, ty: &Rc<Type>) {
    let result = Some(ty.clone());
    let fun = FnDef {
        args: vec![PrimitiveType(INT32_TYPE).into()],
        result,
    };

    def.methods.insert("__lshift__", fun.clone());
    def.methods.insert("__rshift__", fun.clone());
    def.methods.insert(
        "__xor__",
        FnDef {
            args: vec![ty.clone()],
            result: Some(ty.clone()),
        },
    );
}

fn impl_eq(def: &mut TypeDef, ty: &Rc<Type>) {
    let fun = FnDef {
        args: vec![ty.clone()],
        result: Some(PrimitiveType(BOOL_TYPE).into()),
    };

    def.methods.insert("__eq__", fun.clone());
    def.methods.insert("__ne__", fun.clone());
}

fn impl_order(def: &mut TypeDef, ty: &Rc<Type>) {
    let fun = FnDef {
        args: vec![ty.clone()],
        result: Some(PrimitiveType(BOOL_TYPE).into()),
    };

    def.methods.insert("__lt__", fun.clone());
    def.methods.insert("__gt__", fun.clone());
    def.methods.insert("__le__", fun.clone());
    def.methods.insert("__ge__", fun.clone());
}

pub fn basic_ctx() -> GlobalContext<'static> {
    let primitives = [
        TypeDef {
            name: "bool",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
        TypeDef {
            name: "int32",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
        TypeDef {
            name: "int64",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
        TypeDef {
            name: "float",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
    ]
    .to_vec();
    let mut ctx = GlobalContext::new(primitives);

    let b_def = ctx.get_primitive_mut(BOOL_TYPE);
    let b = PrimitiveType(BOOL_TYPE).into();
    impl_eq(b_def, &b);
    let int32_def = ctx.get_primitive_mut(INT32_TYPE);
    let int32 = PrimitiveType(INT32_TYPE).into();
    impl_math(int32_def, &int32);
    impl_bits(int32_def, &int32);
    impl_order(int32_def, &int32);
    impl_eq(int32_def, &int32);
    let int64_def = ctx.get_primitive_mut(INT64_TYPE);
    let int64 = PrimitiveType(INT64_TYPE).into();
    impl_math(int64_def, &int64);
    impl_bits(int64_def, &int64);
    impl_order(int64_def, &int64);
    impl_eq(int64_def, &int64);
    let float_def = ctx.get_primitive_mut(FLOAT_TYPE);
    let float = PrimitiveType(FLOAT_TYPE).into();
    impl_math(float_def, &float);
    impl_order(float_def, &float);
    impl_eq(float_def, &float);

    let t = ctx.add_variable_private(VarDef {
        name: "T",
        bound: vec![],
    });

    ctx.add_parametric(ParametricDef {
        base: TypeDef {
            name: "tuple",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
        // we have nothing for tuple, so no param def
        params: vec![],
    });

    ctx.add_parametric(ParametricDef {
        base: TypeDef {
            name: "list",
            fields: HashMap::new(),
            methods: HashMap::new(),
        },
        params: vec![t],
    });

    let i = ctx.add_variable_private(VarDef {
        name: "I",
        bound: vec![
            PrimitiveType(INT32_TYPE).into(),
            PrimitiveType(INT64_TYPE).into(),
            PrimitiveType(FLOAT_TYPE).into(),
        ],
    });
    let args = vec![TypeVariable(i).into()];
    ctx.add_fn(
        "int32",
        FnDef {
            args: args.clone(),
            result: Some(PrimitiveType(INT32_TYPE).into()),
        },
    );
    ctx.add_fn(
        "int64",
        FnDef {
            args: args.clone(),
            result: Some(PrimitiveType(INT64_TYPE).into()),
        },
    );
    ctx.add_fn(
        "float",
        FnDef {
            args: args.clone(),
            result: Some(PrimitiveType(FLOAT_TYPE).into()),
        },
    );

    ctx
}
