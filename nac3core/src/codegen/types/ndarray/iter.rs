use anyhow::anyhow;
use inkwell::{
    types::BasicTypeEnum,
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    stmt::{BreakContinueHooks, gen_array_var, gen_for_callback, gen_var},
    typed_load, typed_store,
    types::{
        ProxyTypeBase, Value,
        array::ArraySliceValue,
        builtin::BuiltinStruct,
        field,
        ndarray::{NDArrayValue, ScalarOrNDArray},
        structure::StructField,
    },
};

#[derive(Clone, Copy, StructFields)]
pub struct NDIterStructFields<'ctx> {
    #[value_type(size_t)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    pub strides: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    pub indices: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(size_t)]
    pub nth: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    pub element: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(size_t)]
    pub size: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct NDIterType<'ctx> {
    pub(crate) inner: BuiltinStruct<'ctx, NDIterStructFields<'ctx>>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: u64,
}

impl<'ctx> NDIterType<'ctx> {
    fn ndims_val(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        ctx.size_t.const_int(self.ndims, false)
    }
}

pub type NDIterValue<'ctx> = Value<'ctx, NDIterType<'ctx>>;

impl<'ctx> NDIterValue<'ctx> {
    /// Creates an iterator that iterates through `ndarray`.
    pub fn new(
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayValue<'ctx>,
    ) -> anyhow::Result<Self> {
        let ty = NDIterType {
            inner: BuiltinStruct::new(ctx, "nditer"),
            dtype: ndarray.ty.dtype,
            ndims: ndarray.ty.ndims,
        };

        let nditer = ty.alloca(ctx, None)?;

        // The caller has the responsibility to allocate 'indices' for `NDIter`.
        let indices = gen_array_var(ctx, ctx.size_t, ndarray.ty.ndims, None)?.value.0;
        let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");
        call_extern!(ctx: void _ = name(nditer.value, ndarray.value, indices))?;

        Ok(nditer)
    }

    /// Advances the iterator to the next element.
    pub fn next(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_next");
        call_extern!(ctx: void _ = name(self.value))?;
        Ok(())
    }

    /// Returns whether the iterator is currently referring to a valid element.
    pub fn has_element(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<IntValue<'ctx>> {
        let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_has_element");
        call_extern!(ctx: (ctx.i1) _ = name(self.value))
    }

    /// Returns a pointer to the current element.
    pub fn curr_ptr(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        self.load(ctx, field!(element))
    }

    /// Loads and returns the current element as a scalar value.
    pub fn get_scalar(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        let p = self.curr_ptr(ctx)?;
        typed_load(ctx.builder, p, self.ty.dtype, "value")
    }

    /// Returns the current iteration index (i.e., how many elements have been iterated so far).
    pub fn get_index(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        self.load(ctx, field!(nth))
    }

    /// Returns the current indices in each dimension as an array.
    pub fn indices(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        let indices_ptr = self.load(ctx, field!(indices))?;
        Ok(ArraySliceValue::new(ctx.size_t.into(), indices_ptr, self.ty.ndims_val(ctx), self.name))
    }
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Iterate through every element in the ndarray.
    ///
    /// `body` has access to [`BreakContinueHooks`] to short-circuit and [`NDIterValue`] to
    /// get properties of the current iteration (e.g., the current element, indices, etc.)
    pub fn foreach<'a, F>(&self, ctx: &mut CodeGenContext<'ctx, 'a>, body: F) -> anyhow::Result<()>
    where
        F: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            BreakContinueHooks<'ctx>,
            NDIterValue<'ctx>,
        ) -> anyhow::Result<()>,
    {
        gen_for_callback(
            &mut (),
            ctx,
            Some("ndarray_foreach"),
            |(), ctx| NDIterValue::new(ctx, *self),
            |(), ctx, nditer| nditer.has_element(ctx),
            |(), ctx, hooks, nditer| body(ctx, hooks, nditer),
            |(), ctx, nditer| {
                nditer.next(ctx)?;
                Ok(())
            },
            |(), _| Ok(()),
        )
    }

    /// Folds the elements of this ndarray into an accumulator value by applying `f`, returning the
    /// final value.
    ///
    /// `f` has access to [`BreakContinueHooks`] to short-circuit the `fold` operation, an instance
    /// of `V` representing the current accumulated value, and an [`NDIterValue`] to get the
    /// properties of the current iterated element.
    pub fn fold<'a, V, F>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        init: V,
        f: F,
    ) -> anyhow::Result<V>
    where
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug> + Copy,
        F: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            BreakContinueHooks<'ctx>,
            V,
            NDIterValue<'ctx>,
        ) -> anyhow::Result<V>,
    {
        let init = init.as_basic_value_enum();
        let acc_ptr = gen_var(ctx, init.get_type(), None)?;
        typed_store(ctx.builder, acc_ptr, init)?;

        gen_for_callback(
            &mut (),
            ctx,
            Some("ndarray_fold"),
            |(), ctx| NDIterValue::new(ctx, *self),
            |(), ctx, nditer| nditer.has_element(ctx),
            |(), ctx, hooks, nditer| {
                let acc = V::try_from(ctx.builder.build_load(acc_ptr, "")?)
                    .map_err(|e| anyhow!("{e:?}"))?;
                let acc = f(ctx, hooks, acc, nditer)?;
                typed_store(ctx.builder, acc_ptr, acc)?;
                Ok(())
            },
            |(), ctx, nditer| {
                nditer.next(ctx)?;
                Ok(())
            },
            |(), _| Ok(()),
        )?;

        let acc = ctx.builder.build_load(acc_ptr, "")?;
        V::try_from(acc).map_err(|e| anyhow!("{e:?}"))
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
    pub fn fold<'a, V, F>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        init: V,
        f: F,
    ) -> anyhow::Result<V>
    where
        V: BasicValue<'ctx> + TryFrom<BasicValueEnum<'ctx>, Error: std::fmt::Debug> + Copy,
        F: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            Option<&BreakContinueHooks<'ctx>>,
            V,
            BasicValueEnum<'ctx>,
        ) -> anyhow::Result<V>,
    {
        match self {
            ScalarOrNDArray::Scalar(v) => f(ctx, None, init, *v),
            ScalarOrNDArray::NDArray(v) => v.fold(ctx, init, |ctx, hooks, acc, nditer| {
                let elem = nditer.get_scalar(ctx)?;
                f(ctx, Some(&hooks), acc, elem)
            }),
        }
    }
}
