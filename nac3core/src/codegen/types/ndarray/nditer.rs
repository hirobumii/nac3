use inkwell::{
    context::{AsContextRef, Context},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::ProxyType;
use crate::codegen::{
    irrt,
    types::structure::{check_struct_type_matches_fields, StructField, StructFields},
    values::{
        ndarray::{NDArrayValue, NDIterValue},
        ArrayLikeValue, ArraySliceValue, ProxyValue, TypedArrayLikeAdapter,
    },
    CodeGenContext, CodeGenerator,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDIterType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct NDIterStructFields<'ctx> {
    #[value_type(usize)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub strides: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub indices: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(usize)]
    pub nth: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub element: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(usize)]
    pub size: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> NDIterType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(ctx: impl AsContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> NDIterStructFields<'ctx> {
        NDIterStructFields::new(ctx, llvm_usize)
    }

    /// See [`NDIterType::fields`].
    // TODO: Move this into e.g. StructProxyType
    #[must_use]
    pub fn get_fields(&self, ctx: impl AsContextRef<'ctx>) -> NDIterStructFields<'ctx> {
        Self::fields(ctx, self.llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of an `NDIter`.
    #[must_use]
    fn llvm_type(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_nditer = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_nditer, llvm_usize }
    }

    /// Creates an instance of [`NDIter`].
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>) -> Self {
        Self::new_impl(ctx.ctx, ctx.get_size_type())
    }

    /// Creates an instance of [`NDIter`].
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
    ) -> Self {
        Self::new_impl(ctx, generator.get_size_type(ctx))
    }

    /// Creates an [`NDIterType`] from a [`PointerType`] representing an `NDIter`.
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of the `size` field of this `nditer` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.llvm_usize
    }

    /// Allocates an instance of [`NDIterValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca`].
    #[must_use]
    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(ctx, name),
            parent,
            indices,
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an instance of [`NDIterValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca_var`].
    #[must_use]
    pub fn alloca_var<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(generator, ctx, name),
            parent,
            indices,
            self.llvm_usize,
            name,
        )
    }

    /// Allocate an [`NDIter`] that iterates through the given `ndarray`.
    #[must_use]
    pub fn construct<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayValue<'ctx>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let nditer = self.raw_alloca_var(generator, ctx, None);
        let ndims = self.llvm_usize.const_int(ndarray.get_type().ndims(), false);

        // The caller has the responsibility to allocate 'indices' for `NDIter`.
        let indices =
            generator.gen_array_var_alloc(ctx, self.llvm_usize.into(), ndims, None).unwrap();
        let indices =
            TypedArrayLikeAdapter::from(indices, |_, _, v| v.into_int_value(), |_, _, v| v.into());

        let nditer = self.map_value(nditer, ndarray, indices.as_slice_value(ctx, generator), None);

        irrt::ndarray::call_nac3_nditer_initialize(generator, ctx, nditer, ndarray, &indices);

        nditer
    }

    #[must_use]
    pub fn map_value(
        &self,
        value: <<Self as ProxyType<'ctx>>::Value as ProxyValue<'ctx>>::Base,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            value,
            parent,
            indices,
            self.llvm_usize,
            name,
        )
    }
}

impl<'ctx> ProxyType<'ctx> for NDIterType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = NDIterValue<'ctx>;

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

        let llvm_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ndarray_ty) = llvm_ty else {
            return Err(format!("Expected struct type for `NDIter` type, got {llvm_ty}"));
        };

        check_struct_type_matches_fields(
            Self::fields(ctx, llvm_usize),
            llvm_ndarray_ty,
            "NDIter",
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

impl<'ctx> From<NDIterType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDIterType<'ctx>) -> Self {
        value.as_base_type()
    }
}
