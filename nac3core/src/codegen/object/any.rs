use inkwell::values::BasicValueEnum;

use crate::typecheck::typedef::Type;

/// A NAC3 LLVM Python object of any type.
#[derive(Debug, Clone, Copy)]
pub struct AnyObject<'ctx> {
    /// Typechecker type of the object.
    pub ty: Type,
    /// LLVM value of the object.
    pub value: BasicValueEnum<'ctx>,
}
