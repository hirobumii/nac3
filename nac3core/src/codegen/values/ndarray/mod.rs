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
    irrt,
    llvm_intrinsics::{call_int_umin, call_memcpy_generic_array},
    stmt::gen_for_callback_incrementing,
    type_aligned_alloca,
    types::{ndarray::NDArrayType, structure::StructField},
    CodeGenContext, CodeGenerator,
};
pub use contiguous::*;
pub use nditer::*;

mod contiguous;
mod nditer;

/// Proxy type for accessing an `NDArray` value in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayValue<'ctx> {
    value: PointerValue<'ctx>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: Option<u64>,
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
        ndims: Option<u64>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr, llvm_usize).is_ok());

        NDArrayValue { value: ptr, dtype, ndims, llvm_usize, name }
    }

    fn ndims_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).ndims
    }

    /// Returns the pointer to the field storing the number of dimensions of this `NDArray`.
    fn ptr_to_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.ndims_field(ctx).ptr_by_gep(ctx, self.value, self.name)
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

    fn itemsize_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).itemsize
    }

    /// Stores the size of each element `itemsize` into this instance.
    pub fn store_itemsize<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        itemsize: IntValue<'ctx>,
    ) {
        debug_assert_eq!(itemsize.get_type(), generator.get_size_type(ctx.ctx));

        self.itemsize_field(ctx).set(ctx, self.value, itemsize, self.name);
    }

    /// Returns the size of each element of this `NDArray` as a value.
    pub fn load_itemsize(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.itemsize_field(ctx).get(ctx, self.value, self.name)
    }

    fn shape_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).shape
    }

    /// Returns the double-indirection pointer to the `shape` array, as if by calling
    /// `getelementptr` on the field.
    fn ptr_to_shape(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.shape_field(ctx).ptr_by_gep(ctx, self.value, self.name)
    }

    /// Stores the array of dimension sizes `dims` into this instance.
    fn store_shape(&self, ctx: &CodeGenContext<'ctx, '_>, dims: PointerValue<'ctx>) {
        self.shape_field(ctx).set(ctx, self.as_base_value(), dims, self.name);
    }

    /// Convenience method for creating a new array storing dimension sizes with the given `size`.
    pub fn create_shape(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        llvm_usize: IntType<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_shape(ctx, ctx.builder.build_array_alloca(llvm_usize, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the size of each dimension of this `NDArray`.
    #[must_use]
    pub fn shape(&self) -> NDArrayShapeProxy<'ctx, '_> {
        NDArrayShapeProxy(self)
    }

    fn strides_field(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).strides
    }

    /// Returns the double-indirection pointer to the `strides` array, as if by calling
    /// `getelementptr` on the field.
    fn ptr_to_strides(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.strides_field(ctx).ptr_by_gep(ctx, self.value, self.name)
    }

    /// Stores the array of stride sizes `strides` into this instance.
    fn store_strides(&self, ctx: &CodeGenContext<'ctx, '_>, strides: PointerValue<'ctx>) {
        self.strides_field(ctx).set(ctx, self.as_base_value(), strides, self.name);
    }

    /// Convenience method for creating a new array storing the stride with the given `size`.
    pub fn create_strides(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        llvm_usize: IntType<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_strides(ctx, ctx.builder.build_array_alloca(llvm_usize, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the stride of each dimension of this `NDArray`.
    #[must_use]
    pub fn strides(&self) -> NDArrayStridesProxy<'ctx, '_> {
        NDArrayStridesProxy(self)
    }

    fn data_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).data
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    pub fn ptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.data_field(ctx).ptr_by_gep(ctx, self.value, self.name)
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        let data = ctx
            .builder
            .build_bit_cast(data, ctx.ctx.i8_type().ptr_type(AddressSpace::default()), "")
            .unwrap();
        self.data_field(ctx).set(ctx, self.as_base_value(), data.into_pointer_value(), self.name);
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    ///
    /// The data buffer will be allocated on the stack, and is considered to be owned by this ndarray instance.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `shape` and `itemsize` of this ndarray instance is initialized.
    pub unsafe fn create_data<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) {
        let nbytes = self.nbytes(generator, ctx);

        let data = type_aligned_alloca(generator, ctx, self.dtype, nbytes, None);
        self.store_data(ctx, data);

        self.set_strides_contiguous(generator, ctx);
    }

    /// Returns a proxy object to the field storing the data of this `NDArray`.
    #[must_use]
    pub fn data(&self) -> NDArrayDataProxy<'ctx, '_> {
        NDArrayDataProxy(self)
    }

    /// Copy shape dimensions from an array.
    pub fn copy_shape_from_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
        shape: PointerValue<'ctx>,
    ) {
        let num_items = self.load_ndims(ctx);

        call_memcpy_generic_array(
            ctx,
            self.shape().base_ptr(ctx, generator),
            shape,
            num_items,
            ctx.ctx.bool_type().const_zero(),
        );
    }

    /// Copy shape dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_shape_from_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayValue<'ctx>,
    ) {
        if self.ndims.is_some() && src_ndarray.ndims.is_some() {
            assert_eq!(self.ndims, src_ndarray.ndims);
        } else {
            let self_ndims = self.load_ndims(ctx);
            let src_ndims = src_ndarray.load_ndims(ctx);

            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(
                    IntPredicate::EQ,
                    self_ndims,
                    src_ndims,
                    ""
                ).unwrap(),
                "0:AssertionError",
                "NDArrayValue::copy_shape_from_ndarray: Expected self.ndims ({0}) == src_ndarray.ndims ({1})",
                [Some(self_ndims), Some(src_ndims), None],
                ctx.current_loc
            );
        }

        let src_shape = src_ndarray.shape().base_ptr(ctx, generator);
        self.copy_shape_from_array(generator, ctx, src_shape);
    }

    /// Copy strides dimensions from an array.
    pub fn copy_strides_from_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
        strides: PointerValue<'ctx>,
    ) {
        let num_items = self.load_ndims(ctx);

        call_memcpy_generic_array(
            ctx,
            self.strides().base_ptr(ctx, generator),
            strides,
            num_items,
            ctx.ctx.bool_type().const_zero(),
        );
    }

    /// Copy strides dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_strides_from_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayValue<'ctx>,
    ) {
        if self.ndims.is_some() && src_ndarray.ndims.is_some() {
            assert_eq!(self.ndims, src_ndarray.ndims);
        } else {
            let self_ndims = self.load_ndims(ctx);
            let src_ndims = src_ndarray.load_ndims(ctx);

            ctx.make_assert(
                generator,
                ctx.builder.build_int_compare(
                    IntPredicate::EQ,
                    self_ndims,
                    src_ndims,
                    ""
                ).unwrap(),
                "0:AssertionError",
                "NDArrayValue::copy_shape_from_ndarray: Expected self.ndims ({0}) == src_ndarray.ndims ({1})",
                [Some(self_ndims), Some(src_ndims), None],
                ctx.current_loc
            );
        }

        let src_strides = src_ndarray.strides().base_ptr(ctx, generator);
        self.copy_strides_from_array(generator, ctx, src_strides);
    }

    /// Get the `np.size()` of this ndarray.
    pub fn size<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_size(generator, ctx, *self)
    }

    /// Get the `ndarray.nbytes` of this ndarray.
    pub fn nbytes<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_nbytes(generator, ctx, *self)
    }

    /// Get the `len()` of this ndarray.
    pub fn len<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_len(generator, ctx, *self)
    }

    /// Check if this ndarray is C-contiguous.
    ///
    /// See NumPy's `flags["C_CONTIGUOUS"]`: <https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html#numpy.ndarray.flags>
    pub fn is_c_contiguous<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_is_c_contiguous(generator, ctx, *self)
    }

    /// Call [`call_nac3_ndarray_set_strides_by_shape`] on this ndarray to update `strides`.
    ///
    /// Update the ndarray's strides to make the ndarray contiguous.
    pub fn set_strides_contiguous<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) {
        irrt::ndarray::call_nac3_ndarray_set_strides_by_shape(generator, ctx, *self);
    }

    #[must_use]
    pub fn make_copy<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Self {
        let clone = if self.ndims.is_some() {
            self.get_type().construct_uninitialized(generator, ctx, None)
        } else {
            self.get_type().construct_dyn_ndims(generator, ctx, self.load_ndims(ctx), None)
        };

        let shape = self.shape();
        clone.copy_shape_from_array(generator, ctx, shape.base_ptr(ctx, generator));
        unsafe { clone.create_data(generator, ctx) };
        clone.copy_data_from(generator, ctx, *self);
        clone
    }

    /// Copy data from another ndarray.
    ///
    /// This ndarray and `src` is that their `np.size()` should be the same. Their shapes
    /// do not matter. The copying order is determined by how their flattened views look.
    ///
    /// Panics if the `dtype`s of ndarrays are different.
    pub fn copy_data_from<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &CodeGenContext<'ctx, '_>,
        src: NDArrayValue<'ctx>,
    ) {
        assert_eq!(self.dtype, src.dtype, "self and src dtype should match");
        irrt::ndarray::call_nac3_ndarray_copy_data(generator, ctx, src, *self);
    }
}

impl<'ctx> ProxyValue<'ctx> for NDArrayValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = NDArrayType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDArrayType::from_type(
            self.as_base_value().get_type(),
            self.dtype,
            self.ndims,
            self.llvm_usize,
        )
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

/// Proxy type for accessing the `shape` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayShapeProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayShapeProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.shape().base_ptr(ctx, generator).get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        self.0.shape_field(ctx).get(ctx, self.0.as_base_value(), self.0.name)
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {
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

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {}

impl<'ctx> TypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {
    fn downcast_to_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx> TypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {
    fn upcast_from_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        value.into()
    }
}

/// Proxy type for accessing the `strides` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayStridesProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayStridesProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.strides().base_ptr(ctx, generator).get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        self.0.strides_field(ctx).get(ctx, self.0.as_base_value(), self.0.name)
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {
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

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {}

impl<'ctx> TypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {
    fn downcast_to_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx> TypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {
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
        self.0.data_field(ctx).get(ctx, self.0.as_base_value(), self.0.name)
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_ndarray_calc_size(
            generator,
            ctx,
            &self.as_slice_value(ctx, generator),
            (None, None),
        )
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

        let index = irrt::ndarray::call_ndarray_flatten_index(generator, ctx, *self.0, indices);
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
                        self.0.shape().get_typed_unchecked(ctx, generator, &i, None),
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

/// A version of [`call_nac3_ndarray_set_strides_by_shape`] in Rust.
///
/// This function is used generating strides for globally defined contiguous ndarrays.
#[must_use]
pub fn make_contiguous_strides(itemsize: u64, ndims: u64, shape: &[u64]) -> Vec<u64> {
    let mut strides = Vec::with_capacity(ndims as usize);
    let mut stride_product = 1u64;
    for i in 0..ndims {
        let axis = ndims - i - 1;
        strides[axis as usize] = stride_product * itemsize;
        stride_product *= shape[axis as usize];
    }
    strides
}
