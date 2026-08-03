use std::borrow::Cow;

use inkwell::{
    types::{BasicTypeEnum, IntType},
    values::{IntValue, PointerValue},
};
use nac3core_derive::StructFields;

use crate::codegen::{
    CodeGenContext, ModuleContext,
    allocator::AllocationScope,
    types::{
        NDArrayType, ProxyTypeBase as _, RefCountedValue as _, RefType, TypedRefCountedType,
        TypedRefCountedValue, Value, WithTypeinfo,
        array::ArraySliceValue,
        builtin::BuiltinStruct,
        field,
        ndarray::{NDArrayLikeType, NDArrayValue},
        refcounted_fields_for_struct,
        structure::StructField,
    },
};

#[derive(Clone, Copy, StructFields)]
pub struct ContiguousNDArrayStructFields<'ctx> {
    #[value_type(size_t)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    data: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    base: StructField<'ctx, PointerValue<'ctx>>,
    // TODO(Derppening): This field is unused and is always set to zero
    #[value_type(size_t)]
    pub offset: StructField<'ctx, IntValue<'ctx>>,
}

pub type RawContiguousNDArrayType<'ctx> =
    NDArrayLikeType<'ctx, ContiguousNDArrayStructFields<'ctx>>;

impl<'ctx> RawContiguousNDArrayType<'ctx> {
    pub fn new(ctx: &ModuleContext<'ctx>, dtype: BasicTypeEnum<'ctx>, ndims: u64) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "contiguous_ndarray"), dtype, ndims }
    }
}

impl<'ctx> RefType<'ctx> for NDArrayLikeType<'ctx, ContiguousNDArrayStructFields<'ctx>> {
    fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.llvm_ty.into()
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawContiguousNDArrayType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_contiguous_ndarray")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        // `RawContiguousNDArray`s are weak pointers to their `base`, so they are not
        // reference-counted.
        refcounted_fields_for_struct(ctx, Vec::new())
    }
}

pub type ContiguousNDArrayType<'ctx> = TypedRefCountedType<'ctx, RawContiguousNDArrayType<'ctx>>;

impl<'ctx> ContiguousNDArrayType<'ctx> {
    /// Creates an instance of [`ContiguousNDArrayType`].
    pub fn create(ctx: &ModuleContext<'ctx>, dtype: BasicTypeEnum<'ctx>, ndims: u64) -> Self {
        Self::new(ctx, RawContiguousNDArrayType::new(ctx, dtype, ndims))
    }
}

pub type RawContiguousNDArrayValue<'ctx> = Value<'ctx, RawContiguousNDArrayType<'ctx>>;

impl<'ctx> RawContiguousNDArrayValue<'ctx> {
    /// Returns the shape of this array as an [`ArraySliceValue`].
    ///
    /// The pointer wrapped by the returned slice points to the first shape element. `shape` is
    /// weakly owned by this contiguous array; the strong owner is [`base`][Self::base].
    pub fn shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, IntType<'ctx>>> {
        let shape = self.load(ctx, field!(shape))?;
        let len = ctx.size_t.const_int(self.ty.ndims, false);
        Ok(ArraySliceValue::new(ctx.size_t, shape, len, self.name))
    }

    /// Returns the underlying data of this array as an [`ArraySliceValue`].
    ///
    /// The pointer returned by this function points to the first element of the array, similar to
    /// [`NDArrayValue::data`].
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, BasicTypeEnum<'ctx>>> {
        let data = self.load(ctx, field!(data))?;
        let size = self.base(ctx)?.size(ctx)?;
        Ok(ArraySliceValue::new(self.ty.dtype, data, size, self.name))
    }

    /// Returns the base of this array, i.e. the instance of the underlying allocation.
    ///
    /// See [`NDArrayValue::base`] for more details.
    pub fn base(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<NDArrayValue<'ctx>> {
        let base = self.load(ctx, field!(base))?;
        Ok(NDArrayType::create(ctx, self.ty.dtype, self.ty.ndims).map_value(base, self.name))
    }
}

pub type ContiguousNDArrayValue<'ctx> = TypedRefCountedValue<'ctx, RawContiguousNDArrayType<'ctx>>;

impl<'ctx> NDArrayValue<'ctx> {
    /// Create a [`ContiguousNDArrayValue`] from the contents of this ndarray.
    ///
    /// This function may or may not be expensive depending on if this ndarray has contiguous data.
    ///
    /// If this ndarray is not C-contiguous, this function will allocate memory on the stack for the
    /// `data` field of the returned [`ContiguousNDArrayValue`] and copy contents of this ndarray to
    /// there.
    ///
    /// If this ndarray is C-contiguous, contents of this ndarray will not be copied. The created
    /// [`ContiguousNDArrayValue`] will share memory with this ndarray.
    pub fn make_contiguous_ndarray(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ContiguousNDArrayValue<'ctx>> {
        let result = ContiguousNDArrayType::create(ctx, self.ty.object.dtype, self.ty.object.ndims);
        let result = result.allocate(ctx, AllocationScope::Default, self.name)?;

        // Set ndims and shape.
        let ndims = self.ty.object.ndims_val(ctx);
        result.inner_value(ctx)?.store(ctx, field!(ndims), ndims)?;

        let shape = self.inner_value(ctx)?.shape(ctx)?;
        let shape_data_ptr = shape.inner_value(ctx, Some(ndims))?.value.0;
        result.inner_value(ctx)?.store(ctx, field!(shape), shape_data_ptr)?;

        let is_c_contiguous = self.is_c_contiguous(ctx)?;
        ctx.build_if_else(
            is_c_contiguous,
            |ctx| {
                // This ndarray is contiguous.
                let data = self.inner_value(ctx)?.data(ctx)?;
                result.inner_value(ctx)?.store(ctx, field!(data), data.value.0)?;
                result.inner_value(ctx)?.store(ctx, field!(base), self.value)?;
                result.inner_value(ctx)?.store(ctx, field!(offset), ctx.size_t.const_zero())?;
                Ok(())
            },
            |ctx, ()| {
                // This ndarray is not contiguous. Do a full-copy on `data`. `make_copy` produces an
                // ndarray with contiguous `data`.
                let copied_ndarray = self.make_copy(ctx)?;
                let data = copied_ndarray.inner_value(ctx)?.data(ctx)?;
                result.inner_value(ctx)?.store(ctx, field!(data), data.value.0)?;
                result.inner_value(ctx)?.store(ctx, field!(base), copied_ndarray.value)?;
                result.inner_value(ctx)?.store(ctx, field!(offset), ctx.size_t.const_zero())?;
                Ok(())
            },
        )?;

        Ok(result)
    }

    /// Create an [`NDArrayValue`] from a [`ContiguousNDArrayValue`].
    ///
    /// The operation is cheap. The newly created [`NDArrayValue`] will share the same memory as the
    /// [`ContiguousNDArrayValue`].
    pub fn from_contiguous_ndarray(
        ctx: &mut CodeGenContext<'ctx, '_>,
        carray: ContiguousNDArrayValue<'ctx>,
    ) -> anyhow::Result<Self> {
        // The statically-known `ndims` is carried by the contiguous array's type.
        let dtype = carray.ty.object.dtype;
        let ndims = carray.ty.object.ndims;

        // Allocate the resulting ndarray.
        let ndarray = NDArrayType::create(ctx, dtype, ndims).construct(ctx, carray.name)?;

        // Reconstruct the view from `base`, and take a strong reference to it for refcounting
        let base = carray.inner_value(ctx)?.base(ctx)?;
        base.header(ctx).safe_increment_refcount(ctx)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(base), base.value)?;

        // Copy shape and update strides
        let shape = carray.inner_value(ctx)?.shape(ctx)?;
        ndarray.shape(ctx)?.inner_value(ctx, None)?.memcpy_from(ctx, shape.value.0)?;
        ndarray.set_strides_contiguous(ctx)?;

        // Take a strong reference to `data` for refcounting
        let data = base.inner_value(ctx)?.base_data(ctx)?;
        data.header(ctx).safe_increment_refcount(ctx)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(data), data.value)?;

        let offset = base.inner_value(ctx)?.load(ctx, field!(offset))?;
        ndarray.inner_value(ctx)?.store(ctx, field!(offset), offset)?;

        Ok(ndarray)
    }
}
