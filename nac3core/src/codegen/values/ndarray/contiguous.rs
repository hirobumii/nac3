use inkwell::{
    AddressSpace,
    types::{BasicType, BasicTypeEnum, IntType},
    values::{IntValue, PointerValue, StructValue},
};

use super::NDArrayValue;
use crate::codegen::{
    CodeGenContext, CodeGenerator,
    stmt::gen_if_callback,
    types::{
        ndarray::{ContiguousNDArrayType, NDArrayType},
        structure::{StructField, StructProxyType},
    },
    values::{ArrayLikeValue, ProxyValue, structure::StructProxyValue},
};

#[derive(Copy, Clone)]
pub struct ContiguousNDArrayValue<'ctx> {
    value: PointerValue<'ctx>,
    item: BasicTypeEnum<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> ContiguousNDArrayValue<'ctx> {
    /// Creates an [`ContiguousNDArrayValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
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
        Self::from_pointer_value(pval, dtype, llvm_usize, name)
    }

    /// Creates an [`ContiguousNDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        dtype: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        Self { value: ptr, item: dtype, llvm_usize, name }
    }

    fn ndims_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().ndims
    }

    pub fn store_ndims(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
        self.ndims_field().store(ctx, self.as_abi_value(ctx), value, self.name);
    }

    fn shape_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().shape
    }

    pub fn store_shape(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: PointerValue<'ctx>) {
        self.shape_field().store(ctx, self.as_abi_value(ctx), value, self.name);
    }

    pub fn load_shape(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.shape_field().load(ctx, self.value, self.name)
    }

    fn data_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().data
    }

    pub fn store_data(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: PointerValue<'ctx>) {
        self.data_field().store(ctx, self.as_abi_value(ctx), value, self.name);
    }

    pub fn load_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.data_field().load(ctx, self.value, self.name)
    }
}

impl<'ctx> ProxyValue<'ctx> for ContiguousNDArrayValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = ContiguousNDArrayType<'ctx>;

    fn get_type(&self) -> Self::Type {
        <Self as ProxyValue<'ctx>>::Type::from_pointer_type(
            self.as_base_value().get_type(),
            self.item,
            self.llvm_usize,
        )
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for ContiguousNDArrayValue<'ctx> {}

impl<'ctx> From<ContiguousNDArrayValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ContiguousNDArrayValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

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
    pub fn make_contiguous_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> ContiguousNDArrayValue<'ctx> {
        let result =
            ContiguousNDArrayType::new(ctx, &self.dtype).alloca_var(generator, ctx, self.name);

        // Set ndims and shape.
        let ndims = self.llvm_usize.const_int(self.ndims, false);
        result.store_ndims(ctx, ndims);

        let shape = self.shape();
        result.store_shape(ctx, shape.base_ptr(ctx, generator));

        gen_if_callback(
            generator,
            ctx,
            |_, ctx| Ok(self.is_c_contiguous(ctx)),
            |_, ctx| {
                // This ndarray is contiguous.
                let data = self.data_field().load(ctx, self.as_abi_value(ctx), self.name);
                let data = ctx
                    .builder
                    .build_pointer_cast(data, result.item.ptr_type(AddressSpace::default()), "")
                    .unwrap();
                result.store_data(ctx, data);

                Ok(())
            },
            |generator, ctx| {
                // This ndarray is not contiguous. Do a full-copy on `data`. `make_copy` produces an
                // ndarray with contiguous `data`.
                let copied_ndarray = self.make_copy(generator, ctx);
                let data = copied_ndarray.data().base_ptr(ctx, generator);
                let data = ctx
                    .builder
                    .build_pointer_cast(data, result.item.ptr_type(AddressSpace::default()), "")
                    .unwrap();
                result.store_data(ctx, data);

                Ok(())
            },
        )
        .unwrap();

        result
    }

    /// Create an [`NDArrayValue`] from a [`ContiguousNDArrayValue`].
    ///
    /// The operation is cheap. The newly created [`NDArrayValue`] will share the same memory as the
    /// [`ContiguousNDArrayValue`].
    ///
    /// `ndims` has to be provided as [`NDArrayValue`] requires a statically known `ndims` value,
    /// despite the fact that the information should be contained within the
    /// [`ContiguousNDArrayValue`].
    pub fn from_contiguous_ndarray<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        carray: ContiguousNDArrayValue<'ctx>,
        ndims: u64,
    ) -> Self {
        // TODO: Debug assert `ndims == carray.ndims` to catch bugs.

        // Allocate the resulting ndarray.
        let ndarray = NDArrayType::new(ctx, carray.item, ndims).construct_uninitialized(
            generator,
            ctx,
            carray.name,
        );

        // Copy shape and update strides
        let shape = carray.load_shape(ctx);
        ndarray.copy_shape_from_array(generator, ctx, shape);
        ndarray.set_strides_contiguous(ctx);

        // Share data
        let data = carray.load_data(ctx);
        ndarray.store_data(ctx, ctx.builder.build_pointer_cast(data, ctx.ptr, "").unwrap());

        ndarray
    }
}
