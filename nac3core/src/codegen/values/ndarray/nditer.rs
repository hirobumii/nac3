use inkwell::{
    types::{BasicType, IntType},
    values::{BasicValueEnum, IntValue, PointerValue, StructValue},
    AddressSpace,
};

use super::NDArrayValue;
use crate::codegen::{
    irrt,
    stmt::{gen_for_callback, BreakContinueHooks},
    types::{
        ndarray::NDIterType,
        structure::{StructField, StructProxyType},
    },
    values::{structure::StructProxyValue, ArraySliceValue, ProxyValue, TypedArrayLikeAdapter},
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
    /// Creates an [`NDArrayValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        parent: NDArrayValue<'ctx>,
        indices: ArraySliceValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval = generator
            .gen_var_alloc(
                ctx,
                val.get_type().into(),
                name.map(|name| format!("{name}.addr")).as_deref(),
            )
            .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, parent, indices, llvm_usize, name)
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
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

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

    fn element_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().element
    }

    /// Get pointer to the current element.
    #[must_use]
    pub fn get_pointer(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let elem_ty = self.parent.dtype;

        let p = self.element_field().load(ctx, self.as_abi_value(ctx), self.name);
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

    fn nth_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().nth
    }

    /// Get the index of the current element if this ndarray were a flat ndarray.
    #[must_use]
    pub fn get_index(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.nth_field().load(ctx, self.as_abi_value(ctx), self.name)
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
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = NDIterType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDIterType::from_pointer_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for NDIterValue<'ctx> {}

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
            |generator, ctx| Ok(NDIterType::new(ctx).construct(generator, ctx, *self)),
            |_, ctx, nditer| Ok(nditer.has_element(ctx)),
            |generator, ctx, hooks, nditer| body(generator, ctx, hooks, nditer),
            |_, ctx, nditer| {
                nditer.next(ctx);
                Ok(())
            },
        )
    }
}
