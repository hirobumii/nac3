use inkwell::{
    AddressSpace,
    context::ContextRef,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{BasicValue, IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::{
    ProxyType,
    structure::{StructField, StructFields, StructProxyType, check_struct_type_matches_fields},
};
use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, CoreContext,
        values::{TypedArrayLikeMutator, ndarray::NDArrayValue},
    },
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys},
    typecheck::typedef::Type,
};
pub use broadcast::*;
pub use contiguous::*;
pub use indexing::*;
pub use nditer::*;

mod array;
mod broadcast;
mod contiguous;
pub mod factory;
mod indexing;
mod map;
mod nditer;

/// Proxy type for a `ndarray` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDArrayType<'ctx> {
    ty: PointerType<'ctx>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: u64,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct NDArrayStructFields<'ctx> {
    /// The size of each `NDArray` element in bytes.
    #[value_type(usize)]
    pub itemsize: StructField<'ctx, IntValue<'ctx>>,
    /// Number of dimensions in the array.
    #[value_type(usize)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    /// Pointer to an array containing the shape of the `NDArray`.
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array indicating the number of bytes between each element at a dimension
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub strides: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array containing the array data
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

impl<'ctx> NDArrayType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> NDArrayStructFields<'ctx> {
        NDArrayStructFields::new(ctx, llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of an `NDArray`.
    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(
        ctx: ContextRef<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: u64,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        let llvm_ndarray = Self::llvm_type(ctx, llvm_usize);

        NDArrayType { ty: llvm_ndarray, dtype, ndims, llvm_usize }
    }

    /// Creates an instance of [`NDArrayType`].
    #[must_use]
    pub fn new(ctx: &CoreContext<'ctx>, dtype: BasicTypeEnum<'ctx>, ndims: u64) -> Self {
        Self::new_impl(ctx.ctx, dtype, ndims, ctx.size_t)
    }

    /// Creates an instance of [`NDArrayType`] as a result of a broadcast operation over one or more
    /// `ndarray` operands.
    #[must_use]
    pub fn new_broadcast(
        ctx: &CodeGenContext<'ctx, '_>,
        dtype: BasicTypeEnum<'ctx>,
        inputs: &[NDArrayType<'ctx>],
    ) -> Self {
        assert!(!inputs.is_empty());

        Self::new_impl(
            ctx.ctx,
            dtype,
            inputs.iter().map(NDArrayType::ndims).max().unwrap(),
            ctx.size_t,
        )
    }

    /// Creates an instance of [`NDArrayType`] with `ndims` of 0.
    #[must_use]
    pub fn new_unsized(ctx: &CodeGenContext<'ctx, '_>, dtype: BasicTypeEnum<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, dtype, 0, ctx.size_t)
    }

    /// Creates an [`NDArrayType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);

        let llvm_dtype = ctx.get_llvm_type(dtype);
        let ndims = extract_ndims(&ctx.unifier, ndims);

        Self::new_impl(ctx.ctx, llvm_dtype, ndims, ctx.size_t)
    }

    /// Creates an [`NDArrayType`] from a [`StructType`] representing an `NDArray`.
    #[must_use]
    pub fn from_struct_type(
        ty: StructType<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: u64,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        Self::from_pointer_type(ty.ptr_type(AddressSpace::default()), dtype, ndims, llvm_usize)
    }

    /// Creates an [`NDArrayType`] from a [`PointerType`] representing an `NDArray`.
    #[must_use]
    pub fn from_pointer_type(
        ptr_ty: PointerType<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: u64,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        NDArrayType { ty: ptr_ty, dtype, ndims, llvm_usize }
    }

    /// Returns the type of the `size` field of this `ndarray` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.llvm_usize
    }

    /// Returns the element type of this `ndarray` type.
    #[must_use]
    pub fn element_type(&self) -> BasicTypeEnum<'ctx> {
        self.dtype
    }

    /// Returns the number of dimensions of this `ndarray` type.
    #[must_use]
    pub fn ndims(&self) -> u64 {
        self.ndims
    }

    /// Allocates an instance of [`NDArrayValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca`].
    #[must_use]
    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(ctx, name),
            self.dtype,
            self.ndims,
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an instance of [`NDArrayValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca_var`].
    #[must_use]
    pub fn alloca_var<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(generator, ctx, name),
            self.dtype,
            self.ndims,
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an [`NDArrayValue`] on the stack and initializes all fields as follows:
    ///
    /// - `data`: uninitialized.
    /// - `itemsize`: set to the size of `self.dtype`.
    /// - `ndims`: set to the value of  `ndims`.
    /// - `shape`: allocated on the stack with an array of length `ndims` with uninitialized values.
    /// - `strides`: allocated on the stack with an array of length `ndims` with uninitialized
    ///   values.
    #[must_use]
    fn construct_impl<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndims: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let ndarray = self.alloca_var(generator, ctx, name);

        let itemsize = ctx
            .builder
            .build_int_truncate_or_bit_cast(self.dtype.size_of().unwrap(), self.llvm_usize, "")
            .unwrap();
        ndarray.store_itemsize(ctx, itemsize);

        ndarray.store_ndims(ctx, ndims);

        ndarray.create_shape(ctx, self.llvm_usize, ndims);
        ndarray.create_strides(ctx, self.llvm_usize, ndims);

        ndarray
    }

    /// Allocate an [`NDArrayValue`] on the stack using `dtype` and `ndims` of this [`NDArrayType`]
    /// instance.
    ///
    /// The returned ndarray's content will be:
    /// - `data`: uninitialized.
    /// - `itemsize`: set to the size of `dtype`.
    /// - `ndims`: set to the value of `self.ndims`.
    /// - `shape`: allocated on the stack with an array of length `ndims` with uninitialized values.
    /// - `strides`: allocated on the stack with an array of length `ndims` with uninitialized
    ///   values.
    #[must_use]
    pub fn construct_uninitialized<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let ndims = self.llvm_usize.const_int(self.ndims, false);

        self.construct_impl(generator, ctx, ndims, name)
    }

    /// Convenience function. Allocate an [`NDArrayValue`] with a statically known shape.
    ///
    /// The returned [`NDArrayValue`]'s `data` and `strides` are uninitialized.
    #[must_use]
    pub fn construct_const_shape<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &[u64],
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(shape.len() as u64, self.ndims);

        let ndarray = Self::new(ctx, self.dtype, shape.len() as u64)
            .construct_uninitialized(generator, ctx, name);

        let llvm_usize = ctx.size_t;

        // Write shape
        let ndarray_shape = ndarray.shape();
        for (i, dim) in shape.iter().enumerate() {
            let dim = llvm_usize.const_int(*dim, false);
            unsafe {
                ndarray_shape.set_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(i as u64, false),
                    dim,
                );
            }
        }

        ndarray
    }

    /// Convenience function. Allocate an [`NDArrayValue`] with a dynamically known shape.
    ///
    /// The returned [`NDArrayValue`]'s `data` and `strides` are uninitialized.
    #[must_use]
    pub fn construct_dyn_shape<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &[IntValue<'ctx>],
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert_eq!(shape.len() as u64, self.ndims);

        let ndarray = Self::new(ctx, self.dtype, shape.len() as u64)
            .construct_uninitialized(generator, ctx, name);

        let llvm_usize = ctx.size_t;

        // Write shape
        let ndarray_shape = ndarray.shape();
        for (i, dim) in shape.iter().enumerate() {
            assert_eq!(
                dim.get_type(),
                llvm_usize,
                "Expected {} but got {}",
                llvm_usize.print_to_string(),
                dim.get_type().print_to_string()
            );
            unsafe {
                ndarray_shape.set_typed_unchecked(
                    ctx,
                    generator,
                    &llvm_usize.const_int(i as u64, false),
                    *dim,
                );
            }
        }

        ndarray
    }

    /// Create an unsized ndarray to contain `value`.
    #[must_use]
    pub fn construct_unsized<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: &impl BasicValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> NDArrayValue<'ctx> {
        let value = value.as_basic_value_enum();

        assert_eq!(value.get_type(), self.dtype);
        assert_eq!(self.ndims, 0);

        // We have to put the value on the stack to get a data pointer.
        let data = ctx.builder.build_alloca(value.get_type(), "construct_unsized").unwrap();
        ctx.builder.build_store(data, value).unwrap();
        let data = ctx
            .builder
            .build_pointer_cast(data, ctx.ctx.i8_type().ptr_type(AddressSpace::default()), "")
            .unwrap();

        let ndarray =
            Self::new_unsized(ctx, value.get_type()).construct_uninitialized(generator, ctx, name);
        ctx.builder.build_store(ndarray.ptr_to_data(ctx), data).unwrap();
        ndarray
    }

    /// Converts an existing value into a [`NDArrayValue`].
    #[must_use]
    pub fn map_struct_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(
            generator,
            ctx,
            value,
            self.dtype,
            self.ndims,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`NDArrayValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            value,
            self.dtype,
            self.ndims,
            self.llvm_usize,
            name,
        )
    }
}

impl<'ctx> ProxyType<'ctx> for NDArrayType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = NDArrayValue<'ctx>;

    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            Self::has_same_repr(ty, llvm_usize)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn has_same_repr(ty: Self::Base, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        let ctx = ty.get_context();

        let llvm_ndarray_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ndarray_ty) = llvm_ndarray_ty else {
            return Err(format!("Expected struct type for `NDArray` type, got {llvm_ndarray_ty}"));
        };

        check_struct_type_matches_fields(
            Self::fields(ctx, llvm_usize),
            llvm_ndarray_ty,
            "NDArray",
            &[],
        )
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_abi_type().get_element_type().into_struct_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> StructProxyType<'ctx> for NDArrayType<'ctx> {
    type StructFields = NDArrayStructFields<'ctx>;

    fn get_fields(&self) -> Self::StructFields {
        Self::fields(self.ty.get_context(), self.llvm_usize)
    }
}

impl<'ctx> From<NDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}
