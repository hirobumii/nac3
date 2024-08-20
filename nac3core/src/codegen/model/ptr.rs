use inkwell::{
    context::Context,
    types::{BasicType, BasicTypeEnum, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace,
};

use crate::codegen::{llvm_intrinsics::call_memcpy_generic, CodeGenContext, CodeGenerator};

use super::*;

/// A model for [`PointerType`].
///
/// `Item` is the element type this pointer is pointing to, and should be of a [`Model`].
///
// TODO: LLVM 15: `Item` is a Rust type-hint for the LLVM type of value the `.store()/.load()` family
// of functions return. If a truly opaque pointer is needed, tell the programmer to use `OpaquePtr`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ptr<Item>(pub Item);

/// An opaque pointer. Like [`Ptr`] but without any Rust type-hints about its element type.
///
/// `.load()/.store()` is not available for [`Instance`]s of opaque pointers.
pub type OpaquePtr = Ptr<()>;

// TODO: LLVM 15: `Item: Model<'ctx>` don't even need to be a model anymore. It will only be
// a type-hint for the `.load()/.store()` functions for the `pointee_ty`.
//
// See https://thedan64.github.io/inkwell/inkwell/builder/struct.Builder.html#method.build_load.
impl<'ctx, Item: Model<'ctx>> Model<'ctx> for Ptr<Item> {
    type Value = PointerValue<'ctx>;
    type Type = PointerType<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type {
        // TODO: LLVM 15: ctx.ptr_type(AddressSpace::default())
        self.0.get_type(generator, ctx).ptr_type(AddressSpace::default())
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        let Ok(ty) = PointerType::try_from(ty) else {
            return Err(ModelError(format!("Expecting PointerType, but got {ty:?}")));
        };

        let elem_ty = ty.get_element_type();
        let Ok(elem_ty) = BasicTypeEnum::try_from(elem_ty) else {
            return Err(ModelError(format!(
                "Expecting pointer element type to be a BasicTypeEnum, but got {elem_ty:?}"
            )));
        };

        // TODO: inkwell `get_element_type()` will be deprecated.
        // Remove the check for `get_element_type()` when the time comes.
        self.0
            .check_type(generator, ctx, elem_ty)
            .map_err(|err| err.under_context("a PointerType"))?;

        Ok(())
    }
}

impl<'ctx, Item: Model<'ctx>> Ptr<Item> {
    /// Return a ***constant*** nullptr.
    pub fn nullptr<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Ptr<Item>> {
        let ptr = self.get_type(generator, ctx).const_null();
        self.believe_value(ptr)
    }

    /// Cast a pointer into this model with [`inkwell::builder::Builder::build_pointer_cast`]
    pub fn pointer_cast<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        ptr: PointerValue<'ctx>,
    ) -> Instance<'ctx, Ptr<Item>> {
        // TODO: LLVM 15: Write in an impl where `Item` does not have to be `Model<'ctx>`.
        // TODO: LLVM 15: This function will only have to be:
        // ```
        // return self.believe_value(ptr);
        // ```
        let t = self.get_type(generator, ctx.ctx);
        let ptr = ctx.builder.build_pointer_cast(ptr, t, "").unwrap();
        self.believe_value(ptr)
    }
}

impl<'ctx, Item: Model<'ctx>> Instance<'ctx, Ptr<Item>> {
    /// Offset the pointer by [`inkwell::builder::Builder::build_in_bounds_gep`].
    #[must_use]
    pub fn offset(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        offset: IntValue<'ctx>,
    ) -> Instance<'ctx, Ptr<Item>> {
        let p = unsafe { ctx.builder.build_in_bounds_gep(self.value, &[offset], "").unwrap() };
        self.model.believe_value(p)
    }

    /// Offset the pointer by [`inkwell::builder::Builder::build_in_bounds_gep`] by a constant offset.
    #[must_use]
    pub fn offset_const(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        offset: u64,
    ) -> Instance<'ctx, Ptr<Item>> {
        let offset = ctx.ctx.i32_type().const_int(offset, false);
        self.offset(ctx, offset)
    }

    pub fn set_index(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        index: IntValue<'ctx>,
        value: Instance<'ctx, Item>,
    ) {
        self.offset(ctx, index).store(ctx, value);
    }

    pub fn set_index_const(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        index: u64,
        value: Instance<'ctx, Item>,
    ) {
        self.offset_const(ctx, index).store(ctx, value);
    }

    pub fn get_index<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        index: IntValue<'ctx>,
    ) -> Instance<'ctx, Item> {
        self.offset(ctx, index).load(generator, ctx)
    }

    pub fn get_index_const<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        index: u64,
    ) -> Instance<'ctx, Item> {
        self.offset_const(ctx, index).load(generator, ctx)
    }

    /// Load the value with [`inkwell::builder::Builder::build_load`].
    pub fn load<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Item> {
        let value = ctx.builder.build_load(self.value, "").unwrap();
        self.model.0.check_value(generator, ctx.ctx, value).unwrap() // If unwrap() panics, there is a logic error.
    }

    /// Store a value with [`inkwell::builder::Builder::build_store`].
    pub fn store(&self, ctx: &CodeGenContext<'ctx, '_>, value: Instance<'ctx, Item>) {
        ctx.builder.build_store(self.value, value.value).unwrap();
    }

    /// Return a casted pointer of element type `NewElement` with [`inkwell::builder::Builder::build_pointer_cast`].
    pub fn pointer_cast<NewItem: Model<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        new_item: NewItem,
    ) -> Instance<'ctx, Ptr<NewItem>> {
        // TODO: LLVM 15: Write in an impl where `Item` does not have to be `Model<'ctx>`.
        Ptr(new_item).pointer_cast(generator, ctx, self.value)
    }

    /// Cast this pointer to `uint8_t*`
    pub fn cast_to_pi8<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Ptr<Int<Byte>>> {
        Ptr(Int(Byte)).pointer_cast(generator, ctx, self.value)
    }

    /// Check if the pointer is null with [`inkwell::builder::Builder::build_is_null`].
    pub fn is_null(&self, ctx: &CodeGenContext<'ctx, '_>) -> Instance<'ctx, Int<Bool>> {
        let value = ctx.builder.build_is_null(self.value, "").unwrap();
        Int(Bool).believe_value(value)
    }

    /// Check if the pointer is not null with [`inkwell::builder::Builder::build_is_not_null`].
    pub fn is_not_null(&self, ctx: &CodeGenContext<'ctx, '_>) -> Instance<'ctx, Int<Bool>> {
        let value = ctx.builder.build_is_not_null(self.value, "").unwrap();
        Int(Bool).believe_value(value)
    }

    /// `memcpy` from another pointer.
    pub fn copy_from<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        source: Self,
        num_items: IntValue<'ctx>,
    ) {
        // Force extend `num_items` and `itemsize` to `i64` so their types would match.
        let itemsize = self.model.sizeof(generator, ctx.ctx);
        let itemsize = Int(Int64).z_extend_or_truncate(generator, ctx, itemsize);
        let num_items = Int(Int64).z_extend_or_truncate(generator, ctx, num_items);
        let totalsize = itemsize.mul(ctx, num_items);

        let is_volatile = ctx.ctx.bool_type().const_zero(); // is_volatile = false
        call_memcpy_generic(ctx, self.value, source.value, totalsize.value, is_volatile);
    }
}
