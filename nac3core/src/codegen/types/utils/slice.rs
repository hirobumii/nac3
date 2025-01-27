use inkwell::{
    context::{AsContextRef, Context, ContextRef},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{IntValue, PointerValue, StructValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use crate::codegen::{
    types::{
        structure::{
            check_struct_type_matches_fields, FieldIndexCounter, StructField, StructFields,
            StructProxyType,
        },
        ProxyType,
    },
    values::utils::SliceValue,
    CodeGenContext, CodeGenerator,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SliceType<'ctx> {
    ty: PointerType<'ctx>,
    int_ty: IntType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct SliceStructFields<'ctx> {
    #[value_type(bool_type())]
    pub start_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize)]
    pub start: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(bool_type())]
    pub stop_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize)]
    pub stop: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(bool_type())]
    pub step_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize)]
    pub step: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> SliceStructFields<'ctx> {
    /// Creates a new instance of [`SliceStructFields`] with a custom integer type for its range values.
    #[must_use]
    pub fn new_sized(ctx: &impl AsContextRef<'ctx>, int_ty: IntType<'ctx>) -> Self {
        let ctx = unsafe { ContextRef::new(ctx.as_ctx_ref()) };
        let mut counter = FieldIndexCounter::default();

        SliceStructFields {
            start_defined: StructField::create(&mut counter, "start_defined", ctx.bool_type()),
            start: StructField::create(&mut counter, "start", int_ty),
            stop_defined: StructField::create(&mut counter, "stop_defined", ctx.bool_type()),
            stop: StructField::create(&mut counter, "stop", int_ty),
            step_defined: StructField::create(&mut counter, "step_defined", ctx.bool_type()),
            step: StructField::create(&mut counter, "step", int_ty),
        }
    }
}

impl<'ctx> SliceType<'ctx> {
    /// Creates an LLVM type corresponding to the expected structure of a `Slice`.
    #[must_use]
    fn llvm_type(ctx: &'ctx Context, int_ty: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys = SliceStructFields::new_sized(&int_ty.get_context(), int_ty)
            .into_iter()
            .map(|field| field.1)
            .collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: &'ctx Context, int_ty: IntType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_ty = Self::llvm_type(ctx, int_ty);

        Self { ty: llvm_ty, int_ty, llvm_usize }
    }

    /// Creates an instance of [`SliceType`] with `int_ty` as its backing integer type.
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>, int_ty: IntType<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, int_ty, ctx.get_size_type())
    }

    /// Creates an instance of [`SliceType`] with `int_ty` as its backing integer type.
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        int_ty: IntType<'ctx>,
    ) -> Self {
        Self::new_impl(ctx, int_ty, generator.get_size_type(ctx))
    }

    /// Creates an instance of [`SliceType`] with `usize` as its backing integer type.
    #[must_use]
    pub fn new_usize(ctx: &CodeGenContext<'ctx, '_>) -> Self {
        Self::new_impl(ctx.ctx, ctx.get_size_type(), ctx.get_size_type())
    }

    /// Creates an instance of [`SliceType`] with `usize` as its backing integer type.
    #[must_use]
    pub fn new_usize_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
    ) -> Self {
        Self::new_impl(ctx, generator.get_size_type(ctx), generator.get_size_type(ctx))
    }

    /// Creates an [`SliceType`] from a [`StructType`] representing a `slice`.
    #[must_use]
    pub fn from_struct_type(
        ty: StructType<'ctx>,
        int_ty: IntType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        Self::from_pointer_type(ty.ptr_type(AddressSpace::default()), int_ty, llvm_usize)
    }

    /// Creates an [`SliceType`] from a [`PointerType`] representing a `slice`.
    #[must_use]
    pub fn from_pointer_type(
        ptr_ty: PointerType<'ctx>,
        int_ty: IntType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, int_ty).is_ok());

        Self { ty: ptr_ty, int_ty, llvm_usize }
    }

    #[must_use]
    pub fn element_type(&self) -> IntType<'ctx> {
        self.int_ty
    }

    /// Allocates an instance of [`SliceValue`] as if by calling `alloca` on the base type.
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
            self.int_ty,
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an instance of [`SliceValue`] as if by calling `alloca` on the base type.
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
            self.int_ty,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`SliceValue`].
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
            self.int_ty,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`ContiguousNDArrayValue`].
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            value,
            self.int_ty,
            self.llvm_usize,
            name,
        )
    }
}

impl<'ctx> ProxyType<'ctx> for SliceType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = SliceValue<'ctx>;

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

        let fields = SliceStructFields::new(ctx, llvm_usize);

        let llvm_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ty) = llvm_ty else {
            return Err(format!("Expected struct type for `Slice` type, got {llvm_ty}"));
        };

        check_struct_type_matches_fields(
            fields,
            llvm_ty,
            "Slice",
            &[
                (fields.start.name(), &|ty| {
                    if ty.is_int_type() {
                        Ok(())
                    } else {
                        Err(format!("Expected int type for `Slice.start`, got {ty}"))
                    }
                }),
                (fields.stop.name(), &|ty| {
                    if ty.is_int_type() {
                        Ok(())
                    } else {
                        Err(format!("Expected int type for `Slice.stop`, got {ty}"))
                    }
                }),
                (fields.step.name(), &|ty| {
                    if ty.is_int_type() {
                        Ok(())
                    } else {
                        Err(format!("Expected int type for `Slice.step`, got {ty}"))
                    }
                }),
            ],
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

impl<'ctx> StructProxyType<'ctx> for SliceType<'ctx> {
    type StructFields = SliceStructFields<'ctx>;

    fn get_fields(&self) -> Self::StructFields {
        SliceStructFields::new_sized(&self.ty.get_context(), self.int_ty)
    }
}

impl<'ctx> From<SliceType<'ctx>> for PointerType<'ctx> {
    fn from(value: SliceType<'ctx>) -> Self {
        value.as_base_type()
    }
}
