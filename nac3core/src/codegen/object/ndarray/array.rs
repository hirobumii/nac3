use super::NDArrayObject;
use crate::{
    codegen::{
        irrt::{
            call_nac3_ndarray_array_set_and_validate_list_shape,
            call_nac3_ndarray_array_write_list_to_array,
        },
        model::*,
        object::{any::AnyObject, list::ListObject},
        stmt::gen_if_else_expr_callback,
        CodeGenContext, CodeGenerator,
    },
    toplevel::helper::{arraylike_flatten_element_type, arraylike_get_ndims},
    typecheck::typedef::{Type, TypeEnum},
};

/// Get the expected `dtype` and `ndims` of the ndarray returned by `np_array(list)`.
fn get_list_object_dtype_and_ndims<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    list: ListObject<'ctx>,
) -> (Type, u64) {
    let dtype = arraylike_flatten_element_type(&mut ctx.unifier, list.item_type);

    let ndims = arraylike_get_ndims(&mut ctx.unifier, list.item_type);
    let ndims = ndims + 1; // To count `list` itself.

    (dtype, ndims)
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Implementation of `np_array(<list>, copy=True)`
    fn make_np_array_list_copy_true_impl<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        list: ListObject<'ctx>,
    ) -> Self {
        let (dtype, ndims_int) = get_list_object_dtype_and_ndims(ctx, list);
        let list_value = list.instance.with_pi8_items(generator, ctx);

        // Validate `list` has a consistent shape.
        // Raise an exception if `list` is something abnormal like `[[1, 2], [3]]`.
        // If `list` has a consistent shape, deduce the shape and write it to `shape`.
        let ndims = Int(SizeT).const_int(generator, ctx.ctx, ndims_int);
        let shape = Int(SizeT).array_alloca(generator, ctx, ndims.value);
        call_nac3_ndarray_array_set_and_validate_list_shape(
            generator, ctx, list_value, ndims, shape,
        );

        let ndarray = NDArrayObject::alloca(generator, ctx, dtype, ndims_int);
        ndarray.copy_shape_from_array(generator, ctx, shape);
        ndarray.create_data(generator, ctx);

        // Copy all contents from the list.
        call_nac3_ndarray_array_write_list_to_array(generator, ctx, list_value, ndarray.instance);

        ndarray
    }

    /// Implementation of `np_array(<list>, copy=None)`
    fn make_np_array_list_copy_none_impl<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        list: ListObject<'ctx>,
    ) -> Self {
        // np_array without copying is only possible `list` is not nested.
        //
        // If `list` is `list[T]`, we can create an ndarray with `data` set
        // to the array pointer of `list`.
        //
        // If `list` is `list[list[T]]` or worse, copy.

        let (dtype, ndims) = get_list_object_dtype_and_ndims(ctx, list);
        if ndims == 1 {
            // `list` is not nested
            let ndarray = NDArrayObject::alloca(generator, ctx, dtype, 1);

            // Set data
            let data = list.instance.get(generator, ctx, |f| f.items).cast_to_pi8(generator, ctx);
            ndarray.instance.set(ctx, |f| f.data, data);

            // ndarray->shape[0] = list->len;
            let shape = ndarray.instance.get(generator, ctx, |f| f.shape);
            let list_len = list.instance.get(generator, ctx, |f| f.len);
            shape.set_index_const(ctx, 0, list_len);

            // Set strides, the `data` is contiguous
            ndarray.set_strides_contiguous(generator, ctx);

            ndarray
        } else {
            // `list` is nested, copy
            NDArrayObject::make_np_array_list_copy_true_impl(generator, ctx, list)
        }
    }

    /// Implementation of `np_array(<list>, copy=copy)`
    fn make_np_array_list_impl<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        list: ListObject<'ctx>,
        copy: Instance<'ctx, Int<Bool>>,
    ) -> Self {
        let (dtype, ndims) = get_list_object_dtype_and_ndims(ctx, list);

        let ndarray = gen_if_else_expr_callback(
            generator,
            ctx,
            |_generator, _ctx| Ok(copy.value),
            |generator, ctx| {
                let ndarray =
                    NDArrayObject::make_np_array_list_copy_true_impl(generator, ctx, list);
                Ok(Some(ndarray.instance.value))
            },
            |generator, ctx| {
                let ndarray =
                    NDArrayObject::make_np_array_list_copy_none_impl(generator, ctx, list);
                Ok(Some(ndarray.instance.value))
            },
        )
        .unwrap()
        .unwrap();

        NDArrayObject::from_value_and_unpacked_types(generator, ctx, ndarray, dtype, ndims)
    }

    /// Implementation of `np_array(<ndarray>, copy=copy)`.
    pub fn make_np_array_ndarray_impl<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayObject<'ctx>,
        copy: Instance<'ctx, Int<Bool>>,
    ) -> Self {
        let ndarray_val = gen_if_else_expr_callback(
            generator,
            ctx,
            |_generator, _ctx| Ok(copy.value),
            |generator, ctx| {
                let ndarray = ndarray.make_copy(generator, ctx); // Force copy
                Ok(Some(ndarray.instance.value))
            },
            |_generator, _ctx| {
                // No need to copy. Return `ndarray` itself.
                Ok(Some(ndarray.instance.value))
            },
        )
        .unwrap()
        .unwrap();

        NDArrayObject::from_value_and_unpacked_types(
            generator,
            ctx,
            ndarray_val,
            ndarray.dtype,
            ndarray.ndims,
        )
    }

    /// Create a new ndarray like `np.array()`.
    ///
    /// NOTE: The `ndmin` argument is not here. You may want to
    /// do [`NDArrayObject::atleast_nd`] to achieve that.
    pub fn make_np_array<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        object: AnyObject<'ctx>,
        copy: Instance<'ctx, Int<Bool>>,
    ) -> Self {
        match &*ctx.unifier.get_ty(object.ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                let list = ListObject::from_object(generator, ctx, object);
                NDArrayObject::make_np_array_list_impl(generator, ctx, list, copy)
            }
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayObject::from_object(generator, ctx, object);
                NDArrayObject::make_np_array_ndarray_impl(generator, ctx, ndarray, copy)
            }
            _ => panic!("Unrecognized object type: {}", ctx.unifier.stringify(object.ty)), // Typechecker ensures this
        }
    }
}
