use inkwell::{
    context::Context,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::IntValue,
    AddressSpace,
};

use super::ProxyType;
use crate::codegen::{
    values::{ArraySliceValue, NDArrayValue, ProxyValue},
    {CodeGenContext, CodeGenerator},
};

/// Proxy type for a `ndarray` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDArrayType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> NDArrayType<'ctx> {
    /// Checks whether `llvm_ty` represents a `ndarray` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let llvm_ndarray_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ndarray_ty) = llvm_ndarray_ty else {
            return Err(format!("Expected struct type for `NDArray` type, got {llvm_ndarray_ty}"));
        };
        if llvm_ndarray_ty.count_fields() != 3 {
            return Err(format!(
                "Expected 3 fields in `NDArray`, got {}",
                llvm_ndarray_ty.count_fields()
            ));
        }

        let ndarray_ndims_ty = llvm_ndarray_ty.get_field_type_at_index(0).unwrap();
        let Ok(ndarray_ndims_ty) = IntType::try_from(ndarray_ndims_ty) else {
            return Err(format!("Expected int type for `ndarray.0`, got {ndarray_ndims_ty}"));
        };
        if ndarray_ndims_ty.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!(
                "Expected {}-bit int type for `ndarray.0`, got {}-bit int",
                llvm_usize.get_bit_width(),
                ndarray_ndims_ty.get_bit_width()
            ));
        }

        let ndarray_dims_ty = llvm_ndarray_ty.get_field_type_at_index(1).unwrap();
        let Ok(ndarray_pdims) = PointerType::try_from(ndarray_dims_ty) else {
            return Err(format!("Expected pointer type for `ndarray.1`, got {ndarray_dims_ty}"));
        };
        let ndarray_dims = ndarray_pdims.get_element_type();
        let Ok(ndarray_dims) = IntType::try_from(ndarray_dims) else {
            return Err(format!(
                "Expected pointer-to-int type for `ndarray.1`, got pointer-to-{ndarray_dims}"
            ));
        };
        if ndarray_dims.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!(
                "Expected pointer-to-{}-bit int type for `ndarray.1`, got pointer-to-{}-bit int",
                llvm_usize.get_bit_width(),
                ndarray_dims.get_bit_width()
            ));
        }

        let ndarray_data_ty = llvm_ndarray_ty.get_field_type_at_index(2).unwrap();
        let Ok(_) = PointerType::try_from(ndarray_data_ty) else {
            return Err(format!("Expected pointer type for `ndarray.2`, got {ndarray_data_ty}"));
        };

        Ok(())
    }

    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        dtype: BasicTypeEnum<'ctx>,
    ) -> Self {
        let llvm_usize = generator.get_size_type(ctx);

        // struct NDArray { num_dims: size_t, dims: size_t*, data: T* }
        //
        // * num_dims: Number of dimensions in the array
        // * dims: Pointer to an array containing the size of each dimension
        // * data: Pointer to an array containing the array data
        let llvm_ndarray = ctx
            .struct_type(
                &[
                    llvm_usize.into(),
                    llvm_usize.ptr_type(AddressSpace::default()).into(),
                    dtype.ptr_type(AddressSpace::default()).into(),
                ],
                false,
            )
            .ptr_type(AddressSpace::default());

        NDArrayType::from_type(llvm_ndarray, llvm_usize)
    }

    /// Creates an [`NDArrayType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        NDArrayType { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of the `size` field of this `ndarray` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(0)
            .map(BasicTypeEnum::into_int_type)
            .unwrap()
    }

    /// Returns the element type of this `ndarray` type.
    #[must_use]
    pub fn element_type(&self) -> AnyTypeEnum<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(2)
            .map(BasicTypeEnum::into_pointer_type)
            .map(PointerType::get_element_type)
            .unwrap()
    }
}

impl<'ctx> ProxyType<'ctx> for NDArrayType<'ctx> {
    type Base = PointerType<'ctx>;
    type Value = NDArrayValue<'ctx>;

    fn is_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: impl BasicType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            <Self as ProxyType<'ctx>>::is_representable(generator, ctx, ty)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String> {
        Self::is_representable(llvm_ty, generator.get_size_type(ctx))
    }

    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        self.map_value(
            generator
                .gen_var_alloc(
                    ctx,
                    self.as_base_type().get_element_type().into_struct_type().into(),
                    name,
                )
                .unwrap(),
            name,
        )
    }

    fn new_array_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        generator
            .gen_array_var_alloc(
                ctx,
                self.as_base_type().get_element_type().into_struct_type().into(),
                size,
                name,
            )
            .unwrap()
    }

    fn map_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        debug_assert_eq!(value.get_type(), self.as_base_type());

        NDArrayValue::from_pointer_value(value, self.llvm_usize, name)
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<NDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}
