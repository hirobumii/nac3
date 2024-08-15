use inkwell::{
    context::Context,
    types::{BasicType, BasicTypeEnum},
    values::BasicValueEnum,
};

use crate::codegen::CodeGenerator;

use super::*;

/// A [`Model`] of any [`BasicTypeEnum`].
///
/// Use this when it is infeasible to use model abstractions.
#[derive(Debug, Clone, Copy)]
pub struct Any<'ctx>(pub BasicTypeEnum<'ctx>);

impl<'ctx> Model<'ctx> for Any<'ctx> {
    type Value = BasicValueEnum<'ctx>;
    type Type = BasicTypeEnum<'ctx>;

    fn get_type<G: CodeGenerator + ?Sized>(
        &self,
        _generator: &G,
        _ctx: &'ctx Context,
    ) -> Self::Type {
        self.0
    }

    fn check_type<T: BasicType<'ctx>, G: CodeGenerator + ?Sized>(
        &self,
        _generator: &mut G,
        _ctx: &'ctx Context,
        ty: T,
    ) -> Result<(), ModelError> {
        let ty = ty.as_basic_type_enum();
        if ty == self.0 {
            Ok(())
        } else {
            Err(ModelError(format!("Expecting {}, but got {}", self.0, ty)))
        }
    }
}
