use std::iter::once;

use indexmap::IndexMap;
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    IntPredicate,
    types::{BasicMetadataTypeEnum, BasicType},
    values::{BasicMetadataValueEnum, BasicValue, CallSiteValue}
};
use itertools::Either;

use crate::{
    codegen::{
        builtin_fns,
        classes::{ArrayLikeValue, NDArrayValue, RangeValue, TypedArrayLikeAccessor},
        expr::destructure_range,
        irrt::*,
        numpy::*,
        stmt::exn_constructor,
    },
    symbol_resolver::SymbolValue,
    toplevel::{
        helper::PRIMITIVE_DEF_IDS,
        numpy::make_ndarray_ty,
    },
    typecheck::typedef::VarMap,
};

use super::*;

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
        params: VarMap::default(),
    });
    let signature = unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: exn_cons_args,
        ret: exn_type,
        vars: VarMap::default(),
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
            TypeAnnotation::CustomClass { id: PRIMITIVE_DEF_IDS.exception, params: Vec::default() },
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
    unifier: &mut Unifier,
    var_map: &VarMap,
    name: &'static str,
    ret_ty: Type,
    param_ty: &[(Type, &'static str)],
    codegen_callback: Box<GenCallCallback>,
) -> Arc<RwLock<TopLevelDef>> {
    Arc::new(RwLock::new(TopLevelDef::Function {
        name: name.into(),
        simple_name: name.into(),
        signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
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
    unifier: &mut Unifier,
    var_map: &VarMap,
    name: &'static str,
    ret_ty: Type,
    params: &[(Type, &'static str)],
    intrinsic_fn: &'static str,
) -> Arc<RwLock<TopLevelDef>> {
    let param_tys = params.iter()
        .map(|p| p.0)
        .collect_vec();

    create_fn_by_codegen(
        unifier,
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
    unifier: &mut Unifier,
    var_map: &VarMap,
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
        unifier,
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

pub fn get_builtins(unifier: &mut Unifier, primitives: &PrimitiveStore) -> BuiltinInfo {
    let PrimitiveStore {
        int32,
        int64,
        uint32,
        uint64,
        float,
        bool: boolean,
        range,
        str: string,
        ndarray,
        ..
    } = *primitives;

    let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), None);
    let ndarray_float_2d = {
        let value = match primitives.size_t {
            64 => SymbolValue::U64(2u64),
            32 => SymbolValue::U32(2u32),
            _ => unreachable!(),
        };
        let ndims = unifier.add_ty(TypeEnum::TLiteral {
            values: vec![value],
            loc: None,
        });

        make_ndarray_ty(unifier, primitives, Some(float), Some(ndims))
    };
    let list_int32 = unifier.add_ty(TypeEnum::TList { ty: int32 });
    let num_ty = unifier.get_fresh_var_with_range(
        &[int32, int64, float, boolean, uint32, uint64],
        Some("N".into()),
        None,
    );
    let num_var_map: VarMap = vec![
        (num_ty.1, num_ty.0),
    ].into_iter().collect();

    let new_type_or_ndarray_ty = |unifier: &mut Unifier, primitives: &PrimitiveStore, scalar_ty: Type| {
        let ndarray = make_ndarray_ty(unifier, primitives, Some(scalar_ty), None);

        unifier.get_fresh_var_with_range(
            &[scalar_ty, ndarray],
            Some("T".into()),
            None,
        )
    };

    let ndarray_num_ty = make_ndarray_ty(unifier, primitives, Some(num_ty.0), None);
    let float_or_ndarray_ty = unifier.get_fresh_var_with_range(
        &[float, ndarray_float],
        Some("T".into()),
        None,
    );
    let float_or_ndarray_var_map: VarMap = vec![
        (float_or_ndarray_ty.1, float_or_ndarray_ty.0),
    ].into_iter().collect();

    let num_or_ndarray_ty = unifier.get_fresh_var_with_range(
        &[num_ty.0, ndarray_num_ty],
        Some("T".into()),
        None,
    );
    let num_or_ndarray_var_map: VarMap = vec![
        (num_ty.1, num_ty.0),
        (num_or_ndarray_ty.1, num_or_ndarray_ty.0),
    ].into_iter().collect();

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
            unifier.get_ty(primitives.option).as_ref()
        {
            (
                *fields.get(&"is_some".into()).unwrap(),
                *fields.get(&"unwrap".into()).unwrap(),
                (*params.iter().next().unwrap().1, *params.iter().next().unwrap().0),
            )
        } else {
            unreachable!()
        };

    let TypeEnum::TObj {
        fields: ndarray_fields,
        params: ndarray_params,
        ..
    } = &*unifier.get_ty(primitives.ndarray) else {
        unreachable!()
    };

    let (ndarray_dtype_ty, ndarray_dtype_var_id) = ndarray_params
        .iter()
        .next()
        .map(|(var_id, ty)| (*ty, *var_id))
        .unwrap();
    let (ndarray_ndims_ty, ndarray_ndims_var_id) = ndarray_params
        .iter()
        .nth(1)
        .map(|(var_id, ty)| (*ty, *var_id))
        .unwrap();
    let ndarray_copy_ty = *ndarray_fields.get(&"copy".into()).unwrap();
    let ndarray_fill_ty = *ndarray_fields.get(&"fill".into()).unwrap();

    let top_level_def_list = vec![
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.int32,
            None,
            "int32".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.int64,
            None,
            "int64".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.float,
            None,
            "float".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.bool,
            None,
            "bool".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.none,
            None,
            "none".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.range,
            None,
            "range".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.str,
            None,
            "str".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelDef::Class {
            name: "Exception".into(),
            object_id: PRIMITIVE_DEF_IDS.exception,
            type_vars: Vec::default(),
            fields: exception_fields,
            methods: Vec::default(),
            ancestors: vec![],
            constructor: None,
            resolver: None,
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.uint32,
            None,
            "uint32".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new(TopLevelComposer::make_top_level_class_def(
            PRIMITIVE_DEF_IDS.uint64,
            None,
            "uint64".into(),
            None,
            None,
        ))),
        Arc::new(RwLock::new({
            TopLevelDef::Class {
                name: "Option".into(),
                object_id: PRIMITIVE_DEF_IDS.option,
                type_vars: vec![option_ty_var],
                fields: vec![],
                methods: vec![
                    ("is_some".into(), is_some_ty.0, DefinitionId(PRIMITIVE_DEF_IDS.option.0 + 1)),
                    ("is_none".into(), is_some_ty.0, DefinitionId(PRIMITIVE_DEF_IDS.option.0 + 2)),
                    ("unwrap".into(), unwrap_ty.0, DefinitionId(PRIMITIVE_DEF_IDS.option.0 + 3)),
                ],
                ancestors: vec![TypeAnnotation::CustomClass {
                    id: PRIMITIVE_DEF_IDS.option,
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
            codegen_callback: Some(Arc::new(GenCall::create_dummy(
                String::from("handled in gen_expr"),
            ))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Class {
            name: "ndarray".into(),
            object_id: PRIMITIVE_DEF_IDS.ndarray,
            type_vars: vec![ndarray_dtype_ty, ndarray_ndims_ty],
            fields: Vec::default(),
            methods: vec![
                ("copy".into(), ndarray_copy_ty.0, DefinitionId(PRIMITIVE_DEF_IDS.ndarray.0 + 1)),
                ("fill".into(), ndarray_fill_ty.0, DefinitionId(PRIMITIVE_DEF_IDS.ndarray.0 + 2)),
            ],
            ancestors: Vec::default(),
            constructor: None,
            resolver: None,
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "ndarray.copy".into(),
            simple_name: "copy".into(),
            signature: ndarray_copy_ty.0,
            var_id: vec![ndarray_dtype_var_id, ndarray_ndims_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, obj, fun, args, generator| {
                    gen_ndarray_copy(ctx, &obj, fun, &args, generator)
                        .map(|val| Some(val.as_basic_value_enum()))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "ndarray.fill".into(),
            simple_name: "fill".into(),
            signature: ndarray_fill_ty.0,
            var_id: vec![ndarray_dtype_var_id, ndarray_ndims_var_id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, obj, fun, args, generator| {
                    gen_ndarray_fill(ctx, &obj, fun, &args, generator)?;
                    Ok(None)
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "int32".into(),
            simple_name: "int32".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_int32(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "int64".into(),
            simple_name: "int64".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_int64(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "uint32".into(),
            simple_name: "uint32".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_uint32(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "uint64".into(),
            simple_name: "uint64".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_uint64(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "float".into(),
            simple_name: "float".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_float(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            unifier,
            &VarMap::new(),
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
            unifier,
            &VarMap::new(),
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
            unifier,
            &VarMap::new(),
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
            unifier,
            &VarMap::new(),
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
            let tv = unifier.get_fresh_var(Some("T".into()), None);

            create_fn_by_codegen(
                unifier,
                &[(tv.1, tv.0)].into_iter().collect(),
                "np_full",
                ndarray,
                // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
                // type variable
                &[(list_int32, "shape"), (tv.0, "fill_value")],
                Box::new(|ctx, obj, fun, args, generator| {
                    gen_ndarray_full(ctx, &obj, fun, &args, generator)
                        .map(|val| Some(val.as_basic_value_enum()))
                }),
            )
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "np_eye".into(),
            simple_name: "np_eye".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
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
                vars: VarMap::default(),
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
            unifier,
            &VarMap::new(),
            "np_identity",
            ndarray_float_2d,
            &[(int32, "n")],
            Box::new(|ctx, obj, fun, args, generator| {
                gen_ndarray_identity(ctx, &obj, fun, &args, generator)
                    .map(|val| Some(val.as_basic_value_enum()))
            }),
        ),
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int32 = make_ndarray_ty(unifier, primitives, Some(int32), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int32, ndarray_int32],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "round",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_round(generator, ctx, (arg_ty, arg), ctx.primitives.int32)?))
                }),
            )
        },
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int64 = make_ndarray_ty(unifier, primitives, Some(int64), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int64, ndarray_int64],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "round64",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_round(generator, ctx, (arg_ty, arg), ctx.primitives.int64)?))
                }),
            )
        },
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_round",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "n")],
            Box::new(|ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, arg_ty)?;

                Ok(Some(builtin_fns::call_numpy_round(generator, ctx, (arg_ty, arg))?))
            }),
        ),
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "range".into(),
            simple_name: "range".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
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
                vars: VarMap::default(),
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
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "s".into(), ty: string, default_value: None }],
                ret: string,
                vars: VarMap::default(),
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
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_bool(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        })),
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int32 = make_ndarray_ty(unifier, primitives, Some(int32), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int32, ndarray_int32],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "floor",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_floor(generator, ctx, (arg_ty, arg), ctx.primitives.int32)?))
                }),
            )
        },
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int64 = make_ndarray_ty(unifier, primitives, Some(int64), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int64, ndarray_int64],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "floor64",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_floor(generator, ctx, (arg_ty, arg), ctx.primitives.int64)?))
                }),
            )
        },
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_floor",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "n")],
            Box::new(|ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, arg_ty)?;

                Ok(Some(builtin_fns::call_floor(generator, ctx, (arg_ty, arg), ctx.primitives.float)?))
            }),
        ),
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int32 = make_ndarray_ty(unifier, primitives, Some(int32), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int32, ndarray_int32],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "ceil",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_ceil(generator, ctx, (arg_ty, arg), ctx.primitives.int32)?))
                }),
            )
        },
        {
            let common_ndim = unifier.get_fresh_const_generic_var(
                primitives.usize(),
                Some("N".into()),
                None,
            );
            let ndarray_int64 = make_ndarray_ty(unifier, primitives, Some(int64), Some(common_ndim.0));
            let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), Some(common_ndim.0));

            let p0_ty = unifier.get_fresh_var_with_range(
                &[float, ndarray_float],
                Some("T".into()),
                None,
            );
            let ret_ty = unifier.get_fresh_var_with_range(
                &[int64, ndarray_int64],
                Some("R".into()),
                None,
            );

            create_fn_by_codegen(
                unifier,
                &[
                    (common_ndim.1, common_ndim.0),
                    (p0_ty.1, p0_ty.0),
                    (ret_ty.1, ret_ty.0),
                ].into_iter().collect(),
                "ceil64",
                ret_ty.0,
                &[(p0_ty.0, "n")],
                Box::new(|ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, arg_ty)?;

                    Ok(Some(builtin_fns::call_ceil(generator, ctx, (arg_ty, arg), ctx.primitives.int64)?))
                }),
            )
        },
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_ceil",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "n")],
            Box::new(|ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, arg_ty)?;

                Ok(Some(builtin_fns::call_ceil(generator, ctx, (arg_ty, arg), ctx.primitives.float)?))
            }),
        ),
        Arc::new(RwLock::new({
            let tvar = unifier.get_fresh_var(Some("L".into()), None);
            let list = unifier.add_ty(TypeEnum::TList { ty: tvar.0 });
            let ndims = unifier.get_fresh_const_generic_var(primitives.uint64, Some("N".into()), None);
            let ndarray = make_ndarray_ty(
                unifier,
                primitives,
                Some(tvar.0),
                Some(ndims.0),
            );

            let arg_ty = unifier.get_fresh_var_with_range(
                &[list, ndarray, primitives.range],
                Some("I".into()),
                None,
            );
            TopLevelDef::Function {
                name: "len".into(),
                simple_name: "len".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: vec![FuncArg { name: "ls".into(), ty: arg_ty.0, default_value: None }],
                    ret: int32,
                    vars: vec![(tvar.1, tvar.0), (arg_ty.1, arg_ty.0)]
                        .into_iter()
                        .collect(),
                })),
                var_id: Vec::default(),
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
                                TypeEnum::TObj { obj_id, .. } if *obj_id == PRIMITIVE_DEF_IDS.ndarray => {
                                    let llvm_i32 = ctx.ctx.i32_type();
                                    let llvm_usize = generator.get_size_type(ctx.ctx);

                                    let arg = NDArrayValue::from_ptr_val(
                                        arg.into_pointer_value(),
                                        llvm_usize,
                                        None
                                    );

                                    let ndims = arg.dim_sizes().size(ctx, generator);
                                    ctx.make_assert(
                                        generator,
                                        ctx.builder.build_int_compare(
                                            IntPredicate::NE,
                                            ndims,
                                            llvm_usize.const_zero(),
                                            "",
                                        ).unwrap(),
                                        "0:TypeError",
                                        "len() of unsized object",
                                        [None, None, None],
                                        ctx.current_loc,
                                    );

                                    let len = unsafe {
                                        arg.dim_sizes().get_typed_unchecked(
                                            ctx,
                                            generator,
                                            &llvm_usize.const_zero(),
                                            None,
                                        )
                                    };

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
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "m".into(), ty: num_ty.0, default_value: None },
                    FuncArg { name: "n".into(), ty: num_ty.0, default_value: None },
                ],
                ret: num_ty.0,
                vars: num_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let m_ty = fun.0.args[0].ty;
                    let n_ty = fun.0.args[1].ty;
                    let m_val = args[0].1.clone().to_basic_value_enum(ctx, generator, m_ty)?;
                    let n_val = args[1].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;

                    Ok(Some(builtin_fns::call_min(ctx, (m_ty, m_val), (n_ty, n_val))))
                },
            )))),
            loc: None,
        })),
        {
            let ret_ty = unifier.get_fresh_var(Some("R".into()), None);
            let var_map = num_or_ndarray_var_map.clone()
                .into_iter()
                .chain(once((ret_ty.1, ret_ty.0)))
                .collect::<IndexMap<_, _>>();

            create_fn_by_codegen(
                unifier,
                &var_map,
                "np_min",
                ret_ty.0,
                &[(float_or_ndarray_ty.0, "a")],
                Box::new(|ctx, _, fun, args, generator| {
                    let a_ty = fun.0.args[0].ty;
                    let a = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, a_ty)?;

                    Ok(Some(builtin_fns::call_numpy_min(generator, ctx, (a_ty, a))?))
                }),
            )
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, num_ty.0);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, num_ty.0);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_minimum".into(),
                simple_name: "np_minimum".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x2_ty = fun.0.args[1].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, generator, x2_ty)?;
            
                    Ok(Some(builtin_fns::call_numpy_minimum(generator, ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
                })))),
                loc: None,
            }))
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "max".into(),
            simple_name: "max".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "m".into(), ty: num_ty.0, default_value: None },
                    FuncArg { name: "n".into(), ty: num_ty.0, default_value: None },
                ],
                ret: num_ty.0,
                vars: num_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let m_ty = fun.0.args[0].ty;
                    let n_ty = fun.0.args[1].ty;
                    let m_val = args[0].1.clone().to_basic_value_enum(ctx, generator, m_ty)?;
                    let n_val = args[1].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;

                    Ok(Some(builtin_fns::call_max(ctx, (m_ty, m_val), (n_ty, n_val))))
                },
            )))),
            loc: None,
        })),
        {
            let ret_ty = unifier.get_fresh_var(Some("R".into()), None);
            let var_map = num_or_ndarray_var_map.clone()
                .into_iter()
                .chain(once((ret_ty.1, ret_ty.0)))
                .collect::<IndexMap<_, _>>();

            create_fn_by_codegen(
                unifier,
                &var_map,
                "np_max",
                ret_ty.0,
                &[(float_or_ndarray_ty.0, "a")],
                Box::new(|ctx, _, fun, args, generator| {
                    let a_ty = fun.0.args[0].ty;
                    let a = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, a_ty)?;

                    Ok(Some(builtin_fns::call_numpy_max(generator, ctx, (a_ty, a))?))
                }),
            )
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, num_ty.0);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, num_ty.0);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_maximum".into(),
                simple_name: "np_maximum".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x2_ty = fun.0.args[1].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_maximum(generator, ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
                })))),
                loc: None,
            }))
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "abs".into(),
            simple_name: "abs".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: num_or_ndarray_ty.0, default_value: None }],
                ret: num_or_ndarray_ty.0,
                vars: num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let n_ty = fun.0.args[0].ty;
                    let n_val = args[0].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;

                    Ok(Some(builtin_fns::call_abs(generator, ctx, (n_ty, n_val))?))
                },
            )))),
            loc: None,
        })),
        create_fn_by_codegen(
            unifier,
            &VarMap::new(),
            "np_isnan",
            boolean,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_isnan(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &VarMap::new(),
            "np_isinf",
            boolean,
            &[(float, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_isinf(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_sin",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_sin(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_cos",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_cos(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_exp",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_exp(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_exp2",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_exp2(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_log",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_log(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_log10",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_log10(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_log2",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_log2(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_fabs",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_fabs(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_sqrt",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_sqrt(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_rint",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_rint(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_tan",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_tan(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arcsin",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arcsin(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arccos",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arccos(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arctan",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arctan(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_sinh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_sinh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_cosh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_cosh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_tanh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_tanh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arcsinh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arcsinh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arccosh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arccosh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_arctanh",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                   .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_arctanh(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_expm1",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_expm1(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "np_cbrt",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_numpy_cbrt(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_erf",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "z")],
            Box::new(|ctx, _, fun, args, generator| {
                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_erf(generator, ctx, (z_ty, z_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_erfc",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_erfc(generator, ctx, (z_ty, z_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_gamma",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "z")],
            Box::new(|ctx, _, fun, args, generator| {
                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_gamma(generator, ctx, (z_ty, z_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_gammaln",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_gammaln(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_j0",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let z_ty = fun.0.args[0].ty;
                let z_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, z_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_j0(generator, ctx, (z_ty, z_val))?))
            }),
        ),
        create_fn_by_codegen(
            unifier,
            &float_or_ndarray_var_map,
            "sp_spec_j1",
            float_or_ndarray_ty.0,
            &[(float_or_ndarray_ty.0, "x")],
            Box::new(|ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone()
                    .to_basic_value_enum(ctx, generator, x_ty)?;

                Ok(Some(builtin_fns::call_scipy_special_j1(generator, ctx, (x_ty, x_val))?))
            }),
        ),
        // Not mapped: jv/yv, libm only supports integer orders.
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_arctan2".into(),
                simple_name: "np_arctan2".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![ret_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;
            
                    Ok(Some(builtin_fns::call_numpy_arctan2(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_copysign".into(),
                simple_name: "np_copysign".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![ret_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_copysign(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_fmax".into(),
                simple_name: "np_fmax".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_fmax(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_fmin".into(),
                simple_name: "np_fmin".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_fmin(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, int32);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_ldexp".into(),
                simple_name: "np_ldexp".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_ldexp(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_hypot".into(),
                simple_name: "np_hypot".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_hypot(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        {
            let x1_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let x2_ty = new_type_or_ndarray_ty(unifier, primitives, float);
            let param_ty = &[(x1_ty.0, "x1"), (x2_ty.0, "x2")];
            let ret_ty = unifier.get_fresh_var(None, None);

            Arc::new(RwLock::new(TopLevelDef::Function {
                name: "np_nextafter".into(),
                simple_name: "np_nextafter".into(),
                signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: param_ty.iter().map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                    }).collect(),
                    ret: ret_ty.0,
                    vars: [
                        (x1_ty.1, x1_ty.0),
                        (x2_ty.1, x2_ty.0),
                        (ret_ty.1, ret_ty.0),
                    ].into_iter().collect(),
                })),
                var_id: vec![x1_ty.1, x2_ty.1],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone()
                        .to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone()
                        .to_basic_value_enum(ctx, generator, x2_ty)?;

                    Ok(Some(builtin_fns::call_numpy_nextafter(
                        generator,
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                })))),
                loc: None,
            }))
        },
        Arc::new(RwLock::new(TopLevelDef::Function {
            name: "Some".into(),
            simple_name: "Some".into(),
            signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "n".into(), ty: option_ty_var, default_value: None }],
                ret: primitives.option,
                vars: VarMap::from([(option_ty_var_id, option_ty_var)]),
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
