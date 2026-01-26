use inkwell::values::{IntValue, PointerValue};
use nac3core_derive::StructFields;

use crate::codegen::{
    CodeGenContext,
    stmt::gen_if_callback,
    types::{
        ProxyTypeBase, Value,
        builtin::BuiltinStruct,
        field,
        ndarray::{NDArrayLikeType, NDArrayType, NDArrayValue},
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
}

pub type ContiguousNDArrayType<'ctx> = NDArrayLikeType<'ctx, ContiguousNDArrayStructFields<'ctx>>;
pub type ContiguousNDArrayValue<'ctx> = Value<'ctx, ContiguousNDArrayType<'ctx>>;

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
        let result = ContiguousNDArrayType {
            inner: BuiltinStruct::new(ctx, "contiguous_ndarray"),
            dtype: self.ty.dtype,
            ndims: self.ty.ndims,
        };
        let result = result.allocate(ctx, self.name)?;

        // Set ndims and shape.
        let ndims = self.ty.ndims_val(ctx);
        result.store(ctx, field!(ndims), ndims)?;

        let shape = self.load(ctx, field!(shape))?;
        result.store(ctx, field!(shape), shape)?;

        gen_if_callback(
            &mut (),
            ctx,
            |(), ctx| self.is_c_contiguous(ctx),
            |(), ctx| {
                // This ndarray is contiguous.
                let data = self.load(ctx, field!(data))?;
                result.store(ctx, field!(data), data)?;
                Ok(())
            },
            |(), ctx| {
                // This ndarray is not contiguous. Do a full-copy on `data`. `make_copy` produces an
                // ndarray with contiguous `data`.
                let copied_ndarray = self.make_copy(ctx)?;
                let data = copied_ndarray.load(ctx, field!(data))?;
                result.store(ctx, field!(data), data)?;

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
        let ndarray = NDArrayType::new(ctx, carray.ty.dtype, ndims).construct(ctx, carray.name)?;

        // Copy shape and update strides
        let shape = carray.load(ctx, field!(shape))?;
        ndarray.shape(ctx)?.memcpy_from(ctx, shape)?;
        ndarray.set_strides_contiguous(ctx)?;

        // Share data
        let data = carray.load(ctx, field!(data))?;
        ndarray.store(ctx, field!(data), data)?;

        Ok(ndarray)
    }
}
