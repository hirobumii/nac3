use std::fmt;

use inkwell::{
    context::Context,
    types::{ArrayType, BasicType, BasicTypeEnum},
    values::{ArrayValue, IntValue},
};

use crate::codegen::{CodeGenContext, CodeGenerator};

use super::*;

/// Trait for Rust structs identifying length values for [`Array`].
pub trait LenKind: fmt::Debug + Clone + Copy {
    fn get_length(&self) -> u32;
}

/// A statically known length.
#[derive(Debug, Clone, Copy, Default)]
pub struct Len<const N: u32>;

/// A dynamically known length.
#[derive(Debug, Clone, Copy)]
pub struct AnyLen(pub u32);

impl<const N: u32> LenKind for Len<N> {
    fn get_length(&self) -> u32 {
        N
    }
}

impl LenKind for AnyLen {
    fn get_length(&self) -> u32 {
        self.0
    }
}

/// A Model for an [`ArrayType`].
///
/// `Len` should be of a [`LenKind`] and `Item` should be a of [`Model`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Array<Len, Item> {
    /// Length of this array.
    pub len: Len,
    /// [`Model`] of the array items.
    pub item: Item,
}

impl<'ctx, Len: LenKind, Item: Model<'ctx>> Model<'ctx> for Array<Len, Item> {
    type Value = ArrayValue<'ctx>;
    type Type = ArrayType<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type {
        self.item.get_type(generator, ctx).array_type(self.len.get_length())
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        let BasicTypeEnum::ArrayType(ty) = ty else {
            return Err(ModelError(format!("Expecting ArrayType, but got {ty:?}")));
        };

        if ty.len() != self.len.get_length() {
            return Err(ModelError(format!(
                "Expecting ArrayType with size {}, but got an ArrayType with size {}",
                ty.len(),
                self.len.get_length()
            )));
        }

        self.item
            .check_type(generator, ctx, ty.get_element_type())
            .map_err(|err| err.under_context("an ArrayType"))?;

        Ok(())
    }
}

impl<'ctx, Len: LenKind, Item: Model<'ctx>> Instance<'ctx, Ptr<Array<Len, Item>>> {
    /// Get the pointer to the `i`-th (0-based) array element.
    pub fn gep(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        i: IntValue<'ctx>,
    ) -> Instance<'ctx, Ptr<Item>> {
        let zero = ctx.ctx.i32_type().const_zero();
        let ptr = unsafe { ctx.builder.build_in_bounds_gep(self.value, &[zero, i], "").unwrap() };

        Ptr(self.model.0.item).believe_value(ptr)
    }

    /// Like `gep` but `i` is a constant.
    pub fn gep_const(&self, ctx: &CodeGenContext<'ctx, '_>, i: u64) -> Instance<'ctx, Ptr<Item>> {
        assert!(
            i < u64::from(self.model.0.len.get_length()),
            "Index {i} is out of bounds. Array length = {}",
            self.model.0.len.get_length()
        );

        let i = ctx.ctx.i32_type().const_int(i, false);
        self.gep(ctx, i)
    }

    /// Convenience function equivalent to `.gep(...).load(...)`.
    pub fn get<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        i: IntValue<'ctx>,
    ) -> Instance<'ctx, Item> {
        self.gep(ctx, i).load(generator, ctx)
    }

    /// Like `get` but `i` is a constant.
    pub fn get_const<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        i: u64,
    ) -> Instance<'ctx, Item> {
        self.gep_const(ctx, i).load(generator, ctx)
    }

    /// Convenience function equivalent to `.gep(...).store(...)`.
    pub fn set(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        i: IntValue<'ctx>,
        value: Instance<'ctx, Item>,
    ) {
        self.gep(ctx, i).store(ctx, value);
    }

    /// Like `set` but `i` is a constant.
    pub fn set_const(&self, ctx: &CodeGenContext<'ctx, '_>, i: u64, value: Instance<'ctx, Item>) {
        self.gep_const(ctx, i).store(ctx, value);
    }
}
