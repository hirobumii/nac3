use super::*;
use crate::{
    codegen::{
        classes::RangeValue,
        expr::destructure_range,
        irrt::*,
        llvm_intrinsics::*,
        stmt::exn_constructor,
    },
    symbol_resolver::SymbolValue,
    toplevel::numpy::*,
};
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    types::{BasicType, BasicMetadataTypeEnum},
    values::{BasicValue, BasicMetadataValueEnum, CallSiteValue},
    FloatPredicate,
    IntPredicate
};
use itertools::Either;

type BuiltinInfo = Vec<(Arc<RwLock<TopLevelDef>>, Option<Stmt>)>;

pub fn get_exn_constructor(
    name: &str,
    class_id: usize,
    cons_id: usize,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore
)-> (TopLevelDef, TopLevelDef, Type, Type) {
    let int32 = primitives.int32;
    let int64 = primitives.int64;
    let string = primitives.str;
    let exception_fields = vec![
        ("__name__".into(), int32, true),
        ("__file__".into(), string, true),
        ("__line__".into(), int32, true),
        ("__col__".into(), int32, true),
        ("__func__".into(), string, true),
        ("__message__".into(), string, true),
        ("__param0__".into(), int64, true),
        ("__param1__".into(), int64, true),
        ("__param2__".into(), int64, true),
    ];
    let exn_cons_args = vec![
        FuncArg {
            name: "msg".into(),
            ty: string,
            default_value: Some(SymbolValue::Str(String::new())),
        },
        FuncArg { name: "param0".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
        FuncArg { name: "param1".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
        FuncArg { name: "param2".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
    ];
    let exn_type = unifier.add_ty(TypeEnum::TObj {
        obj_id: DefinitionId(class_id),
        fields: exception_fields.iter().map(|(a, b, c)| (*a, (*b, *c))).collect(),
        params: HashMap::default(),
    });
    let signature = unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: exn_cons_args,
        ret: exn_type,
        vars: HashMap::default(),
    }));
    let fun_def = TopLevelDef::Function {
        name: format!("{name}.__init__"),
        simple_name: "__init__".into(),
        signature,
        var_id: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(Box::new(exn_constructor)))),
        loc: None,
    };
    let class_def = TopLevelDef::Class {
        name: name.into(),
        object_id: DefinitionId(class_id),
        type_vars: Vec::default(),
        fields: exception_fields,
        methods: vec![("__init__".into(), signature, DefinitionId(cons_id))],
        ancestors: vec![
            TypeAnnotation::CustomClass { id: DefinitionId(class_id), params: Vec::default() },
            TypeAnnotation::CustomClass { id: DefinitionId(7), params: Vec::default() },
        ],
        constructor: Some(signature),
        resolver: None,
        loc: None,
    };
    (fun_def, class_def, signature, exn_type)
}

/// Creates a NumPy [`TopLevelDef`] function by code generation.
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `codegen_callback`: A lambda generating LLVM IR for the implementation of this function.
fn create_fn_by_codegen(
    primitives: &mut (PrimitiveStore, Unifier),
    var_map: &HashMap<u32, Type>,
    name: &'static str,
    ret_ty: Type,
    param_ty: &[(Type, &'static str)],
    codegen_callback: GenCallCallback,
) -> Arc<RwLock<TopLevelDef>> {
    Arc::new(RwLock::new(TopLevelDef::Function {
        name: name.into(),
        simple_name: name.into(),
        signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
            args: param_ty.iter().map(|p| FuncArg {
                name: p.1.into(),
                ty: p.0,
                default_value: None,
            }).collect(),
            ret: ret_ty,
            vars: var_map.clone(),
        })),
        var_id: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(codegen_callback))),
        loc: None,
    }))
}

/// Creates a NumPy [`TopLevelDef`] function using an LLVM intrinsic.
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `intrinsic_fn`: The fully-qualified name of the LLVM intrinsic function.
fn create_fn_by_intrinsic(
    primitives: &mut (PrimitiveStore, Unifier),
    var_map: &HashMap<u32, Type>,
    name: &'static str,
    ret_ty: Type,
    params: &[(Type, &'static str)],
    intrinsic_fn: &'static str,
) -> Arc<RwLock<TopLevelDef>> {
    let param_tys = params.iter()
        .map(|p| p.0)
        .collect_vec();

    create_fn_by_codegen(
        primitives,
        var_map,
        name,
        ret_ty,
        params,
        Box::new(move |ctx, _, fun, args, generator| {
            let args_ty = fun.0.args.iter().map(|a| a.ty).collect_vec();

            assert!(param_tys.iter().zip(&args_ty)
                .all(|(expected, actual)| ctx.unifier.unioned(*expected, *actual)));

            let args_val = args_ty.iter().zip_eq(args.iter())
                .map(|(ty, arg)| {
                    arg.1.clone()
                        .to_basic_value_enum(ctx, generator, *ty)
                        .unwrap()
                })
                .map_into::<BasicMetadataValueEnum>()
                .collect_vec();

            let intrinsic_fn = ctx.module.get_function(intrinsic_fn).unwrap_or_else(|| {
                let ret_llvm_ty = ctx.get_llvm_abi_type(generator, ret_ty);
                let param_llvm_ty = param_tys.iter()
                    .map(|p| ctx.get_llvm_abi_type(generator, *p))
                    .map_into::<BasicMetadataTypeEnum>()
                    .collect_vec();
                let fn_type = ret_llvm_ty.fn_type(param_llvm_ty.as_slice(), false);

                ctx.module.add_function(intrinsic_fn, fn_type, None)
            });

            let val = ctx.builder
                .build_call(intrinsic_fn, args_val.as_slice(), name)
                .map(CallSiteValue::try_as_basic_value)
                .map(Either::unwrap_left)
                .unwrap();
            Ok(val.into())
        }),
    )
}

/// Creates a unary NumPy [`TopLevelDef`] function using an extern function (e.g. from `libc` or
/// `libm`).
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `extern_fn`: The fully-qualified name of the extern function used as the implementation.
/// * `attrs`: The list of attributes to apply to this function declaration. Note that `nounwind` is
/// already implied by the C ABI.
fn create_fn_by_extern(
    primitives: &mut (PrimitiveStore, Unifier),
    var_map: &HashMap<u32, Type>,
    name: &'static str,
    ret_ty: Type,
    params: &[(Type, &'static str)],
    extern_fn: &'static str,
    attrs: &'static [&str],
) -> Arc<RwLock<TopLevelDef>> {
    let param_tys = params.iter()
        .map(|p| p.0)
        .collect_vec();

    create_fn_by_codegen(
        primitives,
        var_map,
        name,
        ret_ty,
        params,
        Box::new(move |ctx, _, fun, args, generator| {
            let args_ty = fun.0.args.iter().map(|a| a.ty).collect_vec();

            assert!(param_tys.iter().zip(&args_ty)
                .all(|(expected, actual)| ctx.unifier.unioned(*expected, *actual)));

            let args_val = args_ty.iter().zip_eq(args.iter())
                .map(|(ty, arg)| {
                    arg.1.clone()
                        .to_basic_value_enum(ctx, generator, *ty)
                        .unwrap()
                })
                .map_into::<BasicMetadataValueEnum>()
                .collect_vec();

                let intrinsic_fn = ctx.module.get_function(extern_fn).unwrap_or_else(|| {
                    let ret_llvm_ty = ctx.get_llvm_abi_type(generator, ret_ty);
                    let param_llvm_ty = param_tys.iter()
                        .map(|p| ctx.get_llvm_abi_type(generator, *p))
                        .map_into::<BasicMetadataTypeEnum>()
                        .collect_vec();
                    let fn_type = ret_llvm_ty.fn_type(param_llvm_ty.as_slice(), false);
                    let func = ctx.module.add_function(extern_fn, fn_type, None);
                    func.add_attribute(
                        AttributeLoc::Function,
                        ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0)
                    );

                    for attr in attrs {
                        func.add_attribute(
                            AttributeLoc::Function,
                            ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0)
                        );
                    }

                    func
                });

                let val = ctx.builder
                    .build_call(intrinsic_fn, &args_val, name)
                    .map(CallSiteValue::try_as_basic_value)
                    .map(Either::unwrap_left)
                    .unwrap();
                Ok(val.into())
        }),
    )
}

pub fn get_builtins(primitives: &mut (PrimitiveStore, Unifier)) -> BuiltinInfo {
    let int32 = primitives.0.int32;
    let int64 = primitives.0.int64;
    let uint32 = primitives.0.uint32;
    let uint64 = primitives.0.uint64;
    let float = primitives.0.float;
    let boolean = primitives.0.bool;
    let range = primitives.0.range;
    let string = primitives.0.str;
    let ndarray = {
        let ndarray_ty = TypeEnum::ndarray(&mut primitives.1, None, None, &primitives.0);
        primitives.1.add_ty(ndarray_ty)
    };
    let ndarray_float = {
        let ndarray_ty_enum = TypeEnum::ndarray(&mut primitives.1, Some(float), None, &primitives.0);
        primitives.1.add_ty(ndarray_ty_enum)
    };
    let ndarray_float_2d = {
        let value = match primitives.0.size_t {
            64 => SymbolValue::U64(2u64),
            32 => SymbolValue::U32(2u32),
            _ => unreachable!(),
        };
        let ndims = primitives.1.add_ty(TypeEnum::TLiteral {
            values: vec![value],
            loc: None,
        });

        primitives.1.add_ty(TypeEnum::TNDArray {
            ty: float,
            ndims,
        })
    };
    let list_int32 = primitives.1.add_ty(TypeEnum::TList { ty: int32 });
    let num_ty = primitives.1.get_fresh_var_with_range(
        &[int32, int64, float, boolean, uint32, uint64],
        Some("N".into()),
        None,
    );
    let var_map: HashMap<_, _> = vec![(num_ty.1, num_ty.0)].into_iter().collect();
    let exception_fields = vec![
        ("__name__".into(), int32, true),
        ("__file__".into(), string, true),
        ("__line__".into(), int32, true),
        ("__col__".into(), int32, true),
        ("__func__".into(), string, true),
        ("__message__".into(), string, true),
        ("__param0__".into(), int64, true),
        ("__param1__".into(), int64, true),
        ("__param2__".into(), int64, true),
    ];

    // for Option, is_some and is_none share the same type: () -> bool,
    // and they are methods under the same class `Option`
    let (is_some_ty, unwrap_ty, (option_ty_var, option_ty_var_id)) =
        if let TypeEnum::TObj { fields, params, .. } =
            primitives.1.get_ty(primitives.0.option).as_ref()
        {
            (
                *fields.get(&"is_some".into()).unwrap(),
                *fields.get(&"unwrap".into()).unwrap(),
                (*params.iter().next().unwrap().1, *params.iter().next().unwrap().0),
            )
        } else {
            unreachable!()
        };
    let top_level_def_list = vec![
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            0,
            None,
            "int32".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            1,
            None,
            "int64".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            2,
            None,
            "float".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            3,
            None,
            "bool".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            4,
            None,
            "none".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            5,
            None,
            "range".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            6,
            None,
            "str".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelDef::Class {
            name: "Exception".into(),
            object_id: DefinitionId(7),
            type_vars: Vec::default(),
            fields: exception_fields,
            methods: Vec::default(),
            ancestors: vec![],
            constructor: None,
            resolver: None,
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            8,
            None,
            "uint32".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            9,
            None,
            "uint64".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new({
            TopLevelDef::Class {
                name: "Option".into(),
                object_id: DefinitionId(10),
                type_vars: vec![option_ty_var],
                fields: vec![],
                methods: vec![
                    ("is_some".into(), is_some_ty.0, DefinitionId(11)),
                    ("is_none".into(), is_some_ty.0, DefinitionId(12)),
                    ("unwrap".into(), unwrap_ty.0, DefinitionId(13)),
                ],
                ancestors: vec![TypeAnnotation::CustomClass {
                    id: DefinitionId(10),
                    params: Vec::default(),
                }],
                constructor: None,
                resolver: None,
                loc: None,
            }
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "Option.is_some".into(),
            simple_name: "is_some".into(),
            signature: is_some_ty.0,
            var_id: vec![option_ty_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, obj, _, _, generator| {
                    let expect_ty = obj.clone().unwrap().0;
                    let obj_val = obj.unwrap().1.clone().to_basic_value_enum(
                        ctx,
                        generator,
                        expect_ty,
                    )?;
                    let BasicValueEnum::PointerValue(ptr) = obj_val else {
                        unreachable!("option must be ptr")
                    };

                    Ok(Some(ctx.builder
                        .build_is_not_null(ptr, "is_some")
                        .map(Into::into)
                        .unwrap()
                    ))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "Option.is_none".into(),
            simple_name: "is_none".into(),
            signature: is_some_ty.0,
            var_id: vec![option_ty_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, obj, _, _, generator| {
                    let expect_ty = obj.clone().unwrap().0;
                    let obj_val = obj.unwrap().1.clone().to_basic_value_enum(
                        ctx,
                        generator,
                        expect_ty,
                    )?;
                    let BasicValueEnum::PointerValue(ptr) = obj_val else {
                        unreachable!("option must be ptr")
                    };

                    Ok(Some(ctx.builder
                        .build_is_null(ptr, "is_none")
                        .map(Into::into)
                        .unwrap()
                    ))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "Option.unwrap".into(),
            simple_name: "unwrap".into(),
            signature: unwrap_ty.0,
            var_id: vec![option_ty_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |_, _, _, _, _| {
                    unreachable!("handled in gen_expr")
                },
            )))),
            loc: None,
        })),
        {
            let tvar = primitives.1.get_fresh_var(Some("T".into()), None);
            let ndims = primitives.1.get_fresh_const_generic_var(primitives.0.uint64, Some("N".into()), None);

            Arc::new(RwLock::new(TopLevelDef::Class {
                name: "ndarray".into(),
                object_id: DefinitionId(14),
                type_vars: vec![tvar.0, ndims.0],
                fields: Vec::default(),
                methods: Vec::default(),
                ancestors: Vec::default(),
                constructor: None,
                resolver: None,
                loc: None,
            }))
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "int32".into(),
            simple_name: "int32".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: int32,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let PrimitiveStore {
                        int32,
                        int64,
                        uint32,
                        uint64,
                        float,
                        bool: boolean,
                        ..
                    } = ctx.primitives;

                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    Ok(if ctx.unifier.unioned(arg_ty, boolean) {
                        Some(
                            ctx.builder
                                .build_int_z_extend(
                                    arg.into_int_value(),
                                    ctx.ctx.i32_type(),
                                    "zext",
                                )
                                .map(Into::into)
                                .unwrap(),
                        )
                    } else if ctx.unifier.unioned(arg_ty, int32)
                        || ctx.unifier.unioned(arg_ty, uint32)
                    {
                        Some(arg)
                    } else if ctx.unifier.unioned(arg_ty, int64)
                        || ctx.unifier.unioned(arg_ty, uint64)
                    {
                        Some(
                            ctx.builder
                                .build_int_truncate(
                                    arg.into_int_value(),
                                    ctx.ctx.i32_type(),
                                    "trunc",
                                )
                                .map(Into::into)
                                .unwrap(),
                        )
                    } else if ctx.unifier.unioned(arg_ty, float) {
                        let to_int64 = ctx
                            .builder
                            .build_float_to_signed_int(
                                arg.into_float_value(),
                                ctx.ctx.i64_type(),
                                "",
                            )
                            .unwrap();
                        let val = ctx.builder
                            .build_int_truncate(to_int64, ctx.ctx.i32_type(), "conv")
                            .unwrap();

                        Some(val.into())
                    } else {
                        unreachable!()
                    })
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "int64".into(),
            simple_name: "int64".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: int64,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let PrimitiveStore {
                        int32,
                        int64,
                        uint32,
                        uint64,
                        float,
                        bool: boolean,
                        ..
                    } = ctx.primitives;

                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    Ok(
                        if ctx.unifier.unioned(arg_ty, boolean)
                            || ctx.unifier.unioned(arg_ty, uint32)
                        {
                            Some(
                                ctx.builder
                                    .build_int_z_extend(
                                        arg.into_int_value(),
                                        ctx.ctx.i64_type(),
                                        "zext",
                                    )
                                    .map(Into::into)
                                    .unwrap(),
                            )
                        } else if ctx.unifier.unioned(arg_ty, int32) {
                            Some(
                                ctx.builder
                                    .build_int_s_extend(
                                        arg.into_int_value(),
                                        ctx.ctx.i64_type(),
                                        "sext",
                                    )
                                    .map(Into::into)
                                    .unwrap(),
                            )
                        } else if ctx.unifier.unioned(arg_ty, int64)
                            || ctx.unifier.unioned(arg_ty, uint64)
                        {
                            Some(arg)
                        } else if ctx.unifier.unioned(arg_ty, float) {
                            let val = ctx
                                .builder
                                .build_float_to_signed_int(
                                    arg.into_float_value(),
                                    ctx.ctx.i64_type(),
                                    "fptosi",
                                )
                                .map(Into::into)
                                .unwrap();
                            Some(val)
                        } else {
                            unreachable!()
                        },
                    )
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "uint32".into(),
            simple_name: "uint32".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: uint32,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let PrimitiveStore {
                        int32,
                        int64,
                        uint32,
                        uint64,
                        float,
                        bool: boolean,
                        ..
                    } = ctx.primitives;

                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    let res = if ctx.unifier.unioned(arg_ty, boolean) {
                        ctx.builder
                            .build_int_z_extend(arg.into_int_value(), ctx.ctx.i64_type(), "zext")
                            .map(Into::into)
                            .unwrap()
                    } else if ctx.unifier.unioned(arg_ty, int32)
                        || ctx.unifier.unioned(arg_ty, uint32)
                    {
                        arg
                    } else if ctx.unifier.unioned(arg_ty, int64)
                        || ctx.unifier.unioned(arg_ty, uint64)
                    {
                        ctx.builder
                            .build_int_truncate(arg.into_int_value(), ctx.ctx.i32_type(), "trunc")
                            .map(Into::into)
                            .unwrap()
                    } else if ctx.unifier.unioned(arg_ty, float) {
                        let llvm_i32 = ctx.ctx.i32_type();
                        let llvm_i64 = ctx.ctx.i64_type();

                        let arg = arg.into_float_value();
                        let arg_gez = ctx.builder
                            .build_float_compare(
                                FloatPredicate::OGE,
                                arg,
                                arg.get_type().const_zero(),
                                "",
                            )
                            .unwrap();

                        let to_int32 = ctx.builder
                            .build_float_to_signed_int(
                                arg,
                                llvm_i32,
                                "",
                            )
                            .unwrap();
                        let to_uint64 = ctx.builder
                            .build_float_to_unsigned_int(
                                arg,
                                llvm_i64,
                                "",
                            )
                            .unwrap();

                        let val = ctx.builder
                            .build_select(
                                arg_gez,
                                ctx.builder.build_int_truncate(to_uint64, llvm_i32, "").unwrap(),
                                to_int32,
                                "conv",
                            )
                            .unwrap();

                        val
                    } else {
                        unreachable!()
                    };
                    Ok(Some(res))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "uint64".into(),
            simple_name: "uint64".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: uint64,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let PrimitiveStore {
                        int32,
                        int64,
                        uint32,
                        uint64,
                        float,
                        bool: boolean,
                        ..
                    } = ctx.primitives;

                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    let res = if ctx.unifier.unioned(arg_ty, uint32)
                        || ctx.unifier.unioned(arg_ty, boolean)
                    {
                        ctx.builder
                            .build_int_z_extend(arg.into_int_value(), ctx.ctx.i64_type(), "zext")
                            .map(Into::into)
                            .unwrap()
                    } else if ctx.unifier.unioned(arg_ty, int32) {
                        ctx.builder
                            .build_int_s_extend(arg.into_int_value(), ctx.ctx.i64_type(), "sext")
                            .map(Into::into)
                            .unwrap()
                    } else if ctx.unifier.unioned(arg_ty, int64)
                        || ctx.unifier.unioned(arg_ty, uint64)
                    {
                        arg
                    } else if ctx.unifier.unioned(arg_ty, float) {
                        let llvm_i64 = ctx.ctx.i64_type();

                        let arg = arg.into_float_value();
                        let arg_gez = ctx.builder
                            .build_float_compare(
                                FloatPredicate::OGE,
                                arg,
                                arg.get_type().const_zero(),
                                "",
                            )
                            .unwrap();

                        let to_int64 = ctx.builder
                            .build_float_to_signed_int(arg, llvm_i64, "")
                            .unwrap();
                        let to_uint64 = ctx.builder
                            .build_float_to_unsigned_int(arg, llvm_i64, "")
                            .unwrap();

                        let val = ctx.builder
                            .build_select(arg_gez, to_uint64, to_int64, "conv")
                            .unwrap();

                        val
                    } else {
                        unreachable!()
                    };
                    Ok(Some(res))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "float".into(),
            simple_name: "float".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: float,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let PrimitiveStore {
                        int32,
                        int64,
                        uint32,
                        uint64,
                        float,
                        bool: boolean,
                        ..
                    } = ctx.primitives;

                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    Ok(
                        if [boolean, int32, int64].iter().any(|ty| ctx.unifier.unioned(arg_ty, *ty))
                        {
                            let arg = arg.into_int_value();
                            let val = ctx
                                .builder
                                .build_signed_int_to_float(arg, ctx.ctx.f64_type(), "sitofp")
                                .map(Into::into)
                                .unwrap();
                            Some(val)
                        } else if [uint32, uint64].iter().any(|ty| ctx.unifier.unioned(arg_ty, *ty)) {
                            let arg = arg.into_int_value();
                            let val = ctx
                                .builder
                                .build_unsigned_int_to_float(arg, ctx.ctx.f64_type(), "uitofp")
                                .map(Into::into)
                                .unwrap();
                            Some(val)
                        } else if ctx.unifier.unioned(arg_ty, float) {
                            Some(arg)
                        } else {
                            unreachable!()
                        },
                    )
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_ndarray",
            ndarray_float,
            // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
            // type variable
            &[(list_int32, "shape")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_empty(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_empty",
            ndarray_float,
            // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
            // type variable
            &[(list_int32, "shape")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_empty(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_zeros",
            ndarray_float,
            // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
            // type variable
            &[(list_int32, "shape")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_zeros(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_ones",
            ndarray_float,
            // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
            // type variable
            &[(list_int32, "shape")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_ones(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        {
            let tv = primitives.1.get_fresh_var(Some("T".into()), None).0;

            create_fn_by_codegen(
                primitives,
                &var_map,
                "np_full",
                ndarray,
                // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
                // type variable
                &[(list_int32, "shape"), (tv, "fill_value")],
                Box::new(|ctx, obj, fun, args, generator| {
                    gen_ndarray_full(ctx, &obj, fun, &args, generator)
                        .map(|val| Some(val.as_basic_value_enum()))
                }),
            )
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "np_eye".into(),
            simple_name: "np_eye".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "N".into(), ty: int32, default_value: None },
                    // TODO(Derppening): Default values current do not work?
                    FuncArg {
                        name: "M".into(),
                        ty: int32,
                        default_value: Some(SymbolValue::OptionNone)
                    },
                    FuncArg { name: "k".into(), ty: int32, default_value: Some(SymbolValue::I32(0)) },
                ],
                ret: ndarray_float_2d,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, obj, fun, args, generator| {
                    gen_ndarray_eye(ctx, &obj, fun, &args, generator)
                        .map(|val| Some(val.as_basic_value_enum()))
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_identity",
            ndarray_float_2d,
            &[(int32, "n")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_identity(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "round",
            int32,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i32 = ctx.ctx.i32_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_round(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i32, "round")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "round64",
            int64,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i64 = ctx.ctx.i64_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_round(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i64, "round")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_round",
            float,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_roundeven(ctx, arg, None);

                Ok(Some(val.into()))
            }),
        ),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "range".into(),
            simple_name: "range".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "start".into(), ty: int32, default_value: None },
                    FuncArg {
                        name: "stop".into(),
                        ty: int32,
                        // placeholder
                        default_value: Some(SymbolValue::I32(0)),
                    },
                    FuncArg {
                        name: "step".into(),
                        ty: int32,
                        default_value: Some(SymbolValue::I32(1)),
                    },
                ],
                ret: range,
                vars: HashMap::default(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, _, args, generator| {
                    let mut start = None;
                    let mut stop = None;
                    let mut step = None;
                    let int32 = ctx.ctx.i32_type();
                    let zero = int32.const_zero();
                    let ty_i32 = ctx.primitives.int32;
                    for (i, arg) in args.iter().enumerate() {
                        if arg.0 == Some("start".into()) {
                            start = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        } else if arg.0 == Some("stop".into()) {
                            stop = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        } else if arg.0 == Some("step".into()) {
                            step = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        } else if i == 0 {
                            start = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        } else if i == 1 {
                            stop = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        } else if i == 2 {
                            step = Some(arg.1.clone().to_basic_value_enum(ctx, generator, ty_i32)?);
                        }
                    }
                    let step = match step {
                        Some(step) => {
                            let step = step.into_int_value();
                            // assert step != 0, throw exception if not
                            let not_zero = ctx.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    step,
                                    step.get_type().const_zero(),
                                    "range_step_ne",
                                )
                                .unwrap();
                            ctx.make_assert(
                                generator,
                                not_zero,
                                "0:ValueError",
                                "range() step must not be zero",
                                [None, None, None],
                                ctx.current_loc,
                            );
                            step
                        }
                        None => int32.const_int(1, false),
                    };
                    let stop = stop.unwrap_or_else(|| {
                        let v = start.unwrap();
                        start = None;
                        v
                    });
                    let start = start.unwrap_or_else(|| int32.const_zero().into());
                    let ty = int32.array_type(3);
                    let ptr = generator.gen_var_alloc(ctx, ty.into(), Some("range")).unwrap();
                    unsafe {
                        let a = ctx.builder
                            .build_in_bounds_gep(ptr, &[zero, zero], "start")
                            .unwrap();
                        let b = ctx.builder
                            .build_in_bounds_gep(
                                ptr,
                                &[zero, int32.const_int(1, false)],
                                "end",
                            )
                            .unwrap();
                        let c = ctx.builder.build_in_bounds_gep(
                                ptr,
                                &[zero, int32.const_int(2, false)],
                                "step",
                            ).unwrap();
                        ctx.builder.build_store(a, start).unwrap();
                        ctx.builder.build_store(b, stop).unwrap();
                        ctx.builder.build_store(c, step).unwrap();
                    }
                    Ok(Some(ptr.into()))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "str".into(),
            simple_name: "str".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "s".into(), ty: string, default_value: None }],
                ret: string,
                vars: HashMap::default(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    Ok(Some(args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "bool".into(),
            simple_name: "bool".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: primitives.0.bool,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let int32 = ctx.primitives.int32;
                    let int64 = ctx.primitives.int64;
                    let float = ctx.primitives.float;
                    let boolean = ctx.primitives.bool;
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    Ok(if ctx.unifier.unioned(arg_ty, boolean) {
                        Some(arg)
                    } else if ctx.unifier.unioned(arg_ty, int32) {
                        Some(
                            ctx.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    ctx.ctx.i32_type().const_zero(),
                                    arg.into_int_value(),
                                    "bool",
                                )
                                .map(Into::into)
                                .unwrap(),
                        )
                    } else if ctx.unifier.unioned(arg_ty, int64) {
                        Some(
                            ctx.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    ctx.ctx.i64_type().const_zero(),
                                    arg.into_int_value(),
                                    "bool",
                                )
                                .map(Into::into)
                                .unwrap(),
                        )
                    } else if ctx.unifier.unioned(arg_ty, float) {
                        let val = ctx
                            .builder
                            .build_float_compare(
                                // UEQ as bool(nan) is True
                                FloatPredicate::UEQ,
                                arg.into_float_value(),
                                ctx.ctx.f64_type().const_zero(),
                                "bool",
                            )
                            .map(Into::into)
                            .unwrap();
                        Some(val)
                    } else {
                        unreachable!()
                    })
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "floor",
            int32,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i32 = ctx.ctx.i32_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_floor(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i32, "floor")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "floor64",
            int64,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i64 = ctx.ctx.i64_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_floor(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i64, "floor")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_floor",
            float,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_floor(ctx, arg, None);
                Ok(Some(val.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "ceil",
            int32,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i32 = ctx.ctx.i32_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_ceil(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i32, "ceil")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "ceil64",
            int64,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let llvm_i64 = ctx.ctx.i64_type();

                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_ceil(ctx, arg, None);
                let val_toint = ctx.builder
                    .build_float_to_signed_int(val, llvm_i64, "ceil")
                    .unwrap();
                Ok(Some(val_toint.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_ceil",
            float,
            &[(float, "n")],
            Box::new(|ctx, _, _, args, generator| {
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, ctx.primitives.float)?
                    .into_float_value();

                let val = call_float_ceil(ctx, arg, None);
                Ok(Some(val.into()))
            }),
        ),
        Arc::new(RwLock::new({
            let tvar = primitives.1.get_fresh_var(Some("L".into()), None);
            let list = primitives.1.add_ty(TypeEnum::TList { ty: tvar.0 });
            let ndims = primitives.1.get_fresh_const_generic_var(primitives.0.uint64, Some("N".into()), None);
            let ndarray = primitives.1.add_ty(TypeEnum::TNDArray { ty: tvar.0, ndims: ndims.0 });

            let arg_ty = primitives.1.get_fresh_var_with_range(
                &[list, ndarray, primitives.0.range],
                Some("I".into()),
                None,
            );
            TopLevelDef::Function {
                name: "len".into(),
                simple_name: "len".into(),
                signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                    args: vec![FuncArg { name: "ls".into(), ty: arg_ty.0, default_value: None }],
                    ret: int32,
                    vars: vec![(tvar.1, tvar.0), (arg_ty.1, arg_ty.0)]
                        .into_iter()
                        .collect(),
                })),
                var_id: vec![arg_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                    |ctx, _, fun, args, generator| {
                        let range_ty = ctx.primitives.range;
                        let arg_ty = fun.0.args[0].ty;
                        let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                        Ok(if ctx.unifier.unioned(arg_ty, range_ty) {
                            let arg = RangeValue::from_ptr_val(arg.into_pointer_value(), Some("range"));
                            let (start, end, step) = destructure_range(ctx, arg);
                            Some(calculate_len_for_slice_range(generator, ctx, start, end, step).into())
                        } else {
                            match &*ctx.unifier.get_ty_immutable(arg_ty) {
                                TypeEnum::TList { .. } => {
                                    let int32 = ctx.ctx.i32_type();
                                    let zero = int32.const_zero();
                                    let len = ctx
                                        .build_gep_and_load(
                                            arg.into_pointer_value(),
                                            &[zero, int32.const_int(1, false)],
                                            None,
                                        )
                                        .into_int_value();
                                    if len.get_type().get_bit_width() == 32 {
                                        Some(len.into())
                                    } else {
                                        Some(ctx.builder
                                            .build_int_truncate(len, int32, "len2i32")
                                            .map(Into::into)
                                            .unwrap()
                                        )
                                    }
                                }
                                TypeEnum::TNDArray { .. } => {
                                    let llvm_i32 = ctx.ctx.i32_type();
                                    let i32_zero = llvm_i32.const_zero();

                                    let len = ctx.build_gep_and_load(
                                        arg.into_pointer_value(),
                                        &[i32_zero, i32_zero],
                                        None,
                                    ).into_int_value();

                                    if len.get_type().get_bit_width() == 32 {
                                        Some(len.into())
                                    } else {
                                       Some(ctx.builder
                                           .build_int_truncate(len, llvm_i32, "len")
                                           .map(Into::into)
                                           .unwrap()
                                       )
                                    }
                                }
                                _ => unreachable!(),
                            }
                        })
                    },
                )))),
                loc: None,
            }
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "min".into(),
            simple_name: "min".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "m".into(), ty: num_ty.0, default_value: None },
                    FuncArg { name: "n".into(), ty: num_ty.0, default_value: None },
                ],
                ret: num_ty.0,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let boolean = ctx.primitives.bool;
                    let int32 = ctx.primitives.int32;
                    let int64 = ctx.primitives.int64;
                    let uint32 = ctx.primitives.uint32;
                    let uint64 = ctx.primitives.uint64;
                    let float = ctx.primitives.float;
                    let m_ty = fun.0.args[0].ty;
                    let n_ty = fun.0.args[1].ty;
                    let m_val = args[0].1.clone().to_basic_value_enum(ctx, generator, m_ty)?;
                    let n_val = args[1].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;
                    let mut is_type = |a: Type, b: Type| ctx.unifier.unioned(a, b);
                    if !is_type(m_ty, n_ty) {
                        unreachable!()
                    }
                    let val: BasicValueEnum = if [boolean, uint32, uint64].iter().any(|t| is_type(n_ty, *t)) {
                        call_int_umin(
                            ctx,
                            m_val.into_int_value(),
                            n_val.into_int_value(),
                            Some("min"),
                        ).into()
                    } else if [int32, int64].iter().any(|t| is_type(n_ty, *t)) {
                        call_int_smin(
                            ctx,
                            m_val.into_int_value(),
                            n_val.into_int_value(),
                            Some("min"),
                        ).into()
                    } else if is_type(m_ty, n_ty) && is_type(n_ty, float) {
                        call_float_minnum(
                            ctx,
                            m_val.into_float_value(),
                            n_val.into_float_value(),
                            Some("min"),
                        ).into()
                    } else {
                        unreachable!()
                    };
                    Ok(val.into())
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "max".into(),
            simple_name: "max".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "m".into(), ty: num_ty.0, default_value: None },
                    FuncArg { name: "n".into(), ty: num_ty.0, default_value: None },
                ],
                ret: num_ty.0,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let boolean = ctx.primitives.bool;
                    let int32 = ctx.primitives.int32;
                    let int64 = ctx.primitives.int64;
                    let uint32 = ctx.primitives.uint32;
                    let uint64 = ctx.primitives.uint64;
                    let float = ctx.primitives.float;
                    let m_ty = fun.0.args[0].ty;
                    let n_ty = fun.0.args[1].ty;
                    let m_val = args[0].1.clone().to_basic_value_enum(ctx, generator, m_ty)?;
                    let n_val = args[1].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;
                    let mut is_type = |a: Type, b: Type| ctx.unifier.unioned(a, b);
                    if !is_type(m_ty, n_ty) {
                        unreachable!()
                    }
                    let val: BasicValueEnum = if [boolean, uint32, uint64].iter().any(|t| is_type(n_ty, *t)) {
                        call_int_umax(
                            ctx,
                            m_val.into_int_value(),
                            n_val.into_int_value(),
                            Some("max"),
                        ).into()
                    } else if [int32, int64].iter().any(|t| is_type(n_ty, *t)) {
                        call_int_smax(
                            ctx,
                            m_val.into_int_value(),
                            n_val.into_int_value(),
                            Some("max"),
                        ).into()
                    } else if is_type(m_ty, n_ty) && is_type(n_ty, float) {
                        call_float_maxnum(
                            ctx,
                            m_val.into_float_value(),
                            n_val.into_float_value(),
                            Some("max"),
                        ).into()
                    } else {
                        unreachable!()
                    };
                    Ok(val.into())
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "abs".into(),
            simple_name: "abs".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_ty.0, default_value: None }],
                ret: num_ty.0,
                vars: var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let boolean = ctx.primitives.bool;
                    let int32 = ctx.primitives.int32;
                    let int64 = ctx.primitives.int64;
                    let uint32 = ctx.primitives.uint32;
                    let uint64 = ctx.primitives.uint64;
                    let float = ctx.primitives.float;
                    let llvm_i1 = ctx.ctx.bool_type();
                    let n_ty = fun.0.args[0].ty;
                    let n_val = args[0].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;
                    let mut is_type = |a: Type, b: Type| ctx.unifier.unioned(a, b);
                    let val: BasicValueEnum = if [boolean, uint32, uint64].iter().any(|t| is_type(n_ty, *t)) {
                        n_val
                    } else if [int32, int64].iter().any(|t| is_type(n_ty, *t)) {
                        call_int_abs(
                            ctx,
                            n_val.into_int_value(),
                            llvm_i1.const_zero(),
                            Some("abs"),
                        ).into()
                    } else if is_type(n_ty, float) {
                        call_float_fabs(
                            ctx,
                            n_val.into_float_value(),
                            Some("abs"),
                        ).into()
                    } else {
                        unreachable!()
                    };
                    Ok(val.into())
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_isnan",
            boolean,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let float = ctx.primitives.float;

                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                assert!(ctx.unifier.unioned(x_ty, float));

                let val = call_isnan(generator, ctx, x_val.into_float_value());

                Ok(Some(val.into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "np_isinf",
            boolean,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let float = ctx.primitives.float;

                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                assert!(ctx.unifier.unioned(x_ty, float));

                let val = call_isinf(generator, ctx, x_val.into_float_value());

                Ok(Some(val.into()))
            }),
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_sin",
            float,
            &[(float, "x")],
            "llvm.sin.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_cos",
            float,
            &[(float, "x")],
            "llvm.cos.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_exp",
            float,
            &[(float, "x")],
            "llvm.exp.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_exp2",
            float,
            &[(float, "x")],
            "llvm.exp2.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_log",
            float,
            &[(float, "x")],
            "llvm.log.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_log10",
            float,
            &[(float, "x")],
            "llvm.log10.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_log2",
            float,
            &[(float, "x")],
            "llvm.log2.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_fabs",
            float,
            &[(float, "x")],
            "llvm.fabs.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_sqrt",
            float,
            &[(float, "x")],
            "llvm.sqrt.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_rint",
            float,
            &[(float, "x")],
            "llvm.roundeven.f64",
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_tan",
            float,
            &[(float, "x")],
            "tan",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arcsin",
            float,
            &[(float, "x")],
            "asin",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arccos",
            float,
            &[(float, "x")],
            "acos",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arctan",
            float,
            &[(float, "x")],
            "atan",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_sinh",
            float,
            &[(float, "x")],
            "sinh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_cosh",
            float,
            &[(float, "x")],
            "cosh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_tanh",
            float,
            &[(float, "x")],
            "tanh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arcsinh",
            float,
            &[(float, "x")],
            "asinh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arccosh",
            float,
            &[(float, "x")],
            "acosh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arctanh",
            float,
            &[(float, "x")],
            "atanh",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_expm1",
            float,
            &[(float, "x")],
            "expm1",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_cbrt",
            float,
            &[(float, "x")],
            "cbrt",
            &["readnone", "willreturn"],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "sp_spec_erf",
            float,
            &[(float, "z")],
            "erf",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "sp_spec_erfc",
            float,
            &[(float, "x")],
            "erfc",
            &[],
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "sp_spec_gamma",
            float,
            &[(float, "z")],
            Box::new(|ctx, _, fun, args, generator| {
                let float = ctx.primitives.float;

                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                assert!(ctx.unifier.unioned(z_ty, float));

                Ok(Some(call_gamma(ctx, z_val.into_float_value()).into()))
            }
        )),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "sp_spec_gammaln",
            float,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let float = ctx.primitives.float;

                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                assert!(ctx.unifier.unioned(z_ty, float));

                Ok(Some(call_gammaln(ctx, z_val.into_float_value()).into()))
            }),
        ),
        create_fn_by_codegen(
            primitives,
            &var_map,
            "sp_spec_j0",
            float,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let float = ctx.primitives.float;

                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                assert!(ctx.unifier.unioned(z_ty, float));

                Ok(Some(call_j0(ctx, z_val.into_float_value()).into()))
            }),
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "sp_spec_j1",
            float,
            &[(float, "x")],
            "j1",
            &[],
        ),
        // Not mapped: jv/yv, libm only supports integer orders.
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_arctan2",
            float,
            &[(float, "x1"), (float, "x2")],
            "atan2",
            &[],
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_copysign",
            float,
            &[(float, "x1"), (float, "x2")],
            "llvm.copysign.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_fmax",
            float,
            &[(float, "x1"), (float, "x2")],
            "llvm.maxnum.f64",
        ),
        create_fn_by_intrinsic(
            primitives,
            &var_map,
            "np_fmin",
            float,
            &[(float, "x1"), (float, "x2")],
            "llvm.minnum.f64",
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_ldexp",
            float,
            &[(float, "x1"), (int32, "x2")],
            "ldexp",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_hypot",
            float,
            &[(float, "x1"), (float, "x2")],
            "hypot",
            &[],
        ),
        create_fn_by_extern(
            primitives,
            &var_map,
            "np_nextafter",
            float,
            &[(float, "x1"), (float, "x2")],
            "nextafter",
            &[],
        ),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "Some".into(),
            simple_name: "Some".into(),
            signature: primitives.1.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: option_ty_var, default_value: None }],
                ret: primitives.0.option,
                vars: HashMap::from([(option_ty_var_id, option_ty_var)]),
            })),
            var_id: vec![option_ty_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg_val = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    let alloca = generator.gen_var_alloc(ctx, arg_val.get_type(), Some("alloca_some")).unwrap();
                    ctx.builder.build_store(alloca, arg_val).unwrap();
                    Ok(Some(alloca.into()))
                },
            )))),
            loc: None,
        })),
    ];

    let ast_list: Vec<Option<Stmt<()>>> =
        (0..top_level_def_list.len()).map(|_| None).collect();

    izip!(top_level_def_list, ast_list).collect_vec()
}
