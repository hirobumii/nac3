use inkwell::{
    types::{BasicType, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace,
};

use super::{NDArrayValue, ProxyValue};
use crate::codegen::{
    irrt,
    stmt::{gen_for_callback, BreakContinueHooks},
    types::{ndarray::NDIterType, structure::StructField},
    values::{ArraySliceValue, TypedArrayLikeAdapter},
    CodeGenContext, CodeGenerator,
};

#[derive(Copy, Clone)]
pub struct NDIterValue<'ctx> {
    value: PointerValue<'ctx>,
    parent: NDArrayValue<'ctx>,
    indices: ArraySliceValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> NDIterValue<'ctx> {
    /// Checks whether `value` is an instance of `NDArray`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_representable(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        <Self as ProxyValue>::Type::is_representable(value.get_type(), llvm_usize)
    }

    /// Creates an [`NDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr, llvm_usize).is_ok());

        Self { value: ptr, parent, indices, llvm_usize, name }
    }

    /// Is the current iteration valid?
    ///
    /// If true, then `element`, `indices` and `nth` contain details about the current element.
    ///
    /// If `ndarray` is unsized, this returns true only for the first iteration.
    /// If `ndarray` is 0-sized, this always returns false.
    #[must_use]
    pub fn has_element(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        irrt::ndarray::call_nac3_nditer_has_element(ctx, *self)
    }

    /// Go to the next element. If `has_element()` is false, then this has undefined behavior.
    ///
    /// If `ndarray` is unsized, this can only be called once.
    /// If `ndarray` is 0-sized, this can never be called.
    pub fn next(&self, ctx: &CodeGenContext<'ctx, '_>) {
        irrt::ndarray::call_nac3_nditer_next(ctx, *self);
    }

    fn element_field(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).element
    }

    /// Get pointer to the current element.
    #[must_use]
    pub fn get_pointer(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let elem_ty = self.parent.dtype;

        let p = self.element_field(ctx).get(ctx, self.as_base_value(), self.name);
        ctx.builder
            .build_pointer_cast(p, elem_ty.ptr_type(AddressSpace::default()), "element")
            .unwrap()
    }

    /// Get the value of the current element.
    #[must_use]
    pub fn get_scalar(&self, ctx: &CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        let p = self.get_pointer(ctx);
        ctx.builder.build_load(p, "value").unwrap()
    }

    fn nth_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields(ctx.ctx).nth
    }

    /// Get the index of the current element if this ndarray were a flat ndarray.
    #[must_use]
    pub fn get_index(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.nth_field(ctx).get(ctx, self.as_base_value(), self.name)
    }

    /// Get the indices of the current element.
    #[must_use]
    pub fn get_indices<G: CodeGenerator + ?Sized>(
        &self,
    ) -> TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>> {
        TypedArrayLikeAdapter::from(
            self.indices,
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        )
    }
}

impl<'ctx> ProxyValue<'ctx> for NDIterValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = NDIterType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDIterType::from_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<NDIterValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDIterValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Iterate through every element in the ndarray.
    ///
    /// `body` has access to [`BreakContinueHooks`] to short-circuit and [`NDIterValue`] to
    /// get properties of the current iteration (e.g., the current element, indices, etc.)
    ///
    /// Note: The caller is recommended to call `llvm.stacksave` and `llvm.stackrestore` before and
    /// after invoking this function respectively. See [`NDIterType::construct`] for an explanation
    /// on why this is suggested.
    pub fn foreach<'a, G, F>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        body: F,
    ) -> Result<(), String>
    where
        G: CodeGenerator + ?Sized,
        F: FnOnce(
            &mut G,
            &mut CodeGenContext<'ctx, 'a>,
            BreakContinueHooks<'ctx>,
            NDIterValue<'ctx>,
        ) -> Result<(), String>,
    {
        gen_for_callback(
            generator,
            ctx,
            Some("ndarray_foreach"),
            |generator, ctx| {
                Ok(NDIterType::new(generator, ctx.ctx).construct(generator, ctx, *self))
            },
            |_, ctx, nditer| Ok(nditer.has_element(ctx)),
            |generator, ctx, hooks, nditer| body(generator, ctx, hooks, nditer),
            |_, ctx, nditer| {
                nditer.next(ctx);
                Ok(())
            },
        )
    }
}
