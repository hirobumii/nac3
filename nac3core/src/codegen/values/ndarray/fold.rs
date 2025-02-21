use inkwell::values::{BasicValue, BasicValueEnum};

use super::{NDArrayValue, NDIterValue, ScalarOrNDArray};
use crate::codegen::{
    CodeGenContext, CodeGenerator,
    stmt::{LoopHooks, gen_for_callback},
    types::ndarray::NDIterType,
};

impl<'ctx> NDArrayValue<'ctx> {
    /// Folds the elements of this ndarray into an accumulator value by applying `f`, returning the
    /// final value.
    ///
    /// `f` has access to [`LoopHooks`] to short-circuit the `fold` operation, an instance
    /// of `V` representing the current accumulated value, and an [`NDIterValue`] to get the
    /// properties of the current iterated element.
    pub fn fold<'a, G, V, F>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        init: V,
        f: F,
    ) -> Result<V, String>
    where
        G: CodeGenerator + ?Sized,
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>>,
        <V as TryFrom<BasicValueEnum<'ctx>>>::Error: std::fmt::Debug,
        F: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            LoopHooks<'ctx>,
            V,
            NDIterValue<'ctx>,
        ) -> Result<V, String>,
    {
        let acc_ptr =
            generator.gen_var_alloc(ctx, init.as_basic_value_enum().get_type(), None).unwrap();
        ctx.builder.build_store(acc_ptr, init).unwrap();

        gen_for_callback(
            generator,
            ctx,
            Some("ndarray_fold"),
            |generator, ctx| Ok(NDIterType::new(ctx).construct(generator, ctx, *self)),
            |_, ctx, nditer| Ok(nditer.has_element(ctx)),
            |generator, ctx, hooks, nditer| {
                let acc = V::try_from(ctx.builder.build_load(acc_ptr, "").unwrap()).unwrap();
                let acc = f(generator, ctx, hooks, acc, nditer)?;
                ctx.builder.build_store(acc_ptr, acc).unwrap();
                Ok(())
            },
            |_, ctx, nditer| {
                nditer.next(ctx);
                Ok(())
            },
        )?;

        let acc = ctx.builder.build_load(acc_ptr, "").unwrap();
        Ok(V::try_from(acc).unwrap())
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// See [`NDArrayValue::fold`].
    ///
    /// The primary differences between this function and `NDArrayValue::fold` are:
    ///
    /// - The 3rd parameter of `f` is an `Option` of hooks, since `break`/`continue` hooks are not
    ///   available if this instance represents a scalar value.
    /// - The 5th parameter of `f` is a [`BasicValueEnum`], since no [iterator][`NDIterValue`] will
    ///   be created if this instance represents a scalar value.
    pub fn fold<'a, G, V, F>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        init: V,
        f: F,
    ) -> Result<V, String>
    where
        G: CodeGenerator + ?Sized,
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>>,
        <V as TryFrom<BasicValueEnum<'ctx>>>::Error: std::fmt::Debug,
        F: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            Option<&LoopHooks<'ctx>>,
            V,
            BasicValueEnum<'ctx>,
        ) -> Result<V, String>,
    {
        match self {
            ScalarOrNDArray::Scalar(v) => f(generator, ctx, None, init, *v),
            ScalarOrNDArray::NDArray(v) => {
                v.fold(generator, ctx, init, |generator, ctx, hooks, acc, nditer| {
                    let elem = nditer.get_scalar(ctx);
                    f(generator, ctx, Some(&hooks), acc, elem)
                })
            }
        }
    }
}
