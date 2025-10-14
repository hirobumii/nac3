use std::iter::repeat_n;

use inkwell::{
    AddressSpace, IntPredicate,
    types::{AnyType, AnyTypeEnum, BasicType, BasicTypeEnum, IntType},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use super::{
    ArrayLikeIndexer, ArrayLikeValue, ProxyValue, TupleValue, TypedArrayLikeAccessor,
    TypedArrayLikeAdapter, TypedArrayLikeMutator, UntypedArrayLikeAccessor,
    UntypedArrayLikeMutator, structure::StructProxyValue,
};
use crate::{
    codegen::{
        CodeGenContext, irrt,
        llvm_intrinsics::{call_int_umin, call_memcpy_generic_array},
        stmt::{gen_for_callback_incrementing, gen_var},
        type_aligned_alloca,
        types::{
            TupleType,
            ndarray::NDArrayType,
            structure::{StructField, StructProxyType},
        },
    },
    typecheck::typedef::{Type, TypeEnum},
};
pub use broadcast::*;
pub use contiguous::*;
pub use indexing::*;
pub use nditer::*;

mod broadcast;
mod contiguous;
mod fold;
mod indexing;
mod map;
mod matmul;
mod nditer;
pub mod shape;
mod view;

/// Proxy type for accessing an `NDArray` value in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayValue<'ctx> {
    value: PointerValue<'ctx>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: u64,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Creates an [`NDArrayValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: u64,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval =
            gen_var(ctx, val.get_type().into(), name.map(|name| format!("{name}.addr")).as_deref())
                .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, dtype, ndims, llvm_usize, name)
    }

    /// Creates an [`NDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: u64,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        NDArrayValue { value: ptr, dtype, ndims, llvm_usize, name }
    }

    fn ndims_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().ndims
    }

    /// Stores the number of dimensions `ndims` into this instance.
    pub fn store_ndims(&self, ctx: &mut CodeGenContext<'ctx, '_>, ndims: IntValue<'ctx>) {
        debug_assert_eq!(ndims.get_type(), ctx.size_t);

        self.ndims_field().store(ctx, self.value, ndims, self.name);
    }

    /// Returns the number of dimensions of this `NDArray` as a value.
    pub fn load_ndims(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.ndims_field().load(ctx, self.value, self.name)
    }

    fn itemsize_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().itemsize
    }

    /// Stores the size of each element `itemsize` into this instance.
    pub fn store_itemsize(&self, ctx: &mut CodeGenContext<'ctx, '_>, itemsize: IntValue<'ctx>) {
        debug_assert_eq!(itemsize.get_type(), ctx.size_t);

        self.itemsize_field().store(ctx, self.value, itemsize, self.name);
    }

    /// Returns the size of each element of this `NDArray` as a value.
    pub fn load_itemsize(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.itemsize_field().load(ctx, self.value, self.name)
    }

    fn shape_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().shape
    }

    /// Stores the array of dimension sizes `dims` into this instance.
    fn store_shape(&self, ctx: &mut CodeGenContext<'ctx, '_>, dims: PointerValue<'ctx>) {
        self.shape_field().store(ctx, self.value, dims, self.name);
    }

    /// Convenience method for creating a new array storing dimension sizes with the given `size`.
    pub fn create_shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
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

    fn strides_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().strides
    }

    /// Stores the array of stride sizes `strides` into this instance.
    fn store_strides(&self, ctx: &mut CodeGenContext<'ctx, '_>, strides: PointerValue<'ctx>) {
        self.strides_field().store(ctx, self.value, strides, self.name);
    }

    /// Convenience method for creating a new array storing the stride with the given `size`.
    pub fn create_strides(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
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

    fn data_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().data
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    pub fn ptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.data_field().ptr_by_gep(ctx, self.value, self.name)
    }

    /// Stores the array of data elements `data` into this instance.
    pub fn store_data(&self, ctx: &mut CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        let data = ctx.builder.build_bit_cast(data, ctx.ptr, "").unwrap();
        self.data_field().store(ctx, self.value, data.into_pointer_value(), self.name);
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    ///
    /// The data buffer will be allocated on the stack, and is considered to be owned by this ndarray instance.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `shape` and `itemsize` of this ndarray instance is initialized.
    pub unsafe fn create_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) {
        let nbytes = self.nbytes(ctx);

        let data = type_aligned_alloca(ctx, self.dtype, nbytes, None);
        self.store_data(ctx, data);

        self.set_strides_contiguous(ctx);
    }

    /// Returns a proxy object to the field storing the data of this `NDArray`.
    #[must_use]
    pub fn data(&self) -> NDArrayDataProxy<'ctx, '_> {
        NDArrayDataProxy(self)
    }

    /// Copy shape dimensions from an array.
    pub fn copy_shape_from_array(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: PointerValue<'ctx>,
    ) {
        let num_items = self.load_ndims(ctx);

        call_memcpy_generic_array(ctx, self.shape().base_ptr(ctx), shape, num_items);
    }

    /// Copy shape dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_shape_from_ndarray(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayValue<'ctx>,
    ) {
        assert_eq!(self.ndims, src_ndarray.ndims);

        let src_shape = src_ndarray.shape().base_ptr(ctx);
        self.copy_shape_from_array(ctx, src_shape);
    }

    /// Copy strides dimensions from an array.
    pub fn copy_strides_from_array(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        strides: PointerValue<'ctx>,
    ) {
        let num_items = self.load_ndims(ctx);

        call_memcpy_generic_array(ctx, self.strides().base_ptr(ctx), strides, num_items);
    }

    /// Copy strides dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_strides_from_ndarray(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayValue<'ctx>,
    ) {
        assert_eq!(self.ndims, src_ndarray.ndims);

        let src_strides = src_ndarray.strides().base_ptr(ctx);
        self.copy_strides_from_array(ctx, src_strides);
    }

    /// Get the `np.size()` of this ndarray.
    pub fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_size(ctx, *self)
    }

    /// Get the `ndarray.nbytes` of this ndarray.
    pub fn nbytes(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_nbytes(ctx, *self)
    }

    /// Get the `len()` of this ndarray.
    pub fn len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_len(ctx, *self)
    }

    /// Check if this ndarray is C-contiguous.
    ///
    /// See NumPy's `flags["C_CONTIGUOUS"]`: <https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html#numpy.ndarray.flags>
    pub fn is_c_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_is_c_contiguous(ctx, *self)
    }

    /// Call [`call_nac3_ndarray_set_strides_by_shape`] on this ndarray to update `strides`.
    ///
    /// Update the ndarray's strides to make the ndarray contiguous.
    pub fn set_strides_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) {
        irrt::ndarray::call_nac3_ndarray_set_strides_by_shape(ctx, *self);
    }

    /// Clone/Copy this ndarray - Allocate a new ndarray with the same shape as this ndarray and
    /// copy the contents over.
    ///
    /// The new ndarray will own its data and will be C-contiguous.
    #[must_use]
    pub fn make_copy(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Self {
        let clone = self.get_type().construct_uninitialized(ctx, None);

        let shape = self.shape();
        clone.copy_shape_from_array(ctx, shape.base_ptr(ctx));
        unsafe { clone.create_data(ctx) };
        clone.copy_data_from(ctx, *self);
        clone
    }

    /// Copy data from another ndarray.
    ///
    /// This ndarray and `src` is that their `np.size()` should be the same. Their shapes
    /// do not matter. The copying order is determined by how their flattened views look.
    ///
    /// Panics if the `dtype`s of ndarrays are different.
    pub fn copy_data_from(&self, ctx: &mut CodeGenContext<'ctx, '_>, src: NDArrayValue<'ctx>) {
        assert_eq!(self.dtype, src.dtype, "self and src dtype should match");
        irrt::ndarray::call_nac3_ndarray_copy_data(ctx, src, *self);
    }

    /// Fill the ndarray with a scalar.
    ///
    /// `fill_value` must have the same LLVM type as the `dtype` of this ndarray.
    pub fn fill(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: BasicValueEnum<'ctx>) {
        // TODO: It is possible to optimize this by exploiting contiguous strides with memset.
        //       Probably best to implement in IRRT.
        self.foreach(ctx, |ctx, _, nditer| {
            let p = nditer.get_pointer(ctx);
            ctx.builder.build_store(p, value).unwrap();
            Ok(())
        })
        .unwrap();
    }

    /// Create the shape tuple of this ndarray like
    /// [`np.shape(<ndarray>)`](https://numpy.org/doc/stable/reference/generated/numpy.shape.html).
    ///
    /// All elements in the tuple are `i32`.
    pub fn make_shape_tuple(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> TupleValue<'ctx> {
        let llvm_i32 = ctx.i32;

        let objects = (0..self.ndims)
            .map(|i| {
                let dim = unsafe {
                    self.shape().get_typed_unchecked(
                        ctx,
                        &self.llvm_usize.const_int(i, false),
                        None,
                    )
                };
                ctx.builder.build_int_truncate_or_bit_cast(dim, llvm_i32, "").unwrap()
            })
            .map(|obj| obj.as_basic_value_enum())
            .collect_vec();

        TupleType::new(ctx, &repeat_n(llvm_i32, self.ndims as usize).collect_vec())
            .construct_from_objects(ctx, objects, None)
    }

    /// Create the strides tuple of this ndarray like
    /// [`<ndarray>.strides`](https://numpy.org/doc/stable/reference/generated/numpy.ndarray.strides.html).
    ///
    /// All elements in the tuple are `i32`.
    pub fn make_strides_tuple(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> TupleValue<'ctx> {
        let llvm_i32 = ctx.i32;

        let objects = (0..self.ndims)
            .map(|i| {
                let dim = unsafe {
                    self.strides().get_typed_unchecked(
                        ctx,
                        &self.llvm_usize.const_int(i, false),
                        None,
                    )
                };
                ctx.builder.build_int_truncate_or_bit_cast(dim, llvm_i32, "").unwrap()
            })
            .map(|obj| obj.as_basic_value_enum())
            .collect_vec();

        TupleType::new(ctx, &repeat_n(llvm_i32, self.ndims as usize).collect_vec())
            .construct_from_objects(ctx, objects, None)
    }

    /// Returns true if this ndarray is unsized - `ndims == 0` and only contains a scalar.
    #[must_use]
    pub fn is_unsized(&self) -> bool {
        self.ndims == 0
    }

    /// Returns the element present in this `ndarray` if this is unsized.
    pub fn get_unsized_element(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Option<BasicValueEnum<'ctx>> {
        if self.is_unsized() {
            // NOTE: `np.size(self) == 0` here is never possible.
            let zero = ctx.size_t.const_zero();
            let value = unsafe { self.data().get_unchecked(ctx, &zero, None) };

            Some(value)
        } else {
            None
        }
    }

    /// If this ndarray is unsized, return its sole value as an [`BasicValueEnum`].
    /// Otherwise, do nothing and return the ndarray itself.
    pub fn split_unsized(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ScalarOrNDArray<'ctx> {
        if let Some(unsized_elem) = self.get_unsized_element(ctx) {
            ScalarOrNDArray::Scalar(unsized_elem)
        } else {
            ScalarOrNDArray::NDArray(*self)
        }
    }

    /// Check if this `NDArray` can be used as an `out` ndarray for an operation.
    ///
    /// Raise an exception if the shapes do not match.
    pub fn assert_can_be_written_by_out(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        out_shape: impl TypedArrayLikeAccessor<'ctx, IntValue<'ctx>>,
    ) {
        let ndarray_shape = self.shape();
        let output_shape = out_shape;

        irrt::ndarray::call_nac3_ndarray_util_assert_output_shape_same(
            ctx,
            &ndarray_shape,
            &output_shape,
        );
    }
}

impl<'ctx> ProxyValue<'ctx> for NDArrayValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = NDArrayType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDArrayType::from_pointer_type(
            self.as_base_value().get_type(),
            self.dtype,
            self.ndims,
            self.llvm_usize,
        )
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for NDArrayValue<'ctx> {}

impl<'ctx> From<NDArrayValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDArrayValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for accessing the `shape` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayShapeProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayShapeProxy<'ctx, '_> {
    fn element_type(&self, ctx: &CodeGenContext<'ctx, '_>) -> AnyTypeEnum<'ctx> {
        self.0.shape().base_ptr(ctx).get_type().get_element_type()
    }

    fn base_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.0.shape_field().load(ctx, self.0.value, self.0.name)
    }

    fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayShapeProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name.map(|v| format!("{v}.addr")).unwrap_or_default();
        let base_ptr = self.base_ptr(ctx);
        unsafe { ctx.builder.build_in_bounds_gep(base_ptr, &[*idx], var_name.as_str()).unwrap() }
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let size = self.size(ctx);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        let ndims = self.0.load_ndims(ctx);
        ctx.make_assert(
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for axis 0 with size {1}",
            [Some(*idx), Some(ndims), None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, idx, name) }
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
    fn element_type(&self, ctx: &CodeGenContext<'ctx, '_>) -> AnyTypeEnum<'ctx> {
        self.0.strides().base_ptr(ctx).get_type().get_element_type()
    }

    fn base_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.0.strides_field().load(ctx, self.0.value, self.0.name)
    }

    fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayStridesProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name.map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(self.base_ptr(ctx), &[*idx], var_name.as_str()).unwrap()
        }
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let size = self.size(ctx);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        let ndims = self.0.load_ndims(ctx);
        ctx.make_assert(
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for axis 0 with size {1}",
            [Some(*idx), Some(ndims), None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, idx, name) }
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
    fn element_type(&self, _: &CodeGenContext<'ctx, '_>) -> AnyTypeEnum<'ctx> {
        self.0.dtype.as_any_type_enum()
    }

    fn base_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.0.data_field().load(ctx, self.0.value, self.0.name)
    }

    fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_len(ctx, *self.0)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for NDArrayDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let ptr = irrt::ndarray::call_nac3_ndarray_get_nth_pelement(ctx, *self.0, *idx);

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                name.unwrap_or_default(),
            )
            .unwrap()
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let data_sz = self.size(ctx);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, data_sz, "").unwrap();
        let ndims = self.0.load_ndims(ctx);
        ctx.make_assert(
            in_range,
            "0:IndexError",
            "index {0} is out of bounds with size {1}",
            [Some(*idx), Some(ndims), None],
            ctx.current_loc,
        );

        let ptr = unsafe { self.ptr_offset_unchecked(ctx, idx, name) };

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx))
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
    unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        assert_eq!(indices.element_type(ctx), ctx.size_t.into());

        let indices = TypedArrayLikeAdapter::from(
            indices.as_slice_value(ctx),
            |_, v| v.into_int_value(),
            |_, v| v.into(),
        );

        let ptr = irrt::ndarray::call_nac3_ndarray_get_pelement_by_indices(ctx, *self.0, &indices);

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                name.unwrap_or_default(),
            )
            .unwrap()
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,

        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = ctx.size_t;

        let indices_size = indices.size(ctx);
        let ndims = self.0.load_ndims(ctx);
        let nidx_leq_ndims =
            ctx.builder.build_int_compare(IntPredicate::SLE, indices_size, ndims, "").unwrap();
        ctx.make_assert(
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        let indices_len = indices.size(ctx);
        let ndarray_len = self.0.load_ndims(ctx);
        let len = call_int_umin(ctx, indices_len, ndarray_len, None);
        gen_for_callback_incrementing(
            &mut (),
            ctx,
            None,
            llvm_usize.const_zero(),
            (len, false),
            |(), ctx, _, i| {
                let (dim_idx, dim_sz) = unsafe {
                    (
                        indices.get_unchecked(ctx, &i, None).into_int_value(),
                        self.0.shape().get_typed_unchecked(ctx, &i, None),
                    )
                };
                let dim_idx = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(dim_idx, dim_sz.get_type(), "")
                    .unwrap();

                let dim_lt =
                    ctx.builder.build_int_compare(IntPredicate::SLT, dim_idx, dim_sz, "").unwrap();

                ctx.make_assert(
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

        let ptr = unsafe { self.ptr_offset_unchecked(ctx, indices, name) };
        // TODO: Current implementation is transparent
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx))
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
    let mut strides = vec![0u64; ndims as usize];
    let mut stride_product = 1u64;
    for axis in (0..ndims).rev() {
        strides[axis as usize] = stride_product * itemsize;
        stride_product *= shape[axis as usize];
    }
    strides
}

/// A convenience enum for implementing functions that acts on scalars or ndarrays or both.
#[derive(Clone, Copy)]
pub enum ScalarOrNDArray<'ctx> {
    Scalar(BasicValueEnum<'ctx>),
    NDArray(NDArrayValue<'ctx>),
}

impl<'ctx> TryFrom<&ScalarOrNDArray<'ctx>> for BasicValueEnum<'ctx> {
    type Error = ();

    fn try_from(value: &ScalarOrNDArray<'ctx>) -> Result<Self, Self::Error> {
        match value {
            ScalarOrNDArray::Scalar(scalar) => Ok(*scalar),
            ScalarOrNDArray::NDArray(_) => Err(()),
        }
    }
}

impl<'ctx> TryFrom<&ScalarOrNDArray<'ctx>> for NDArrayValue<'ctx> {
    type Error = ();

    fn try_from(value: &ScalarOrNDArray<'ctx>) -> Result<Self, Self::Error> {
        match value {
            ScalarOrNDArray::Scalar(_) => Err(()),
            ScalarOrNDArray::NDArray(ndarray) => Ok(*ndarray),
        }
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Split on `object` either into a scalar or an ndarray.
    ///
    /// If `object` is an ndarray, [`ScalarOrNDArray::NDArray`].
    ///
    /// For everything else, it is wrapped with [`ScalarOrNDArray::Scalar`].
    pub fn from_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (object_ty, object): (Type, BasicValueEnum<'ctx>),
    ) -> ScalarOrNDArray<'ctx> {
        match &*ctx.unifier.get_ty(object_ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayType::from_unifier_type(ctx, object_ty)
                    .map_pointer_value(object.into_pointer_value(), None);
                ScalarOrNDArray::NDArray(ndarray)
            }

            _ => ScalarOrNDArray::Scalar(object),
        }
    }

    /// Get the underlying [`BasicValueEnum<'ctx>`] of this [`ScalarOrNDArray`].
    #[must_use]
    pub fn to_basic_value_enum(self) -> BasicValueEnum<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(scalar) => scalar,
            ScalarOrNDArray::NDArray(ndarray) => ndarray.value.into(),
        }
    }

    /// Convert this [`ScalarOrNDArray`] to an ndarray - behaves like `np.asarray`.
    ///
    /// - If this is an ndarray, the ndarray is returned.
    /// - If this is a scalar, this function returns new ndarray created with
    ///   [`NDArrayType::construct_unsized`].
    pub fn to_ndarray(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> NDArrayValue<'ctx> {
        match self {
            ScalarOrNDArray::NDArray(ndarray) => *ndarray,
            ScalarOrNDArray::Scalar(scalar) => NDArrayType::new_unsized(ctx, scalar.get_type())
                .construct_unsized(ctx, scalar, None),
        }
    }

    /// Get the dtype of the ndarray created if this were called with
    /// [`ScalarOrNDArray::to_ndarray`].
    #[must_use]
    pub fn get_dtype(&self) -> BasicTypeEnum<'ctx> {
        match self {
            ScalarOrNDArray::NDArray(ndarray) => ndarray.dtype,
            ScalarOrNDArray::Scalar(scalar) => scalar.get_type(),
        }
    }
}

/// An helper enum specifying how a function should produce its output.
///
/// Many functions in NumPy has an optional `out` parameter (e.g., `matmul`). If `out` is specified
/// with an ndarray, the result of a function will be written to `out`. If `out` is not specified, a
/// function will create a new ndarray and store the result in it.
#[derive(Clone, Copy)]
pub enum NDArrayOut<'ctx> {
    /// Tell a function should create a new ndarray with the expected element type `dtype`.
    NewNDArray { dtype: BasicTypeEnum<'ctx> },
    /// Tell a function to write the result to `ndarray`.
    WriteToNDArray { ndarray: NDArrayValue<'ctx> },
}

impl<'ctx> NDArrayOut<'ctx> {
    /// Get the dtype of this output.
    #[must_use]
    pub fn get_dtype(&self) -> BasicTypeEnum<'ctx> {
        match self {
            NDArrayOut::NewNDArray { dtype } => *dtype,
            NDArrayOut::WriteToNDArray { ndarray } => ndarray.dtype,
        }
    }
}
