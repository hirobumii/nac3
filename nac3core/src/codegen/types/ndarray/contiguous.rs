use inkwell::{
    types::{BasicTypeEnum, IntType},
    values::{IntValue, PointerValue},
};
use nac3core_derive::StructFields;

use crate::codegen::{
    CodeGenContext, ModuleContext,
    allocator::AllocationScope,
    stmt::gen_if_callback,
    types::{
        NDArrayType, OpaqueRefCountedType, OpaqueRefCountedValue, ProxyTypeBase as _,
        RefCountedArrayType, RefCountedArrayValue, RefCountedValue as _, TypedRefCountedType,
        TypedRefCountedValue, Value, WithTypeinfo,
        builtin::BuiltinStruct,
        field,
        ndarray::{NDArrayLikeType, NDArrayValue},
        structure::StructField,
    },
};

#[derive(Clone, Copy, StructFields)]
pub struct ContiguousNDArrayStructFields<'ctx> {
    #[value_type(size_t)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(ptr)]
    pub base: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(size_t)]
    pub offset: StructField<'ctx, IntValue<'ctx>>,
}

pub type RawContiguousNDArrayType<'ctx> =
    NDArrayLikeType<'ctx, ContiguousNDArrayStructFields<'ctx>>;

impl<'ctx> WithTypeinfo<'ctx> for RawContiguousNDArrayType<'ctx> {
    fn typename() -> &'static str {
        "__nac3_contiguous_ndarray"
    }

    fn refcounted_field_offset(ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        vec![
            ctx.i32.const_int(ctx.sizeof(ctx.size_t), false),
            ctx.i32.const_int(ctx.sizeof(ctx.size_t) + ctx.sizeof(ctx.ptr), false),
            ctx.i32.const_int(ctx.sizeof(ctx.size_t) + 2 * ctx.sizeof(ctx.ptr), false),
        ]
    }
}

pub type ContiguousNDArrayType<'ctx> = TypedRefCountedType<'ctx, RawContiguousNDArrayType<'ctx>>;
pub type RawContiguousNDArrayValue<'ctx> = Value<'ctx, RawContiguousNDArrayType<'ctx>>;

impl<'ctx> RawContiguousNDArrayValue<'ctx> {
    pub fn shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        let shape = self.load(ctx, field!(shape))?;
        Ok(RefCountedArrayType::new(ctx, ctx.size_t, None).map_value(shape, None))
    }

    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, BasicTypeEnum<'ctx>>> {
        let data = self.load(ctx, field!(data))?;
        Ok(RefCountedArrayType::new(ctx, ctx.i8.into(), None).map_value(data, None))
    }

    pub fn base(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<OpaqueRefCountedValue<'ctx>> {
        Ok(OpaqueRefCountedType::new(ctx).map_value(self.load(ctx, field!(base))?, None))
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
        let result = TypedRefCountedType::new(
            ctx,
            RawContiguousNDArrayType {
                inner: BuiltinStruct::new(ctx, "contiguous_ndarray"),
                dtype: self.ty.object.dtype,
                ndims: self.ty.object.ndims,
            },
        );
        let result = result.allocate(ctx, AllocationScope::Default, self.name)?;

        // Set ndims and shape.
        let ndims = self.ty.object.ndims_val(ctx);
        result.inner_value(ctx)?.store(ctx, field!(ndims), ndims)?;

        let shape = self.inner_value(ctx)?.load(ctx, field!(shape))?;
        result.inner_value(ctx)?.store(ctx, field!(shape), shape)?;

        gen_if_callback(
            &mut (),
            ctx,
            |(), ctx| self.is_c_contiguous(ctx),
            |(), ctx| {
                // This ndarray is contiguous.
                let data = self.inner_value(ctx)?.load(ctx, field!(data))?;
                result.inner_value(ctx)?.store(ctx, field!(data), data)?;

                let base = self.inner_value(ctx)?.load(ctx, field!(base))?;
                result.inner_value(ctx)?.store(ctx, field!(base), base)?;

                let offset = self.inner_value(ctx)?.load(ctx, field!(offset))?;
                result.inner_value(ctx)?.store(ctx, field!(offset), offset)?;

                Ok(())
            },
            |(), ctx| {
                // This ndarray is not contiguous. Do a full-copy on `data`. `make_copy` produces an
                // ndarray with contiguous `data`.
                let copied_ndarray = self.make_copy(ctx)?;
                let data = copied_ndarray.inner_value(ctx)?.load(ctx, field!(data))?;
                copied_ndarray.header(ctx).increment_refcount(ctx);
                result.inner_value(ctx)?.store(ctx, field!(data), data)?;

                result.inner_value(ctx)?.store(ctx, field!(base), ctx.ptr.const_null())?;
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
    ///
    /// `ndims` has to be provided as [`NDArrayValue`] requires a statically known `ndims` value,
    /// despite the fact that the information should be contained within the
    /// [`ContiguousNDArrayValue`].
    pub fn from_contiguous_ndarray(
        ctx: &mut CodeGenContext<'ctx, '_>,
        carray: ContiguousNDArrayValue<'ctx>,
        ndims: u64,
    ) -> anyhow::Result<Self> {
        // TODO: Debug assert `ndims == carray.ndims` to catch bugs.

        // Allocate the resulting ndarray.
        let ndarray =
            NDArrayType::create(ctx, carray.ty.object.dtype, ndims).construct(ctx, carray.name)?;

        // Copy shape and update strides
        let shape = carray.inner_value(ctx)?.shape(ctx)?;
        ndarray.shape(ctx)?.inner_value(ctx)?.memcpy_from(ctx, shape.inner_value(ctx)?.value.0)?;
        ndarray.set_strides_contiguous(ctx)?;

        // Share data
        let data = carray.inner_value(ctx)?.data(ctx)?;
        data.header(ctx).safe_increment_refcount(ctx)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(data), data.value)?;

        let base = carray.inner_value(ctx)?.data(ctx)?;
        base.header(ctx).safe_increment_refcount(ctx)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(base), base.value)?;

        let offset = carray.inner_value(ctx)?.load(ctx, field!(offset))?;
        ndarray.inner_value(ctx)?.store(ctx, field!(offset), offset)?;

        Ok(ndarray)
    }
}
