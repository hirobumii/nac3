use crate::{
    codegen::{
        model::*,
        object::{
            any::AnyObject,
            ndarray::{nditer::NDIterHandle, shape_util::parse_numpy_int_sequence, NDArrayObject},
        },
        stmt::gen_for_callback,
        CodeGenContext, CodeGenerator,
    },
    symbol_resolver::ValueEnum,
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys, DefinitionId},
    typecheck::typedef::{FunSignature, Type},
};
use inkwell::{
    values::{BasicValue, BasicValueEnum, PointerValue},
    IntPredicate,
};
use nac3parser::ast::StrRef;

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
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = AnyObject { value: shape_arg, ty: shape_ty };
    let (_, shape) = parse_numpy_int_sequence(generator, context, shape);
    let ndarray = NDArrayObject::make_np_empty(generator, context, dtype, ndims, shape);
    Ok(ndarray.instance.value)
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
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = AnyObject { value: shape_arg, ty: shape_ty };
    let (_, shape) = parse_numpy_int_sequence(generator, context, shape);
    let ndarray = NDArrayObject::make_np_zeros(generator, context, dtype, ndims, shape);
    Ok(ndarray.instance.value)
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
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = AnyObject { value: shape_arg, ty: shape_ty };
    let (_, shape) = parse_numpy_int_sequence(generator, context, shape);
    let ndarray = NDArrayObject::make_np_ones(generator, context, dtype, ndims, shape);
    Ok(ndarray.instance.value)
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
    let ndims = extract_ndims(&context.unifier, ndims);

    let shape = AnyObject { value: shape_arg, ty: shape_ty };
    let (_, shape) = parse_numpy_int_sequence(generator, context, shape);
    let ndarray =
        NDArrayObject::make_np_full(generator, context, dtype, ndims, shape, fill_value_arg);
    Ok(ndarray.instance.value)
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

    let object = AnyObject { value: obj_arg, ty: obj_ty };
    // NAC3 booleans are i8.
    let copy = Int(Bool).truncate(generator, context, copy_arg.into_int_value());
    let ndarray = NDArrayObject::make_np_array(generator, context, object, copy)
        .atleast_nd(generator, context, ndims);

    Ok(ndarray.instance.value)
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

    let nrows = Int(Int32)
        .check_value(generator, context.ctx, nrows_arg)
        .unwrap()
        .s_extend_or_bit_cast(generator, context, SizeT);
    let ncols = Int(Int32)
        .check_value(generator, context.ctx, ncols_arg)
        .unwrap()
        .s_extend_or_bit_cast(generator, context, SizeT);
    let offset = Int(Int32)
        .check_value(generator, context.ctx, offset_arg)
        .unwrap()
        .s_extend_or_bit_cast(generator, context, SizeT);

    let ndarray = NDArrayObject::make_np_eye(generator, context, dtype, nrows, ncols, offset);
    Ok(ndarray.instance.value)
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

    let (dtype, _) = unpack_ndarray_var_tys(&mut context.unifier, fun.0.ret);

    let n_ty = fun.0.args[0].ty;
    let n_arg = args[0].1.clone().to_basic_value_enum(context, generator, n_ty)?;

    let n = Int(Int32).check_value(generator, context.ctx, n_arg).unwrap();
    let n = n.s_extend_or_bit_cast(generator, context, SizeT);
    let ndarray = NDArrayObject::make_np_identity(generator, context, dtype, n);
    Ok(ndarray.instance.value)
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

    let this = AnyObject { value: this_arg, ty: this_ty };
    let this = NDArrayObject::from_object(generator, context, this);
    let ndarray = this.make_copy(generator, context);
    Ok(ndarray.instance.value)
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

    let this = AnyObject { value: this_arg, ty: this_ty };
    let this = NDArrayObject::from_object(generator, context, this);
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

    match (x1, x2) {
        (BasicValueEnum::PointerValue(_), BasicValueEnum::PointerValue(_)) => {
            let a = AnyObject { ty: x1_ty, value: x1 };
            let b = AnyObject { ty: x2_ty, value: x2 };

            let a = NDArrayObject::from_object(generator, ctx, a);
            let b = NDArrayObject::from_object(generator, ctx, b);

            // TODO: General `np.dot()` https://numpy.org/doc/stable/reference/generated/numpy.dot.html.
            assert_eq!(a.ndims, 1);
            assert_eq!(b.ndims, 1);
            let common_dtype = a.dtype;

            // Check shapes.
            let a_size = a.size(generator, ctx);
            let b_size = b.size(generator, ctx);
            let same_shape = a_size.compare(ctx, IntPredicate::EQ, b_size);
            ctx.make_assert(
                generator,
                same_shape.value,
                "0:ValueError",
                "shapes ({0},) and ({1},) not aligned: {0} (dim 0) != {1} (dim 1)",
                [Some(a_size.value), Some(b_size.value), None],
                ctx.current_loc,
            );

            let dtype_llvm = ctx.get_llvm_type(generator, common_dtype);

            let result = ctx.builder.build_alloca(dtype_llvm, "np_dot_result").unwrap();
            ctx.builder.build_store(result, dtype_llvm.const_zero()).unwrap();

            // Do dot product.
            gen_for_callback(
                generator,
                ctx,
                Some("np_dot"),
                |generator, ctx| {
                    let a_iter = NDIterHandle::new(generator, ctx, a);
                    let b_iter = NDIterHandle::new(generator, ctx, b);
                    Ok((a_iter, b_iter))
                },
                |generator, ctx, (a_iter, _b_iter)| {
                    // Only a_iter drives the condition, b_iter should have the same status.
                    Ok(a_iter.has_next(generator, ctx).value)
                },
                |generator, ctx, _hooks, (a_iter, b_iter)| {
                    let a_scalar = a_iter.get_scalar(generator, ctx).value;
                    let b_scalar = b_iter.get_scalar(generator, ctx).value;

                    let old_result = ctx.builder.build_load(result, "").unwrap();
                    let new_result: BasicValueEnum<'ctx> = match old_result {
                        BasicValueEnum::IntValue(old_result) => {
                            let a_scalar = a_scalar.into_int_value();
                            let b_scalar = b_scalar.into_int_value();
                            let x = ctx.builder.build_int_mul(a_scalar, b_scalar, "").unwrap();
                            ctx.builder.build_int_add(old_result, x, "").unwrap().into()
                        }
                        BasicValueEnum::FloatValue(old_result) => {
                            let a_scalar = a_scalar.into_float_value();
                            let b_scalar = b_scalar.into_float_value();
                            let x = ctx.builder.build_float_mul(a_scalar, b_scalar, "").unwrap();
                            ctx.builder.build_float_add(old_result, x, "").unwrap().into()
                        }
                        _ => {
                            panic!("Unrecognized dtype: {}", ctx.unifier.stringify(common_dtype));
                        }
                    };

                    ctx.builder.build_store(result, new_result).unwrap();
                    Ok(())
                },
                |generator, ctx, (a_iter, b_iter)| {
                    a_iter.next(generator, ctx);
                    b_iter.next(generator, ctx);
                    Ok(())
                },
            )
            .unwrap();

            Ok(ctx.builder.build_load(result, "").unwrap())
        }
        (BasicValueEnum::IntValue(e1), BasicValueEnum::IntValue(e2)) => {
            Ok(ctx.builder.build_int_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        (BasicValueEnum::FloatValue(e1), BasicValueEnum::FloatValue(e2)) => {
            Ok(ctx.builder.build_float_mul(e1, e2, "").unwrap().as_basic_value_enum())
        }
        _ => unreachable!(
            "{FN_NAME}() not supported for '{}'",
            format!("'{}'", ctx.unifier.stringify(x1_ty))
        ),
    }
}
