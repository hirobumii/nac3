use inkwell::{
    types::{AnyType, AnyTypeEnum, BasicType, BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};

use super::{
    ArrayLikeIndexer, ArrayLikeValue, ProxyValue, TypedArrayLikeAccessor, TypedArrayLikeMutator,
    UntypedArrayLikeAccessor, UntypedArrayLikeMutator,
};
use crate::codegen::{
    irrt::{call_ndarray_calc_size, call_ndarray_flatten_index},
    llvm_intrinsics::call_int_umin,
    stmt::gen_for_callback_incrementing,
    types::NDArrayType,
    CodeGenContext, CodeGenerator,
};

/// Proxy type for accessing an `NDArray` value in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayValue<'ctx> {
    value: PointerValue<'ctx>,
    dtype: BasicTypeEnum<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Checks whether `value` is an instance of `NDArray`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_representable(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        NDArrayType::is_representable(value.get_type(), llvm_usize)
    }

    /// Creates an [`NDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr, llvm_usize).is_ok());

        NDArrayValue { value: ptr, dtype, llvm_usize, name }
    }

    /// Returns the pointer to the field storing the number of dimensions of this `NDArray`.
    fn ptr_to_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.ndims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the number of dimensions `ndims` into this instance.
    pub fn store_ndims<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        ndims: IntValue<'ctx>,
    ) {
        debug_assert_eq!(ndims.get_type(), generator.get_size_type(ctx.ctx));

        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_store(pndims, ndims).unwrap();
    }

    /// Returns the number of dimensions of this `NDArray` as a value.
    pub fn load_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_load(pndims, "").map(BasicValueEnum::into_int_value).unwrap()
    }

    /// Returns the double-indirection pointer to the `dims` array, as if by calling `getelementptr`
    /// on the field.
    fn ptr_to_dims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.dims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the array of dimension sizes `dims` into this instance.
    fn store_dim_sizes(&self, ctx: &CodeGenContext<'ctx, '_>, dims: PointerValue<'ctx>) {
        ctx.builder.build_store(self.ptr_to_dims(ctx), dims).unwrap();
    }

    /// Convenience method for creating a new array storing dimension sizes with the given `size`.
    pub fn create_dim_sizes(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        llvm_usize: IntType<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_dim_sizes(ctx, ctx.builder.build_array_alloca(llvm_usize, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the size of each dimension of this `NDArray`.
    #[must_use]
    pub fn dim_sizes(&self) -> NDArrayDimsProxy<'ctx, '_> {
        NDArrayDimsProxy(self)
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    pub fn ptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(2, true)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        let data = ctx
            .builder
            .build_bit_cast(data, ctx.ctx.i8_type().ptr_type(AddressSpace::default()), "")
            .unwrap();
        ctx.builder.build_store(self.ptr_to_data(ctx), data).unwrap();
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    pub fn create_data(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        elem_ty: BasicTypeEnum<'ctx>,
        size: IntValue<'ctx>,
    ) {
        let itemsize =
            ctx.builder.build_int_cast(elem_ty.size_of().unwrap(), size.get_type(), "").unwrap();
        let nbytes = ctx.builder.build_int_mul(size, itemsize, "").unwrap();

        // TODO: What about alignment?
        self.store_data(
            ctx,
            ctx.builder.build_array_alloca(ctx.ctx.i8_type(), nbytes, "").unwrap(),
        );
    }

    /// Returns a proxy object to the field storing the data of this `NDArray`.
    #[must_use]
    pub fn data(&self) -> NDArrayDataProxy<'ctx, '_> {
        NDArrayDataProxy(self)
    }
}

impl<'ctx> ProxyValue<'ctx> for NDArrayValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = NDArrayType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDArrayType::from_type(self.as_base_value().get_type(), self.dtype, self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<NDArrayValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDArrayValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for accessing the `dims` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDimsProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayDimsProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.dim_sizes().base_ptr(ctx, generator).get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        let var_name = self.0.name.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder
            .build_load(self.0.ptr_to_dims(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name.map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(self.base_ptr(ctx, generator), &[*idx], var_name.as_str())
                .unwrap()
        }
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let size = self.size(ctx, generator);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for axis 0 with size {1}",
            [Some(*idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) }
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {}

impl<'ctx> TypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    fn downcast_to_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx> TypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    fn upcast_from_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        value.into()
    }
}

/// Proxy type for accessing the `data` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDataProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayDataProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.dtype.as_any_type_enum()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        let var_name = self.0.name.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder
            .build_load(self.0.ptr_to_data(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> IntValue<'ctx> {
        call_ndarray_calc_size(generator, ctx, &self.as_slice_value(ctx, generator), (None, None))
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for NDArrayDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let sizeof_elem = ctx
            .builder
            .build_int_truncate_or_bit_cast(
                self.element_type(ctx, generator).size_of().unwrap(),
                idx.get_type(),
                "",
            )
            .unwrap();
        let idx = ctx.builder.build_int_mul(*idx, sizeof_elem, "").unwrap();
        let ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.base_ptr(ctx, generator),
                    &[idx],
                    name.unwrap_or_default(),
                )
                .unwrap()
        };

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                "",
            )
            .unwrap()
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let data_sz = self.size(ctx, generator);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, data_sz, "").unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds with size {1}",
            [Some(*idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        let ptr = unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) };

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                "",
            )
            .unwrap()
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDataProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDataProxy<'ctx, '_> {}

impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> ArrayLikeIndexer<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let indices_elem_ty = indices
            .ptr_offset(ctx, generator, &llvm_usize.const_zero(), None)
            .get_type()
            .get_element_type();
        let Ok(indices_elem_ty) = IntType::try_from(indices_elem_ty) else {
            panic!("Expected list[int32] but got {indices_elem_ty}")
        };
        assert_eq!(
            indices_elem_ty.get_bit_width(),
            32,
            "Expected list[int32] but got list[int{}]",
            indices_elem_ty.get_bit_width()
        );

        let index = call_ndarray_flatten_index(generator, ctx, *self.0, indices);
        let sizeof_elem = ctx
            .builder
            .build_int_truncate_or_bit_cast(
                self.element_type(ctx, generator).size_of().unwrap(),
                index.get_type(),
                "",
            )
            .unwrap();
        let index = ctx.builder.build_int_mul(index, sizeof_elem, "").unwrap();

        let ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.base_ptr(ctx, generator),
                    &[index],
                    name.unwrap_or_default(),
                )
                .unwrap()
        };
        // TODO: Current implementation is transparent
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                "",
            )
            .unwrap()
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let indices_size = indices.size(ctx, generator);
        let nidx_leq_ndims = ctx
            .builder
            .build_int_compare(IntPredicate::SLE, indices_size, self.0.load_ndims(ctx), "")
            .unwrap();
        ctx.make_assert(
            generator,
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        let indices_len = indices.size(ctx, generator);
        let ndarray_len = self.0.load_ndims(ctx);
        let len = call_int_umin(ctx, indices_len, ndarray_len, None);
        gen_for_callback_incrementing(
            generator,
            ctx,
            None,
            llvm_usize.const_zero(),
            (len, false),
            |generator, ctx, _, i| {
                let (dim_idx, dim_sz) = unsafe {
                    (
                        indices.get_unchecked(ctx, generator, &i, None).into_int_value(),
                        self.0.dim_sizes().get_typed_unchecked(ctx, generator, &i, None),
                    )
                };
                let dim_idx = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(dim_idx, dim_sz.get_type(), "")
                    .unwrap();

                let dim_lt =
                    ctx.builder.build_int_compare(IntPredicate::SLT, dim_idx, dim_sz, "").unwrap();

                ctx.make_assert(
                    generator,
                    dim_lt,
                    "0:IndexError",
                    "index {0} is out of bounds for axis 0 with size {1}",
                    [Some(dim_idx), Some(dim_sz), None],
                    ctx.current_loc,
                );

                Ok(())
            },
            llvm_usize.const_int(1, false),
        )
        .unwrap();

        let ptr = unsafe { self.ptr_offset_unchecked(ctx, generator, indices, name) };
        // TODO: Current implementation is transparent
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                "",
            )
            .unwrap()
    }
}

impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> UntypedArrayLikeAccessor<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
}
impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> UntypedArrayLikeMutator<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
}
