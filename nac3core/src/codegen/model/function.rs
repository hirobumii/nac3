use inkwell::{
    attributes::{Attribute, AttributeLoc},
    types::{BasicMetadataTypeEnum, BasicType, FunctionType},
    values::{AnyValue, BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue},
};
use itertools::Itertools;

use crate::codegen::{CodeGenContext, CodeGenerator};

use super::*;

#[derive(Debug, Clone, Copy)]
struct Arg<'ctx> {
    ty: BasicMetadataTypeEnum<'ctx>,
    val: BasicMetadataValueEnum<'ctx>,
}

/// A convenience structure to construct & call an LLVM function.
///
/// ### Usage
///
/// The syntax is like this:
/// ```ignore
/// let result = CallFunction::begin("my_function_name")
///   .attrs(...)
///   .arg(arg1)
///   .arg(arg2)
///   .arg(arg3)
///   .returning("my_function_result", Int32);
/// ```
///
/// The function `my_function_name` is called when `.returning()` (or its variants) is called, returning
/// the result as an `Instance<'ctx, Int<Int32>>`.
///
/// If `my_function_name` has not been declared in `ctx.module`, once `.returning()` is called, a function
/// declaration of `my_function_name` is added to `ctx.module`, where the [`FunctionType`] is deduced from
/// the argument types and returning type.
pub struct CallFunction<'ctx, 'a, 'b, 'c, 'd, G: CodeGenerator + ?Sized> {
    generator: &'d mut G,
    ctx: &'b CodeGenContext<'ctx, 'a>,
    /// Function name
    name: &'c str,
    /// Call arguments
    args: Vec<Arg<'ctx>>,
    /// LLVM function Attributes
    attrs: Vec<&'static str>,
}

impl<'ctx, 'a, 'b, 'c, 'd, G: CodeGenerator + ?Sized> CallFunction<'ctx, 'a, 'b, 'c, 'd, G> {
    pub fn begin(generator: &'d mut G, ctx: &'b CodeGenContext<'ctx, 'a>, name: &'c str) -> Self {
        CallFunction { generator, ctx, name, args: Vec::new(), attrs: Vec::new() }
    }

    /// Push a list of LLVM function attributes to the function declaration.
    #[must_use]
    pub fn attrs(mut self, attrs: Vec<&'static str>) -> Self {
        self.attrs = attrs;
        self
    }

    /// Push a call argument to the function call.
    #[allow(clippy::needless_pass_by_value)]
    #[must_use]
    pub fn arg<M: Model<'ctx>>(mut self, arg: Instance<'ctx, M>) -> Self {
        let arg = Arg {
            ty: arg.model.get_type(self.generator, self.ctx.ctx).as_basic_type_enum().into(),
            val: arg.value.as_basic_value_enum().into(),
        };
        self.args.push(arg);
        self
    }

    /// Call the function and expect the function to return a value of type of `return_model`.
    #[must_use]
    pub fn returning<M: Model<'ctx>>(self, name: &str, return_model: M) -> Instance<'ctx, M> {
        let ret_ty = return_model.get_type(self.generator, self.ctx.ctx);

        let ret = self.call(|tys| ret_ty.fn_type(tys, false), name);
        let ret = BasicValueEnum::try_from(ret.as_any_value_enum()).unwrap(); // Must work
        let ret = return_model.check_value(self.generator, self.ctx.ctx, ret).unwrap(); // Must work
        ret
    }

    /// Like [`CallFunction::returning_`] but `return_model` is automatically inferred.
    #[must_use]
    pub fn returning_auto<M: Model<'ctx> + Default>(self, name: &str) -> Instance<'ctx, M> {
        self.returning(name, M::default())
    }

    /// Call the function and expect the function to return a void-type.
    pub fn returning_void(self) {
        let ret_ty = self.ctx.ctx.void_type();

        let _ = self.call(|tys| ret_ty.fn_type(tys, false), "");
    }

    fn call<F>(&self, make_fn_type: F, return_value_name: &str) -> CallSiteValue<'ctx>
    where
        F: FnOnce(&[BasicMetadataTypeEnum<'ctx>]) -> FunctionType<'ctx>,
    {
        // Get the LLVM function.
        let func = self.ctx.module.get_function(self.name).unwrap_or_else(|| {
            // Declare the function if it doesn't exist.
            let tys = self.args.iter().map(|arg| arg.ty).collect_vec();

            let func_type = make_fn_type(&tys);
            let func = self.ctx.module.add_function(self.name, func_type, None);

            for attr in &self.attrs {
                func.add_attribute(
                    AttributeLoc::Function,
                    self.ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
                );
            }

            func
        });

        let vals = self.args.iter().map(|arg| arg.val).collect_vec();
        self.ctx.builder.build_call(func, &vals, return_value_name).unwrap()
    }
}
