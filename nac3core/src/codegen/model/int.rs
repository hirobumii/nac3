use std::{cmp::Ordering, fmt};

use inkwell::{
    context::Context,
    types::{BasicType, IntType},
    values::IntValue,
    IntPredicate,
};

use crate::codegen::{CodeGenContext, CodeGenerator};

use super::*;

pub trait IntKind<'ctx>: fmt::Debug + Clone + Copy {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Bool;
#[derive(Debug, Clone, Copy, Default)]
pub struct Byte;
#[derive(Debug, Clone, Copy, Default)]
pub struct Int32;
#[derive(Debug, Clone, Copy, Default)]
pub struct Int64;
#[derive(Debug, Clone, Copy, Default)]
pub struct SizeT;

impl<'ctx> IntKind<'ctx> for Bool {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        ctx.bool_type()
    }
}

impl<'ctx> IntKind<'ctx> for Byte {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        ctx.i8_type()
    }
}

impl<'ctx> IntKind<'ctx> for Int32 {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        ctx.i32_type()
    }
}

impl<'ctx> IntKind<'ctx> for Int64 {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        ctx.i64_type()
    }
}

impl<'ctx> IntKind<'ctx> for SizeT {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        generator.get_size_type(ctx)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AnyInt<'ctx>(pub IntType<'ctx>);

impl<'ctx> IntKind<'ctx> for AnyInt<'ctx> {
    fn get_int_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        _ctx: &'ctx Context,
    ) -> IntType<'ctx> {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Int<N>(pub N);

impl<'ctx, N: IntKind<'ctx>> Model<'ctx> for Int<N> {
    type Value = IntValue<'ctx>;
    type Type = IntType<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type {
        self.0.get_int_type(generator, ctx)
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        let Ok(ty) = IntType::try_from(ty) else {
            return Err(ModelError(format!("Expecting IntType, but got {ty:?}")));
        };

        let exp_ty = self.0.get_int_type(generator, ctx);
        if ty.get_bit_width() != exp_ty.get_bit_width() {
            return Err(ModelError(format!(
                "Expecting IntType to have {} bit(s), but got {} bit(s)",
                exp_ty.get_bit_width(),
                ty.get_bit_width()
            )));
        }

        Ok(())
    }
}

impl<'ctx, N: IntKind<'ctx>> Int<N> {
    pub fn const_int<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        value: u64,
    ) -> Instance<'ctx, Self> {
        let value = self.get_type(generator, ctx).const_int(value, false);
        self.believe_value(value)
    }

    pub fn const_0<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Self> {
        let value = self.get_type(generator, ctx).const_zero();
        self.believe_value(value)
    }

    pub fn const_1<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Self> {
        self.const_int(generator, ctx, 1)
    }

    pub fn const_all_ones<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Self> {
        let value = self.get_type(generator, ctx).const_all_ones();
        self.believe_value(value)
    }

    pub fn s_extend_or_bit_cast<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                <= self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value = ctx
            .builder
            .build_int_s_extend_or_bit_cast(value, self.get_type(generator, ctx.ctx), "")
            .unwrap();
        self.believe_value(value)
    }

    pub fn s_extend<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                < self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value =
            ctx.builder.build_int_s_extend(value, self.get_type(generator, ctx.ctx), "").unwrap();
        self.believe_value(value)
    }

    pub fn z_extend_or_bit_cast<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                <= self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value = ctx
            .builder
            .build_int_z_extend_or_bit_cast(value, self.get_type(generator, ctx.ctx), "")
            .unwrap();
        self.believe_value(value)
    }

    pub fn z_extend<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                < self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value =
            ctx.builder.build_int_z_extend(value, self.get_type(generator, ctx.ctx), "").unwrap();
        self.believe_value(value)
    }

    pub fn truncate_or_bit_cast<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                >= self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value = ctx
            .builder
            .build_int_truncate_or_bit_cast(value, self.get_type(generator, ctx.ctx), "")
            .unwrap();
        self.believe_value(value)
    }

    pub fn truncate<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        assert!(
            value.get_type().get_bit_width()
                > self.0.get_int_type(generator, ctx.ctx).get_bit_width()
        );
        let value =
            ctx.builder.build_int_truncate(value, self.get_type(generator, ctx.ctx), "").unwrap();
        self.believe_value(value)
    }

    /// `sext` or `trunc` an int to this model's int type. Does nothing if equal bit-widths.
    pub fn s_extend_or_truncate<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        let their_width = value.get_type().get_bit_width();
        let our_width = self.0.get_int_type(generator, ctx.ctx).get_bit_width();
        match their_width.cmp(&our_width) {
            Ordering::Less => self.s_extend(generator, ctx, value),
            Ordering::Equal => self.believe_value(value),
            Ordering::Greater => self.truncate(generator, ctx, value),
        }
    }

    /// `zext` or `trunc` an int to this model's int type. Does nothing if equal bit-widths.
    pub fn z_extend_or_truncate<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> Instance<'ctx, Self> {
        let their_width = value.get_type().get_bit_width();
        let our_width = self.0.get_int_type(generator, ctx.ctx).get_bit_width();
        match their_width.cmp(&our_width) {
            Ordering::Less => self.z_extend(generator, ctx, value),
            Ordering::Equal => self.believe_value(value),
            Ordering::Greater => self.truncate(generator, ctx, value),
        }
    }
}

impl Int<Bool> {
    #[must_use]
    pub fn const_false<'ctx, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Self> {
        self.const_int(generator, ctx, 0)
    }

    #[must_use]
    pub fn const_true<'ctx, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Self> {
        self.const_int(generator, ctx, 1)
    }
}

impl<'ctx, N: IntKind<'ctx>> Instance<'ctx, Int<N>> {
    pub fn s_extend_or_bit_cast<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).s_extend_or_bit_cast(generator, ctx, self.value)
    }

    pub fn s_extend<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).s_extend(generator, ctx, self.value)
    }

    pub fn z_extend_or_bit_cast<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).z_extend_or_bit_cast(generator, ctx, self.value)
    }

    pub fn z_extend<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).z_extend(generator, ctx, self.value)
    }

    pub fn truncate_or_bit_cast<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).truncate_or_bit_cast(generator, ctx, self.value)
    }

    pub fn truncate<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).truncate(generator, ctx, self.value)
    }

    pub fn s_extend_or_truncate<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).s_extend_or_truncate(generator, ctx, self.value)
    }

    pub fn z_extend_or_truncate<NewN: IntKind<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        to_int_kind: NewN,
    ) -> Instance<'ctx, Int<NewN>> {
        Int(to_int_kind).z_extend_or_truncate(generator, ctx, self.value)
    }

    #[must_use]
    pub fn add(&self, ctx: &CodeGenContext<'ctx, '_>, other: Self) -> Self {
        let value = ctx.builder.build_int_add(self.value, other.value, "").unwrap();
        self.model.believe_value(value)
    }

    #[must_use]
    pub fn sub(&self, ctx: &CodeGenContext<'ctx, '_>, other: Self) -> Self {
        let value = ctx.builder.build_int_sub(self.value, other.value, "").unwrap();
        self.model.believe_value(value)
    }

    #[must_use]
    pub fn mul(&self, ctx: &CodeGenContext<'ctx, '_>, other: Self) -> Self {
        let value = ctx.builder.build_int_mul(self.value, other.value, "").unwrap();
        self.model.believe_value(value)
    }

    pub fn compare(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        op: IntPredicate,
        other: Self,
    ) -> Instance<'ctx, Int<Bool>> {
        let value = ctx.builder.build_int_compare(op, self.value, other.value, "").unwrap();
        Int(Bool).believe_value(value)
    }
}
