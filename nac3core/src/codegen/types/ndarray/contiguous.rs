use inkwell::{
    context::Context,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use crate::{
    codegen::{
        types::{
            structure::{
                check_struct_type_matches_fields, FieldIndexCounter, StructField, StructFields,
            },
            ProxyType,
        },
        values::{ndarray::ContiguousNDArrayValue, ProxyValue},
        CodeGenContext, CodeGenerator,
    },
    toplevel::numpy::unpack_ndarray_var_tys,
    typecheck::typedef::Type,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ContiguousNDArrayType<'ctx> {
    ty: PointerType<'ctx>,
    item: BasicTypeEnum<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct ContiguousNDArrayStructFields<'ctx> {
    #[value_type(usize)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

impl<'ctx> ContiguousNDArrayStructFields<'ctx> {
    #[must_use]
    pub fn new_typed(item: BasicTypeEnum<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let mut counter = FieldIndexCounter::default();

        ContiguousNDArrayStructFields {
            ndims: StructField::create(&mut counter, "ndims", llvm_usize),
            shape: StructField::create(
                &mut counter,
                "shape",
                llvm_usize.ptr_type(AddressSpace::default()),
            ),
            data: StructField::create(&mut counter, "data", item.ptr_type(AddressSpace::default())),
        }
    }
}

impl<'ctx> ContiguousNDArrayType<'ctx> {
    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(
        item: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> ContiguousNDArrayStructFields<'ctx> {
        ContiguousNDArrayStructFields::new_typed(item, llvm_usize)
    }

    /// See [`NDArrayType::fields`].
    // TODO: Move this into e.g. StructProxyType
    #[must_use]
    pub fn get_fields(&self) -> ContiguousNDArrayStructFields<'ctx> {
        Self::fields(self.item, self.llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of an `NDArray`.
    #[must_use]
    fn llvm_type(
        ctx: &'ctx Context,
        item: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(item, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: &'ctx Context, item: BasicTypeEnum<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_cndarray = Self::llvm_type(ctx, item, llvm_usize);

        Self { ty: llvm_cndarray, item, llvm_usize }
    }

    /// Creates an instance of [`ContiguousNDArrayType`].
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>, item: &impl BasicType<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, item.as_basic_type_enum(), ctx.get_size_type())
    }

    /// Creates an instance of [`ContiguousNDArrayType`].
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        item: BasicTypeEnum<'ctx>,
    ) -> Self {
        Self::new_impl(ctx, item, generator.get_size_type(ctx))
    }

    /// Creates an [`ContiguousNDArrayType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ty: Type,
    ) -> Self {
        let (dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);

        let llvm_dtype = ctx.get_llvm_type(generator, dtype);

        Self::new_impl(ctx.ctx, llvm_dtype, ctx.get_size_type())
    }

    /// Creates an [`ContiguousNDArrayType`] from a [`PointerType`] representing an `NDArray`.
    #[must_use]
    pub fn from_type(
        ptr_ty: PointerType<'ctx>,
        item: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, item, llvm_usize }
    }

    /// Allocates an instance of [`ContiguousNDArrayValue`] as if by calling `alloca` on the base
    /// type.
    ///
    /// See [`ProxyType::raw_alloca`].
    #[must_use]
    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(ctx, name),
            self.item,
            self.llvm_usize,
            name,
        )
    }

    /// Allocates an instance of [`ContiguousNDArrayValue`] as if by calling `alloca` on the base
    /// type.
    ///
    /// See [`ProxyType::raw_alloca_var`].
    #[must_use]
    pub fn alloca_var<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(generator, ctx, name),
            self.item,
            self.llvm_usize,
            name,
        )
    }

    /// Converts an existing value into a [`ContiguousNDArrayValue`].
    #[must_use]
    pub fn map_value(
        &self,
        value: <<Self as ProxyType<'ctx>>::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            value,
            self.item,
            self.llvm_usize,
            name,
        )
    }
}

impl<'ctx> ProxyType<'ctx> for ContiguousNDArrayType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = ContiguousNDArrayValue<'ctx>;

    fn is_representable(
        llvm_ty: impl BasicType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            Self::has_same_repr(ty, llvm_usize)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn has_same_repr(ty: Self::Base, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        let ctx = ty.get_context();

        let llvm_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ty) = llvm_ty else {
            return Err(format!(
                "Expected struct type for `ContiguousNDArray` type, got {llvm_ty}"
            ));
        };

        let fields = ContiguousNDArrayStructFields::new(ctx, llvm_usize);

        check_struct_type_matches_fields(
            fields,
            llvm_ty,
            "ContiguousNDArray",
            &[(fields.data.name(), &|ty| {
                if ty.is_pointer_type() {
                    Ok(())
                } else {
                    Err(format!("Expected T* for `ContiguousNDArray.data`, got {ty}"))
                }
            })],
        )
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_abi_type().get_element_type().into_struct_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_abi_type(&self) -> Self::ABI {
        self.as_base_type()
    }
}

impl<'ctx> From<ContiguousNDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: ContiguousNDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}
