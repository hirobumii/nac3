use std::iter::repeat_n;

use inkwell::{
    types::{AnyType, AnyTypeEnum, BasicType, BasicTypeEnum, IntType},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};
use itertools::Itertools;

use super::{
    ArrayLikeIndexer, ArrayLikeValue, ProxyValue, TupleValue, TypedArrayLikeAccessor,
    TypedArrayLikeAdapter, TypedArrayLikeMutator, UntypedArrayLikeAccessor,
    UntypedArrayLikeMutator,
};
use crate::{
    codegen::{
        irrt,
        llvm_intrinsics::{call_int_umin, call_memcpy_generic_array},
        stmt::gen_for_callback_incrementing,
        type_aligned_alloca,
        types::{ndarray::NDArrayType, structure::StructField, TupleType},
        CodeGenContext, CodeGenerator,
    },
    typecheck::typedef::{Type, TypeEnum},
};
pub use broadcast::*;
pub use contiguous::*;
pub use indexing::*;
pub use nditer::*;

mod broadcast;
mod contiguous;
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
        ndims: u64,
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
    pub fn store_ndims(&self, ctx: &CodeGenContext<'ctx, '_>, ndims: IntValue<'ctx>) {
        debug_assert_eq!(ndims.get_type(), ctx.get_size_type());

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
    pub fn store_itemsize(&self, ctx: &CodeGenContext<'ctx, '_>, itemsize: IntValue<'ctx>) {
        debug_assert_eq!(itemsize.get_type(), ctx.get_size_type());

        self.itemsize_field(ctx).set(ctx, self.value, itemsize, self.name);
    }

    /// Returns the size of each element of this `NDArray` as a value.
    pub fn load_itemsize(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.itemsize_field(ctx).get(ctx, self.value, self.name)
    }

    fn shape_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).shape
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
    pub fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
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
        let nbytes = self.nbytes(ctx);

        let data = type_aligned_alloca(generator, ctx, self.dtype, nbytes, None);
        self.store_data(ctx, data);

        self.set_strides_contiguous(ctx);
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
        assert_eq!(self.ndims, src_ndarray.ndims);

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
        assert_eq!(self.ndims, src_ndarray.ndims);

        let src_strides = src_ndarray.strides().base_ptr(ctx, generator);
        self.copy_strides_from_array(generator, ctx, src_strides);
    }

    /// Get the `np.size()` of this ndarray.
    pub fn size(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_size(ctx, *self)
    }

    /// Get the `ndarray.nbytes` of this ndarray.
    pub fn nbytes(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_nbytes(ctx, *self)
    }

    /// Get the `len()` of this ndarray.
    pub fn len(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_len(ctx, *self)
    }

    /// Check if this ndarray is C-contiguous.
    ///
    /// See NumPy's `flags["C_CONTIGUOUS"]`: <https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html#numpy.ndarray.flags>
    pub fn is_c_contiguous(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_is_c_contiguous(ctx, *self)
    }

    /// Call [`call_nac3_ndarray_set_strides_by_shape`] on this ndarray to update `strides`.
    ///
    /// Update the ndarray's strides to make the ndarray contiguous.
    pub fn set_strides_contiguous(&self, ctx: &CodeGenContext<'ctx, '_>) {
        irrt::ndarray::call_nac3_ndarray_set_strides_by_shape(ctx, *self);
    }

    /// Clone/Copy this ndarray - Allocate a new ndarray with the same shape as this ndarray and
    /// copy the contents over.
    ///
    /// The new ndarray will own its data and will be C-contiguous.
    #[must_use]
    pub fn make_copy<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Self {
        let clone = self.get_type().construct_uninitialized(generator, ctx, None);

        let shape = self.shape();
        clone.copy_shape_from_array(generator, ctx, shape.base_ptr(ctx, generator));
        unsafe { clone.create_data(generator, ctx) };
        clone.copy_data_from(ctx, *self);
        clone
    }

    /// Copy data from another ndarray.
    ///
    /// This ndarray and `src` is that their `np.size()` should be the same. Their shapes
    /// do not matter. The copying order is determined by how their flattened views look.
    ///
    /// Panics if the `dtype`s of ndarrays are different.
    pub fn copy_data_from(&self, ctx: &CodeGenContext<'ctx, '_>, src: NDArrayValue<'ctx>) {
        assert_eq!(self.dtype, src.dtype, "self and src dtype should match");
        irrt::ndarray::call_nac3_ndarray_copy_data(ctx, src, *self);
    }

    /// Fill the ndarray with a scalar.
    ///
    /// `fill_value` must have the same LLVM type as the `dtype` of this ndarray.
    pub fn fill<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) {
        // TODO: It is possible to optimize this by exploiting contiguous strides with memset.
        //       Probably best to implement in IRRT.
        self.foreach(generator, ctx, |_, ctx, _, nditer| {
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
    pub fn make_shape_tuple<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> TupleValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();

        let objects = (0..self.ndims)
            .map(|i| {
                let dim = unsafe {
                    self.shape().get_typed_unchecked(
                        ctx,
                        generator,
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
    pub fn make_strides_tuple<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> TupleValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();

        let objects = (0..self.ndims)
            .map(|i| {
                let dim = unsafe {
                    self.strides().get_typed_unchecked(
                        ctx,
                        generator,
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
    pub fn get_unsized_element<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Option<BasicValueEnum<'ctx>> {
        if self.is_unsized() {
            // NOTE: `np.size(self) == 0` here is never possible.
            let zero = ctx.get_size_type().const_zero();
            let value = unsafe { self.data().get_unchecked(ctx, generator, &zero, None) };

            Some(value)
        } else {
            None
        }
    }

    /// If this ndarray is unsized, return its sole value as an [`BasicValueEnum`].
    /// Otherwise, do nothing and return the ndarray itself.
    pub fn split_unsized<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> ScalarOrNDArray<'ctx> {
        if let Some(unsized_elem) = self.get_unsized_element(generator, ctx) {
            ScalarOrNDArray::Scalar(unsized_elem)
        } else {
            ScalarOrNDArray::NDArray(*self)
        }
    }

    /// Check if this `NDArray` can be used as an `out` ndarray for an operation.
    ///
    /// Raise an exception if the shapes do not match.
    pub fn assert_can_be_written_by_out<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        out_shape: impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    ) {
        let ndarray_shape = self.shape();
        let output_shape = out_shape;

        irrt::ndarray::call_nac3_ndarray_util_assert_output_shape_same(
            generator,
            ctx,
            &ndarray_shape,
            &output_shape,
        );
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
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
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

impl<'ctx, G: CodeGenerator + ?Sized> TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>
    for NDArrayShapeProxy<'ctx, '_>
{
    fn downcast_to_type(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx, G: CodeGenerator + ?Sized> TypedArrayLikeMutator<'ctx, G, IntValue<'ctx>>
    for NDArrayShapeProxy<'ctx, '_>
{
    fn upcast_from_type(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
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
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
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

impl<'ctx, G: CodeGenerator + ?Sized> TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>
    for NDArrayStridesProxy<'ctx, '_>
{
    fn downcast_to_type(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx, G: CodeGenerator + ?Sized> TypedArrayLikeMutator<'ctx, G, IntValue<'ctx>>
    for NDArrayStridesProxy<'ctx, '_>
{
    fn upcast_from_type(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
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
        _: &G,
    ) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_ndarray_len(ctx, *self.0)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for NDArrayDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
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
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                name.unwrap_or_default(),
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
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        assert_eq!(indices.element_type(ctx, generator), ctx.get_size_type().into());

        let indices = TypedArrayLikeAdapter::from(
            indices.as_slice_value(ctx, generator),
            |_, _, v| v.into_int_value(),
            |_, _, v| v.into(),
        );

        let ptr = irrt::ndarray::call_nac3_ndarray_get_pelement_by_indices(
            generator, ctx, *self.0, &indices,
        );

        // Current implementation is transparent - The returned pointer type is
        // already cast into the expected type, allowing for immediately
        // load/store.
        ctx.builder
            .build_pointer_cast(
                ptr,
                BasicTypeEnum::try_from(self.element_type(ctx, generator))
                    .unwrap()
                    .ptr_type(AddressSpace::default()),
                name.unwrap_or_default(),
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
        let llvm_usize = ctx.get_size_type();

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
    pub fn from_value<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        (object_ty, object): (Type, BasicValueEnum<'ctx>),
    ) -> ScalarOrNDArray<'ctx> {
        match &*ctx.unifier.get_ty(object_ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayType::from_unifier_type(generator, ctx, object_ty)
                    .map_value(object.into_pointer_value(), None);
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
            ScalarOrNDArray::NDArray(ndarray) => ndarray.as_base_value().into(),
        }
    }

    /// Convert this [`ScalarOrNDArray`] to an ndarray - behaves like `np.asarray`.
    ///
    /// - If this is an ndarray, the ndarray is returned.
    /// - If this is a scalar, this function returns new ndarray created with
    ///   [`NDArrayType::construct_unsized`].
    pub fn to_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> NDArrayValue<'ctx> {
        match self {
            ScalarOrNDArray::NDArray(ndarray) => *ndarray,
            ScalarOrNDArray::Scalar(scalar) => NDArrayType::new_unsized(ctx, scalar.get_type())
                .construct_unsized(generator, ctx, scalar, None),
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
