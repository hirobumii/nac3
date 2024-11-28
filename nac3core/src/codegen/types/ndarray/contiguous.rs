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
        values::{ndarray::ContiguousNDArrayValue, ArraySliceValue, ProxyValue},
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
pub struct ContiguousNDArrayFields<'ctx> {
    #[value_type(usize)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(usize.ptr_type(AddressSpace::default()))]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

impl<'ctx> ContiguousNDArrayFields<'ctx> {
    #[must_use]
    pub fn new_typed(item: BasicTypeEnum<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let mut counter = FieldIndexCounter::default();

        ContiguousNDArrayFields {
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
    /// Checks whether `llvm_ty` represents a `ndarray` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let ctx = llvm_ty.get_context();

        let llvm_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ty) = llvm_ty else {
            return Err(format!(
                "Expected struct type for `ContiguousNDArray` type, got {llvm_ty}"
            ));
        };

        let fields = ContiguousNDArrayFields::new(ctx, llvm_usize);

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

    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(
        item: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> ContiguousNDArrayFields<'ctx> {
        ContiguousNDArrayFields::new_typed(item, llvm_usize)
    }

    /// See [`NDArrayType::fields`].
    // TODO: Move this into e.g. StructProxyType
    #[must_use]
    pub fn get_fields(&self) -> ContiguousNDArrayFields<'ctx> {
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

    /// Creates an instance of [`ContiguousNDArrayType`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        item: BasicTypeEnum<'ctx>,
    ) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_cndarray = Self::llvm_type(ctx, item, llvm_usize);

        Self { ty: llvm_cndarray, item, llvm_usize }
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
        let llvm_usize = generator.get_size_type(ctx.ctx);

        Self { ty: Self::llvm_type(ctx.ctx, llvm_dtype, llvm_usize), item: llvm_dtype, llvm_usize }
    }

    /// Creates an [`ContiguousNDArrayType`] from a [`PointerType`] representing an `NDArray`.
    #[must_use]
    pub fn from_type(
        ptr_ty: PointerType<'ctx>,
        item: BasicTypeEnum<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, item, llvm_usize }
    }

    /// Allocates an instance of [`ContiguousNDArrayValue`] as if by calling `alloca` on the base type.
    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(generator, ctx, name),
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
    type Base = PointerType<'ctx>;
    type Value = ContiguousNDArrayValue<'ctx>;

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

    fn raw_alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self::Value as ProxyValue<'ctx>>::Base {
        generator
            .gen_var_alloc(
                ctx,
                self.as_base_type().get_element_type().into_struct_type().into(),
                name,
            )
            .unwrap()
    }

    fn array_alloca<G: CodeGenerator + ?Sized>(
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

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<ContiguousNDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: ContiguousNDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}
