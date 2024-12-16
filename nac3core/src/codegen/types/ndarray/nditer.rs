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
    /// Checks whether `llvm_ty` represents a `nditer` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let ctx = llvm_ty.get_context();

        let llvm_ty = llvm_ty.get_element_type();
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

    /// Creates an instance of [`NDIter`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(generator: &G, ctx: &'ctx Context) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_nditer = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_nditer, llvm_usize }
    }

    /// Creates an [`NDIterType`] from a [`PointerType`] representing an `NDIter`.
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of the `size` field of this `nditer` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.llvm_usize
    }

    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(generator, ctx, name),
            parent,
            indices,
            self.llvm_usize,
            name,
        )
    }

    /// Allocate an [`NDIter`] that iterates through the given `ndarray`.
    ///
    /// Note: This function allocates an array on the stack at the current builder location, which
    /// may lead to stack explosion if called in a hot loop. Therefore, callers are recommended to
    /// call `llvm.stacksave` before calling this function and call `llvm.stackrestore` after the
    /// [`NDIter`] is no longer needed.
    #[must_use]
    pub fn construct<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayValue<'ctx>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let nditer = self.raw_alloca(generator, ctx, None);
        let ndims = ndarray.load_ndims(ctx);

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
    type Base = PointerType<'ctx>;
    type Value = NDIterValue<'ctx>;

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
    ) -> PointerValue<'ctx> {
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

impl<'ctx> From<NDIterType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDIterType<'ctx>) -> Self {
        value.as_base_type()
    }
}
