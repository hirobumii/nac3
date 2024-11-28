use inkwell::{
    context::{AsContextRef, Context},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::{
    structure::{check_struct_type_matches_fields, StructField, StructFields},
    ProxyType,
};
use crate::{
    codegen::{
        values::{ndarray::NDArrayValue, ArraySliceValue, ProxyValue, TypedArrayLikeMutator},
        {CodeGenContext, CodeGenerator},
    },
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys},
    typecheck::typedef::Type,
};
pub use contiguous::*;

mod contiguous;

/// Proxy type for a `ndarray` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDArrayType<'ctx> {
    ty: PointerType<'ctx>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: Option<u64>,
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
    /// Checks whether `llvm_ty` represents a `ndarray` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let ctx = llvm_ty.get_context();

        let llvm_ndarray_ty = llvm_ty.get_element_type();
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

    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(
        ctx: impl AsContextRef<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> NDArrayStructFields<'ctx> {
        NDArrayStructFields::new(ctx, llvm_usize)
    }

    /// See [`NDArrayType::fields`].
    // TODO: Move this into e.g. StructProxyType
    #[must_use]
    pub fn get_fields(&self, ctx: impl AsContextRef<'ctx>) -> NDArrayStructFields<'ctx> {
        Self::fields(ctx, self.llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of an `NDArray`.
    #[must_use]
    fn llvm_type(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    /// Creates an instance of [`NDArrayType`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        dtype: BasicTypeEnum<'ctx>,
        ndims: Option<u64>,
    ) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_ndarray = Self::llvm_type(ctx, llvm_usize);

        NDArrayType { ty: llvm_ndarray, dtype, ndims, llvm_usize }
    }

    /// Creates an [`NDArrayType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ty: Type,
    ) -> Self {
        let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);

        let llvm_dtype = ctx.get_llvm_type(generator, dtype);
        let llvm_usize = generator.get_size_type(ctx.ctx);
        let ndims = extract_ndims(&ctx.unifier, ndims);

        NDArrayType {
            ty: Self::llvm_type(ctx.ctx, llvm_usize),
            dtype: llvm_dtype,
            ndims: Some(ndims),
            llvm_usize,
        }
    }

    /// Creates an [`NDArrayType`] from a [`PointerType`] representing an `NDArray`.
    #[must_use]
    pub fn from_type(
        ptr_ty: PointerType<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        ndims: Option<u64>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

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
    pub fn ndims(&self) -> Option<u64> {
        self.ndims
    }

    /// Allocates an instance of [`NDArrayValue`] as if by calling `alloca` on the base type.
    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(generator, ctx, name),
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
        let ndarray = self.alloca(generator, ctx, name);

        let itemsize = ctx
            .builder
            .build_int_truncate_or_bit_cast(self.dtype.size_of().unwrap(), self.llvm_usize, "")
            .unwrap();
        ndarray.store_itemsize(ctx, generator, itemsize);

        ndarray.store_ndims(ctx, generator, ndims);

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
        assert!(self.ndims.is_some(), "NDArrayType::construct can only be called on an instance with compile-time known ndims (self.ndims = Some(ndims))");

        let Some(ndims) = self.ndims.map(|ndims| self.llvm_usize.const_int(ndims, false)) else {
            unreachable!()
        };

        self.construct_impl(generator, ctx, ndims, name)
    }

    /// Allocate an [`NDArrayValue`] on the stack given its `ndims` and `dtype`.
    ///
    /// `shape` and `strides` will be automatically allocated onto the stack.
    ///
    /// The returned ndarray's content will be:
    /// - `data`: uninitialized.
    /// - `itemsize`: set to the size of `dtype`.
    /// - `ndims`: set to the value of `ndims`.
    /// - `shape`: allocated with an array of length `ndims` with uninitialized values.
    /// - `strides`: allocated with an array of length `ndims` with uninitialized values.
    #[deprecated = "Prefer construct_uninitialized or construct_*_shape."]
    #[must_use]
    pub fn construct_dyn_ndims<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndims: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        assert!(self.ndims.is_none(), "NDArrayType::construct_dyn_ndims can only be called on an instance with compile-time unknown ndims (self.ndims = None)");

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
        assert!(self.ndims.is_none_or(|ndims| shape.len() as u64 == ndims));

        let ndarray = Self::new(generator, ctx.ctx, self.dtype, Some(shape.len() as u64))
            .construct_uninitialized(generator, ctx, name);

        let llvm_usize = generator.get_size_type(ctx.ctx);

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
        assert!(self.ndims.is_none_or(|ndims| shape.len() as u64 == ndims));

        let ndarray = Self::new(generator, ctx.ctx, self.dtype, Some(shape.len() as u64))
            .construct_uninitialized(generator, ctx, name);

        let llvm_usize = generator.get_size_type(ctx.ctx);

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

    /// Converts an existing value into a [`NDArrayValue`].
    #[must_use]
    pub fn map_value(
        &self,
        value: <<Self as ProxyType<'ctx>>::Value as ProxyValue<'ctx>>::Base,
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
    type Base = PointerType<'ctx>;
    type Value = NDArrayValue<'ctx>;

    fn is_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: impl BasicType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            <Self as ProxyType<'ctx>>::is_representable(generator, ctx, ty)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String> {
        Self::is_representable(llvm_ty, generator.get_size_type(ctx))
    }

    fn raw_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self::Value as ProxyValue<'ctx>>::Base {
        generator
            .gen_var_alloc(
                ctx,
                self.as_base_type().get_element_type().into_struct_type().into(),
                name,
            )
            .unwrap()
    }

    fn array_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        generator
            .gen_array_var_alloc(
                ctx,
                self.as_base_type().get_element_type().into_struct_type().into(),
                size,
                name,
            )
            .unwrap()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<NDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}
