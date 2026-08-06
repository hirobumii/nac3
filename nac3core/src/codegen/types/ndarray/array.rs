use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue},
};

use crate::{
    codegen::{
        CodeGenContext,
        expr::call_extern,
        types::{
            ListValue, NDArrayType, ProxyTypeBase, RefCountedArrayType, RefCountedValue,
            array::ArrayLikeIndexer,
            field,
            list::ListType,
            ndarray::{NDArrayValue, RawNDArrayType},
            reference::TypedRefCountedType,
        },
    },
    toplevel::helper::{arraylike_flatten_element_type, arraylike_get_ndims},
    typecheck::typedef::{Type, TypeEnum},
};

/// Get the expected `dtype` and `ndims` of the ndarray returned by `np_array(<list>)`.
fn get_list_object_dtype_and_ndims<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    list_ty: Type,
) -> (BasicTypeEnum<'ctx>, u64) {
    let dtype = arraylike_flatten_element_type(&mut ctx.unifier, list_ty);
    let ndims = arraylike_get_ndims(&mut ctx.unifier, list_ty);

    (ctx.get_llvm_type(dtype), ndims)
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Implementation of `np_array(<list>, copy=True)`
    fn from_list_must_copy(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        let (dtype, ndims_int) = get_list_object_dtype_and_ndims(ctx, list_ty);

        // Validate `list` has a consistent shape.
        // Raise an exception if `list` is something abnormal like `[[1, 2], [3]]`.
        // If `list` has a consistent shape, deduce the shape and write it to `shape`.
        let ndims = ctx.size_t.const_int(ndims_int, false);
        let shape = RefCountedArrayType::new(ctx, ctx.size_t, Some(ndims_int as u32))
            .allocate(ctx, ndims, None)?;
        call_extern!(ctx: void _ = "__nac3_ndarray_array_set_and_validate_list_shape"(list.value, ndims, shape.inner_value(ctx, None)?.value.0))?;

        let ndarray = NDArrayType::create(ctx, dtype, ndims_int).construct(ctx, name)?;
        ndarray
            .shape(ctx)?
            .inner_value(ctx, Some(ctx.size_t.const_int(ndims_int, false)))?
            .memcpy_from(ctx, shape.inner_value(ctx, None)?.value.0)?;
        ndarray.create_data(ctx)?;

        // Copy all contents from the list.
        call_extern!(ctx: void _ = "__nac3_ndarray_array_write_list_to_array"(list.value, ndarray.value))?;

        Ok(ndarray)
    }

    /// Implementation of `np_array(<list>, copy=None)`
    fn from_list_maybe_copy(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        // np_array without copying is only possible `list` is not nested.
        //
        // If `list` is `list[T]`, we can create an ndarray with `data` set
        // to the array pointer of `list`.
        //
        // If `list` is `list[list[T]]` or worse, copy.

        let (dtype, ndims) = get_list_object_dtype_and_ndims(ctx, list_ty);
        if ndims == 1 {
            // `list` is not nested
            assert_eq!(ndims, 1);

            let ndarray = NDArrayType::create(ctx, dtype, 1).construct(ctx, name)?;

            let list_len = list.inner_value(ctx)?.load(ctx, field!(len))?;
            let list_data = list.inner_value(ctx)?.data(ctx)?;
            let len = list_len;
            // ndarray->data->refcount += 1;
            list_data.header(ctx).safe_increment_refcount(ctx)?;
            // ndarray->data = list->data;
            ndarray.inner_value(ctx)?.store(ctx, field!(data), list_data.value)?;
            // ndarray->shape[0] = list->len;
            ndarray.shape(ctx)?.inner_value(ctx, None)?.set_unchecked(
                ctx,
                &ctx.size_t.const_zero(),
                len,
                None,
            )?;
            // Set strides, the `data` is contiguous
            ndarray.set_strides_contiguous(ctx)?;

            Ok(ndarray)
        } else {
            // `list` is nested, copy
            Self::from_list_must_copy(ctx, (list_ty, list), name)
        }
    }

    /// Implementation of `np_array(<list>, copy=copy)`
    fn from_list(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        copy: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        assert_eq!(copy.get_type(), ctx.i1);

        let (dtype, ndims) = get_list_object_dtype_and_ndims(ctx, list_ty);

        let ndarray = ctx.build_ternary(
            "np_array.list.copy",
            copy,
            |ctx| {
                let ndarray = Self::from_list_must_copy(ctx, (list_ty, list), name)?;
                Ok(ndarray.value)
            },
            |ctx| {
                let ndarray = Self::from_list_maybe_copy(ctx, (list_ty, list), name)?;
                Ok(ndarray.value)
            },
        )?;

        Ok(TypedRefCountedType::new(ctx, RawNDArrayType::new(ctx, dtype, ndims))
            .map_value(ndarray, None))
    }

    /// Implementation of `np_array(<ndarray>, copy=copy)`.
    fn from_ndarray(
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: Self,
        copy: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        assert_eq!(copy.get_type(), ctx.i1);

        let ndarray_val = ctx.build_ternary(
            "np_array.ndarray.copy",
            copy,
            |ctx| {
                let ndarray = ndarray.make_copy(ctx)?; // Force copy
                Ok(ndarray.value)
            },
            |_| {
                // No need to copy. Return `ndarray` itself.
                Ok(ndarray.value)
            },
        )?;

        Ok(ndarray.ty.map_value(ndarray_val, name))
    }

    /// Create a new ndarray like
    /// [`np.array()`](https://numpy.org/doc/stable/reference/generated/numpy.array.html).
    ///
    /// Note that the returned [`NDArrayValue`] may have fewer dimensions than is specified by this
    /// instance. Use [`NDArrayValue::atleast_nd`] on the returned value if an `ndarray` instance
    /// with the exact number of dimensions is needed.
    pub fn construct_from(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (object_ty, object): (Type, BasicValueEnum<'ctx>),
        copy: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<Self> {
        match &*ctx.unifier.get_ty_immutable(object_ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                let obj = object.into_pointer_value();
                let list = ListType::from_unifier_type(ctx, object_ty).map_value(obj, None);
                Self::from_list(ctx, (object_ty, list), copy, name)
            }

            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let obj = object.into_pointer_value();
                let ndarray = NDArrayType::from_unifier_type(ctx, object_ty).map_value(obj, None);
                Self::from_ndarray(ctx, ndarray, copy, name)
            }

            _ => panic!("Unrecognized object type: {}", ctx.unifier.stringify(object_ty)), // Typechecker ensures this
        }
    }
}
