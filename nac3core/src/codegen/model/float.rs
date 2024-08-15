use std::fmt;

use inkwell::{
    context::Context,
    types::{BasicType, FloatType},
    values::FloatValue,
};

use crate::codegen::CodeGenerator;

use super::*;

pub trait FloatKind<'ctx>: fmt::Debug + Clone + Copy {
    fn get_float_type<G: CodeGenerator + ?Sized>(
        &self,
        generator: &G,
        ctx: &'ctx Context,
    ) -> FloatType<'ctx>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Float32;
#[derive(Debug, Clone, Copy, Default)]
pub struct Float64;

impl<'ctx> FloatKind<'ctx> for Float32 {
    fn get_float_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> FloatType<'ctx> {
        ctx.f32_type()
    }
}

impl<'ctx> FloatKind<'ctx> for Float64 {
    fn get_float_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        ctx: &'ctx Context,
    ) -> FloatType<'ctx> {
        ctx.f64_type()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AnyFloat<'ctx>(FloatType<'ctx>);

impl<'ctx> FloatKind<'ctx> for AnyFloat<'ctx> {
    fn get_float_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        _ctx: &'ctx Context,
    ) -> FloatType<'ctx> {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Float<N>(pub N);

impl<'ctx, N: FloatKind<'ctx>> Model<'ctx> for Float<N> {
    type Value = FloatValue<'ctx>;
    type Type = FloatType<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(&self, generator: &G, ctx: &'ctx Context) -> Self::Type {
        self.0.get_float_type(generator, ctx)
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        let Ok(ty) = FloatType::try_from(ty) else {
            return Err(ModelError(format!("Expecting FloatType, but got {ty:?}")));
        };

        let exp_ty = self.0.get_float_type(generator, ctx);

        // TODO: Inkwell does not have get_bit_width for FloatType?
        if ty != exp_ty {
            return Err(ModelError(format!("Expecting {exp_ty:?}, but got {ty:?}")));
        }

        Ok(())
    }
}
