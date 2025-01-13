use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue},
    AddressSpace,
};

use crate::{
    codegen::{
        irrt,
        stmt::gen_if_else_expr_callback,
        types::{ndarray::NDArrayType, ListType, ProxyType},
        values::{
            ndarray::NDArrayValue, ArrayLikeValue, ArraySliceValue, ListValue, ProxyValue,
            TypedArrayLikeAdapter, TypedArrayLikeMutator,
        },
        CodeGenContext, CodeGenerator,
    },
    toplevel::helper::{arraylike_flatten_element_type, arraylike_get_ndims},
    typecheck::typedef::{Type, TypeEnum},
};

/// Get the expected `dtype` and `ndims` of the ndarray returned by `np_array(<list>)`.
fn get_list_object_dtype_and_ndims<'ctx, G: CodeGenerator + ?Sized>(
    generator: &G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    list_ty: Type,
) -> (BasicTypeEnum<'ctx>, u64) {
    let dtype = arraylike_flatten_element_type(&mut ctx.unifier, list_ty);
    let ndims = arraylike_get_ndims(&mut ctx.unifier, list_ty);

    (ctx.get_llvm_type(generator, dtype), ndims)
}

impl<'ctx> NDArrayType<'ctx> {
    /// Implementation of `np_array(<list>, copy=True)`
    fn construct_numpy_array_from_list_copy_true_impl<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let (dtype, ndims_int) = get_list_object_dtype_and_ndims(generator, ctx, list_ty);
        assert!(self.ndims >= ndims_int);
        assert_eq!(dtype, self.dtype);

        let list_value = list.as_i8_list(generator, ctx);

        // Validate `list` has a consistent shape.
        // Raise an exception if `list` is something abnormal like `[[1, 2], [3]]`.
        // If `list` has a consistent shape, deduce the shape and write it to `shape`.
        let ndims = self.llvm_usize.const_int(ndims_int, false);
        let shape = ctx.builder.build_array_alloca(self.llvm_usize, ndims, "").unwrap();
        let shape = ArraySliceValue::from_ptr_val(shape, ndims, None);
        let shape = TypedArrayLikeAdapter::from(
            shape,
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );
        irrt::ndarray::call_nac3_ndarray_array_set_and_validate_list_shape(
            generator, ctx, list_value, ndims, &shape,
        );

        let ndarray = Self::new(generator, ctx.ctx, dtype, ndims_int)
            .construct_uninitialized(generator, ctx, name);
        ndarray.copy_shape_from_array(generator, ctx, shape.base_ptr(ctx, generator));
        unsafe { ndarray.create_data(generator, ctx) };

        // Copy all contents from the list.
        irrt::ndarray::call_nac3_ndarray_array_write_list_to_array(ctx, list_value, ndarray);

        ndarray
    }

    /// Implementation of `np_array(<list>, copy=None)`
    fn construct_numpy_array_from_list_copy_none_impl<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        // np_array without copying is only possible `list` is not nested.
        //
        // If `list` is `list[T]`, we can create an ndarray with `data` set
        // to the array pointer of `list`.
        //
        // If `list` is `list[list[T]]` or worse, copy.

        let (dtype, ndims) = get_list_object_dtype_and_ndims(generator, ctx, list_ty);
        if ndims == 1 {
            // `list` is not nested
            assert_eq!(ndims, 1);
            assert!(self.ndims >= ndims);
            assert_eq!(dtype, self.dtype);

            let llvm_pi8 = ctx.ctx.i8_type().ptr_type(AddressSpace::default());

            let ndarray = Self::new(generator, ctx.ctx, dtype, 1)
                .construct_uninitialized(generator, ctx, name);

            // Set data
            let data = ctx
                .builder
                .build_pointer_cast(list.data().base_ptr(ctx, generator), llvm_pi8, "")
                .unwrap();
            ndarray.store_data(ctx, data);

            // ndarray->shape[0] = list->len;
            let shape = ndarray.shape();
            let list_len = list.load_size(ctx, None);
            unsafe {
                shape.set_typed_unchecked(ctx, generator, &self.llvm_usize.const_zero(), list_len);
            }

            // Set strides, the `data` is contiguous
            ndarray.set_strides_contiguous(ctx);

            ndarray
        } else {
            // `list` is nested, copy
            self.construct_numpy_array_from_list_copy_true_impl(
                generator,
                ctx,
                (list_ty, list),
                name,
            )
        }
    }

    /// Implementation of `np_array(<list>, copy=copy)`
    fn construct_numpy_array_list_impl<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        (list_ty, list): (Type, ListValue<'ctx>),
        copy: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(copy.get_type(), ctx.ctx.bool_type());

        let (dtype, ndims) = get_list_object_dtype_and_ndims(generator, ctx, list_ty);

        let ndarray = gen_if_else_expr_callback(
            generator,
            ctx,
            |_generator, _ctx| Ok(copy),
            |generator, ctx| {
                let ndarray = self.construct_numpy_array_from_list_copy_true_impl(
                    generator,
                    ctx,
                    (list_ty, list),
                    name,
                );
                Ok(Some(ndarray.as_base_value()))
            },
            |generator, ctx| {
                let ndarray = self.construct_numpy_array_from_list_copy_none_impl(
                    generator,
                    ctx,
                    (list_ty, list),
                    name,
                );
                Ok(Some(ndarray.as_base_value()))
            },
        )
        .unwrap()
        .map(BasicValueEnum::into_pointer_value)
        .unwrap();

        NDArrayType::new(generator, ctx.ctx, dtype, ndims).map_value(ndarray, None)
    }

    /// Implementation of `np_array(<ndarray>, copy=copy)`.
    pub fn construct_numpy_array_ndarray_impl<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayValue<'ctx>,
        copy: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(ndarray.get_type().dtype, self.dtype);
        assert!(self.ndims >= ndarray.get_type().ndims);
        assert_eq!(copy.get_type(), ctx.ctx.bool_type());

        let ndarray_val = gen_if_else_expr_callback(
            generator,
            ctx,
            |_generator, _ctx| Ok(copy),
            |generator, ctx| {
                let ndarray = ndarray.make_copy(generator, ctx); // Force copy
                Ok(Some(ndarray.as_base_value()))
            },
            |_generator, _ctx| {
                // No need to copy. Return `ndarray` itself.
                Ok(Some(ndarray.as_base_value()))
            },
        )
        .unwrap()
        .map(BasicValueEnum::into_pointer_value)
        .unwrap();

        ndarray.get_type().map_value(ndarray_val, name)
    }

    /// Create a new ndarray like
    /// [`np.array()`](https://numpy.org/doc/stable/reference/generated/numpy.array.html).
    ///
    /// Note that the returned [`NDArrayValue`] may have fewer dimensions than is specified by this
    /// instance. Use [`NDArrayValue::atleast_nd`] on the returned value if an `ndarray` instance
    /// with the exact number of dimensions is needed.
    pub fn construct_numpy_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        (object_ty, object): (Type, BasicValueEnum<'ctx>),
        copy: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        match &*ctx.unifier.get_ty_immutable(object_ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                let list = ListType::from_unifier_type(generator, ctx, object_ty)
                    .map_value(object.into_pointer_value(), None);
                self.construct_numpy_array_list_impl(generator, ctx, (object_ty, list), copy, name)
            }

            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayType::from_unifier_type(generator, ctx, object_ty)
                    .map_value(object.into_pointer_value(), None);
                self.construct_numpy_array_ndarray_impl(generator, ctx, ndarray, copy, name)
            }

            _ => panic!("Unrecognized object type: {}", ctx.unifier.stringify(object_ty)), // Typechecker ensures this
        }
    }
}
