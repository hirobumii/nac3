use inkwell::{types::BasicType, values::PointerValue, AddressSpace};

use super::NDArrayObject;
use crate::codegen::{
    irrt::{call_nac3_nditer_has_element, call_nac3_nditer_initialize, call_nac3_nditer_next},
    model::*,
    object::any::AnyObject,
    stmt::{gen_for_callback, BreakContinueHooks},
    CodeGenContext, CodeGenerator,
};

/// Fields of [`NDIter`]
pub struct NDIterFields<'ctx, F: FieldTraversal<'ctx>> {
    pub ndims: F::Output<Int<SizeT>>,
    pub shape: F::Output<Ptr<Int<SizeT>>>,
    pub strides: F::Output<Ptr<Int<SizeT>>>,

    pub indices: F::Output<Ptr<Int<SizeT>>>,
    pub nth: F::Output<Int<SizeT>>,
    pub element: F::Output<Ptr<Int<Byte>>>,

    pub size: F::Output<Int<SizeT>>,
}

/// An IRRT helper structure used to iterate through an ndarray.
#[derive(Debug, Clone, Copy, Default)]
pub struct NDIter;

impl<'ctx> StructKind<'ctx> for NDIter {
    type Fields<F: FieldTraversal<'ctx>> = NDIterFields<'ctx, F>;

    fn iter_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields {
            ndims: traversal.add_auto("ndims"),
            shape: traversal.add_auto("shape"),
            strides: traversal.add_auto("strides"),

            indices: traversal.add_auto("indices"),
            nth: traversal.add_auto("nth"),
            element: traversal.add_auto("element"),

            size: traversal.add_auto("size"),
        }
    }
}

/// A helper structure with a convenient interface to interact with [`NDIter`].
#[derive(Debug, Clone)]
pub struct NDIterHandle<'ctx> {
    instance: Instance<'ctx, Ptr<Struct<NDIter>>>,
    /// The ndarray this [`NDIter`] to iterating over.
    ndarray: NDArrayObject<'ctx>,
    /// The current indices of [`NDIter`].
    indices: Instance<'ctx, Ptr<Int<SizeT>>>,
}

impl<'ctx> NDIterHandle<'ctx> {
    /// Allocate an [`NDIter`] that iterates through an ndarray.
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayObject<'ctx>,
    ) -> Self {
        let nditer = Struct(NDIter).alloca(generator, ctx);
        let ndims = ndarray.ndims_llvm(generator, ctx.ctx);

        // The caller has the responsibility to allocate 'indices' for `NDIter`.
        let indices = Int(SizeT).array_alloca(generator, ctx, ndims.value);
        call_nac3_nditer_initialize(generator, ctx, nditer, ndarray.instance, indices);

        NDIterHandle { ndarray, instance: nditer, indices }
    }

    /// Is the current iteration valid?
    ///
    /// If true, then `element`, `indices` and `nth` contain details about the current element.
    ///
    /// If `ndarray` is unsized, this returns true only for the first iteration.
    /// If `ndarray` is 0-sized, this always returns false.
    #[must_use]
    pub fn has_element<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<Bool>> {
        call_nac3_nditer_has_element(generator, ctx, self.instance)
    }

    /// Go to the next element. If `has_element()` is false, then this has undefined behavior.
    ///
    /// If `ndarray` is unsized, this can only be called once.
    /// If `ndarray` is 0-sized, this can never be called.
    pub fn next<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) {
        call_nac3_nditer_next(generator, ctx, self.instance);
    }

    /// Get pointer to the current element.
    #[must_use]
    pub fn get_pointer<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> PointerValue<'ctx> {
        let elem_ty = ctx.get_llvm_type(generator, self.ndarray.dtype);

        let p = self.instance.get(generator, ctx, |f| f.element);
        ctx.builder
            .build_pointer_cast(p.value, elem_ty.ptr_type(AddressSpace::default()), "element")
            .unwrap()
    }

    /// Get the value of the current element.
    #[must_use]
    pub fn get_scalar<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> AnyObject<'ctx> {
        let p = self.get_pointer(generator, ctx);
        let value = ctx.builder.build_load(p, "value").unwrap();
        AnyObject { ty: self.ndarray.dtype, value }
    }

    /// Get the index of the current element if this ndarray were a flat ndarray.
    #[must_use]
    pub fn get_index<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        self.instance.get(generator, ctx, |f| f.nth)
    }

    /// Get the indices of the current element.
    #[must_use]
    pub fn get_indices(&self) -> Instance<'ctx, Ptr<Int<SizeT>>> {
        self.indices
    }
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Iterate through every element in the ndarray.
    ///
    /// `body` has access to [`BreakContinueHooks`] to short-circuit and [`NDIterHandle`] to
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
            NDIterHandle<'ctx>,
        ) -> Result<(), String>,
    {
        gen_for_callback(
            generator,
            ctx,
            Some("ndarray_foreach"),
            |generator, ctx| Ok(NDIterHandle::new(generator, ctx, *self)),
            |generator, ctx, nditer| Ok(nditer.has_element(generator, ctx).value),
            |generator, ctx, hooks, nditer| body(generator, ctx, hooks, nditer),
            |generator, ctx, nditer| {
                nditer.next(generator, ctx);
                Ok(())
            },
        )
    }
}
