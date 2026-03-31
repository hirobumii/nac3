use std::borrow::Cow;

use anyhow::anyhow;
use inkwell::{
    types::{BasicTypeEnum, IntType},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    allocator::AllocationScope,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    stmt::{BreakContinueHooks, gen_for_callback},
    typed_load, typed_store,
    types::{
        NDArrayType, NDArrayValue, ProxyTypeBase, RefCountedArrayType, TypedRefCountedType,
        TypedRefCountedValue, Value, WithTypeinfo, array::ArraySliceValue, builtin::BuiltinStruct,
        field, ndarray::ScalarOrNDArray, structure::StructField,
    },
};

#[derive(Clone, Copy, StructFields)]
pub struct NDIterStructFields<'ctx> {
    #[value_type(ptr)]
    array: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    indices: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(size_t)]
    pub nth: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(size_t)]
    pub offset: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(size_t)]
    pub size: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct RawNDIterType<'ctx> {
    pub(crate) inner: BuiltinStruct<'ctx, NDIterStructFields<'ctx>>,
    dtype: BasicTypeEnum<'ctx>,
    ndims: u64,
}

impl<'ctx> RawNDIterType<'ctx> {
    fn ndims_val(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        ctx.size_t.const_int(self.ndims, false)
    }
}

pub type NDIterType<'ctx> = TypedRefCountedType<'ctx, RawNDIterType<'ctx>>;
pub type RawNDIterValue<'ctx> = Value<'ctx, RawNDIterType<'ctx>>;
pub type NDIterValue<'ctx> = TypedRefCountedValue<'ctx, RawNDIterType<'ctx>>;

impl<'ctx> RawNDIterValue<'ctx> {
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

    fn array(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<NDArrayValue<'ctx>> {
        let array_ptr = self.load(ctx, field!(array))?;
        Ok(NDArrayType::create(ctx, self.ty.dtype, self.ty.ndims).map_value(array_ptr, self.name))
    }

    /// Returns a pointer to the current element.
    pub fn curr_ptr(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        let data = self.array(ctx)?.inner_value(ctx)?.base_data(ctx)?;
        let array_size = self.load(ctx, field!(size))?;
        let data_ptr = data.inner_value(ctx, Some(array_size))?.value.0;
        Ok(unsafe { ctx.builder.build_gep(data_ptr, &[self.load(ctx, field!(offset))?], "")? })
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
    ) -> anyhow::Result<ArraySliceValue<'ctx, IntType<'ctx>>> {
        let indices_ptr = self.load(ctx, field!(indices))?;
        let ndims = self.ty.ndims_val(ctx);
        let indices_arr = RefCountedArrayType::new(ctx, ctx.size_t, Some(self.ty.ndims as u32))
            .map_value(indices_ptr, self.name);
        indices_arr.inner_value(ctx, Some(ndims))
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawNDIterType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_nditer")
    }

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        vec![ctx.i32.const_zero(), ctx.i32.const_int(ctx.sizeof(ctx.ptr), false)]
    }
}

impl<'ctx> NDIterValue<'ctx> {
    /// Creates an iterator that iterates through `ndarray`.
    pub fn new(
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarray: NDArrayValue<'ctx>,
    ) -> anyhow::Result<Self> {
        let ty = TypedRefCountedType::new(
            ctx,
            RawNDIterType {
                inner: BuiltinStruct::new(ctx, "nditer"),
                dtype: ndarray.ty.object.dtype,
                ndims: ndarray.ty.object.ndims,
            },
        );

        let nditer = ty.allocate(ctx, AllocationScope::Default, None)?;

        // The caller has the responsibility to allocate 'indices' for `NDIter`.
        let indices =
            RefCountedArrayType::new(ctx, ctx.size_t, Some(ndarray.ty.object.ndims as u32))
                .allocate(ctx, ctx.size_t.const_int(ndarray.ty.object.ndims, false), None)?;
        let name = get_usize_dependent_function_name(ctx, "__nac3_nditer_initialize");
        call_extern!(ctx: void _ = name(nditer.inner_value(ctx)?.value, ndarray.value, indices.value))?;

        Ok(nditer)
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
            |(), ctx, nditer| nditer.inner_value(ctx)?.has_element(ctx),
            |(), ctx, hooks, nditer| body(ctx, hooks, nditer),
            |(), ctx, nditer| {
                nditer.inner_value(ctx)?.next(ctx)?;
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
        let acc_ptr = ctx.build_allocate(AllocationScope::Default, init.get_type(), None)?;
        typed_store(ctx.builder, acc_ptr, init)?;

        gen_for_callback(
            &mut (),
            ctx,
            Some("ndarray_fold"),
            |(), ctx| NDIterValue::new(ctx, *self),
            |(), ctx, nditer| nditer.inner_value(ctx)?.has_element(ctx),
            |(), ctx, hooks, nditer| {
                let acc = V::try_from(ctx.builder.build_load(acc_ptr, "")?)
                    .map_err(|e| anyhow!("{e:?}"))?;
                let acc = f(ctx, hooks, acc, nditer)?;
                typed_store(ctx.builder, acc_ptr, acc)?;
                Ok(())
            },
            |(), ctx, nditer| {
                nditer.inner_value(ctx)?.next(ctx)?;
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
                let elem = nditer.inner_value(ctx)?.get_scalar(ctx)?;
                f(ctx, Some(&hooks), acc, elem)
            }),
        }
    }
}
