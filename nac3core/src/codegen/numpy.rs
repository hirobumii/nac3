use inkwell::{
    values::{BasicValue, BasicValueEnum, PointerValue},
    IntPredicate,
};

use nac3parser::ast::StrRef;

use super::{
    macros::codegen_unreachable,
    stmt::gen_for_callback_incrementing,
    types::ndarray::NDArrayType,
    values::{ndarray::shape::parse_numpy_int_sequence, ProxyValue, UntypedArrayLikeAccessor},
    CodeGenContext, CodeGenerator,
};
use crate::{
    symbol_resolver::ValueEnum,
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys, DefinitionId},
    typecheck::typedef::{FunSignature, Type},
};

/// Generates LLVM IR for `ndarray.empty`.
pub fn gen_ndarray_empty<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_empty(generator, context, &shape, None);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.zeros`.
pub fn gen_ndarray_zeros<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_zeros(generator, context, dtype, &shape, None);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.ones`.
pub fn gen_ndarray_ones<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_ones(generator, context, dtype, &shape, None);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.full`.
pub fn gen_ndarray_full<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 2);

    let shape_ty = fun.0.args[0].ty;
    let shape_arg = args[0].1.clone().to_basic_value_enum(context, generator, shape_ty)?;
    let fill_value_ty = fun.0.args[1].ty;
    let fill_value_arg =
        args[1].1.clone().to_basic_value_enum(context, generator, fill_value_ty)?;

    let (dtype, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let llvm_dtype = context.get_llvm_type(generator, dtype);
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = parse_numpy_int_sequence(generator, context, (shape_ty, shape_arg));

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(ndims))
        .construct_numpy_full(generator, context, &shape, fill_value_arg, None);
    Ok(ndarray.as_base_value())
}

pub fn gen_ndarray_array<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert!(matches!(args.len(), 1..=3));

    let obj_ty = fun.0.args[0].ty;
    let obj_arg = args[0].1.clone().to_basic_value_enum(context, generator, obj_ty)?;

    let copy_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name))
    {
        let copy_ty = fun.0.args[1].ty;
        arg.1.clone().to_basic_value_enum(context, generator, copy_ty)?
    } else {
        context.gen_symbol_val(
            generator,
            fun.0.args[1].default_value.as_ref().unwrap(),
            fun.0.args[1].ty,
        )
    };

    // The ndmin argument is ignored. We can simply force the ndarray's number of dimensions to be
    // the `ndims` of the function return type.
    let (_, ndims) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);
    let ndims = extract_ndims(&context.unifier, ndims);

    let copy = generator.bool_to_i1(context, copy_arg.into_int_value());
    let ndarray = NDArrayType::from_unifier_type(generator, context, fun.0.ret)
        .construct_numpy_array(generator, context, (obj_ty, obj_arg), copy, None)
        .atleast_nd(generator, context, ndims);

    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.eye`.
pub fn gen_ndarray_eye<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert!(matches!(args.len(), 1..=3));

    let nrows_ty = fun.0.args[0].ty;
    let nrows_arg = args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)?;

    let ncols_ty = fun.0.args[1].ty;
    let ncols_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[1].name))
    {
        arg.1.clone().to_basic_value_enum(context, generator, ncols_ty)
    } else {
        args[0].1.clone().to_basic_value_enum(context, generator, nrows_ty)
    }?;

    let offset_ty = fun.0.args[2].ty;
    let offset_arg = if let Some(arg) =
        args.iter().find(|arg| arg.0.is_some_and(|name| name == fun.0.args[2].name))
    {
        arg.1.clone().to_basic_value_enum(context, generator, offset_ty)
    } else {
        Ok(context.gen_symbol_val(
            generator,
            fun.0.args[2].default_value.as_ref().unwrap(),
            offset_ty,
        ))
    }?;

    let (dtype, _) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);

    let llvm_usize = generator.get_size_type(context.ctx);
    let llvm_dtype = context.get_llvm_type(generator, dtype);

    let nrows = context
        .builder
        .build_int_s_extend_or_bit_cast(nrows_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let ncols = context
        .builder
        .build_int_s_extend_or_bit_cast(ncols_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let offset = context
        .builder
        .build_int_s_extend_or_bit_cast(offset_arg.into_int_value(), llvm_usize, "")
        .unwrap();

    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(2))
        .construct_numpy_eye(generator, context, dtype, nrows, ncols, offset, None);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.identity`.
pub fn gen_ndarray_identity<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let n_ty = fun.0.args[0].ty;
    let n_arg = args[0].1.clone().to_basic_value_enum(context, generator, n_ty)?;

    let (dtype, _) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);

    let llvm_usize = generator.get_size_type(context.ctx);
    let llvm_dtype = context.get_llvm_type(generator, dtype);

    let n = context
        .builder
        .build_int_s_extend_or_bit_cast(n_arg.into_int_value(), llvm_usize, "")
        .unwrap();
    let ndarray = NDArrayType::new(generator, context.ctx, llvm_dtype, Some(2))
        .construct_numpy_identity(generator, context, dtype, n, None);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.copy`.
pub fn gen_ndarray_copy<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    _fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_some());
    assert!(args.is_empty());

    let this_ty = obj.as_ref().unwrap().0;
    let this_arg =
        obj.as_ref().unwrap().1.clone().to_basic_value_enum(context, generator, this_ty)?;

    let this = NDArrayType::from_unifier_type(generator, context, this_ty)
        .map_value(this_arg.into_pointer_value(), None);
    let ndarray = this.make_copy(generator, context);
    Ok(ndarray.as_base_value())
}

/// Generates LLVM IR for `ndarray.fill`.
pub fn gen_ndarray_fill<'ctx>(
    context: &mut CodeGenContext<'ctx, '_>,
    obj: &Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<(), String> {
    assert!(obj.is_some());
    assert_eq!(args.len(), 1);

    let this_ty = obj.as_ref().unwrap().0;
    let this_arg =
        obj.as_ref().unwrap().1.clone().to_basic_value_enum(context, generator, this_ty)?;
    let value_ty = fun.0.args[0].ty;
    let value_arg = args[0].1.clone().to_basic_value_enum(context, generator, value_ty)?;

    let this = NDArrayType::from_unifier_type(generator, context, this_ty)
        .map_value(this_arg.into_pointer_value(), None);
    this.fill(generator, context, value_arg);
    Ok(())
}

/// Generates LLVM IR for `ndarray.dot`.
/// Calculate inner product of two vectors or literals
/// For matrix multiplication use `np_matmul`
///
/// The input `NDArray` are flattened and treated as 1D
/// The operation is equivalent to `np.dot(arr1.ravel(), arr2.ravel())`
pub fn ndarray_dot<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    x1: (Type, BasicValueEnum<'ctx>),
    x2: (Type, BasicValueEnum<'ctx>),
) -> Result<BasicValueEnum<'ctx>, String> {
    const FN_NAME: &str = "ndarray_dot";
    let (x1_ty, x1) = x1;
    let (x2_ty, x2) = x2;

    let llvm_usize = generator.get_size_type(ctx.ctx);

    match (x1, x2) {
        (BasicValueEnum::PointerValue(n1), BasicValueEnum::PointerValue(n2)) => {
            let n1 = NDArrayType::from_unifier_type(generator, ctx, x1_ty).map_value(n1, None);
            let n2 = NDArrayType::from_unifier_type(generator, ctx, x2_ty).map_value(n2, None);

            let n1_sz = n1.size(generator, ctx);
            let n2_sz = n2.size(generator, ctx);

            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(IntPredicate::EQ, n1_sz, n2_sz, "").unwrap(),
                "0:ValueError",
                "shapes ({0}), ({1}) not aligned",
                [Some(n1_sz), Some(n2_sz), None],
                ctx.current_loc,
            );

            let identity =
                unsafe { n1.data().get_unchecked(ctx, generator, &llvm_usize.const_zero(), None) };
            let acc = ctx.builder.build_alloca(identity.get_type(), "").unwrap();
            ctx.builder.build_store(acc, identity.get_type().const_zero()).unwrap();

            gen_for_callback_incrementing(
                generator,
                ctx,
                None,
                llvm_usize.const_zero(),
                (n1_sz, false),
                |generator, ctx, _, idx| {
                    let elem1 = unsafe { n1.data().get_unchecked(ctx, generator, &idx, None) };
                    let elem2 = unsafe { n2.data().get_unchecked(ctx, generator, &idx, None) };

                    let product = match elem1 {
                        BasicValueEnum::IntValue(e1) => ctx
                            .builder
                            .build_int_mul(e1, elem2.into_int_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        BasicValueEnum::FloatValue(e1) => ctx
                            .builder
                            .build_float_mul(e1, elem2.into_float_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        _ => codegen_unreachable!(ctx, "product: {}", elem1.get_type()),
                    };
                    let acc_val = ctx.builder.build_load(acc, "").unwrap();
                    let acc_val = match acc_val {
                        BasicValueEnum::IntValue(e1) => ctx
                            .builder
                            .build_int_add(e1, product.into_int_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        BasicValueEnum::FloatValue(e1) => ctx
                            .builder
                            .build_float_add(e1, product.into_float_value(), "")
                            .unwrap()
                            .as_basic_value_enum(),
                        _ => codegen_unreachable!(ctx, "acc_val: {}", acc_val.get_type()),
                    };
                    ctx.builder.build_store(acc, acc_val).unwrap();

                    Ok(())
                },
                llvm_usize.const_int(1, false),
            )?;
            let acc_val = ctx.builder.build_load(acc, "").unwrap();
            Ok(acc_val)
        }
        (BasicValueEnum::IntValue(e1), BasicValueEnum::IntValue(e2)) => {
            Ok(ctx.builder.build_int_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        (BasicValueEnum::FloatValue(e1), BasicValueEnum::FloatValue(e2)) => {
            Ok(ctx.builder.build_float_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        _ => codegen_unreachable!(
            ctx,
            "{FN_NAME}() not supported for '{}'",
            format!("'{}'", ctx.unifier.stringify(x1_ty))
        ),
    }
}
