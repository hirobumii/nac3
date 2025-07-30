use inkwell::{
    AddressSpace,
    context::ContextRef,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType, StructType},
    values::{IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use crate::codegen::{
    CoreContext, CodeGenContext, CodeGenerator,
    types::{
        ProxyType,
        structure::{StructField, StructFields, StructProxyType, check_struct_type_matches_fields},
    },
    values::{
        ArrayLikeIndexer, ArraySliceValue,
        ndarray::{NDIndexValue, RustNDIndex},
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDIndexType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct NDIndexStructFields<'ctx> {
    #[value_type(i8_type())]
    pub type_: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

impl<'ctx> NDIndexType<'ctx> {
    #[must_use]
    fn fields(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> NDIndexStructFields<'ctx> {
        NDIndexStructFields::new(ctx, llvm_usize)
    }

    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_ndindex = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_ndindex, llvm_usize }
    }

    #[must_use]
    pub fn new(ctx: &CoreContext<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, ctx.size_t)
    }

    #[must_use]
    pub fn from_struct_type(ty: StructType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        Self::from_pointer_type(ty.ptr_type(AddressSpace::default()), llvm_usize)
    }

    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    /// Allocates an instance of [`NDIndexValue`] as if by calling `alloca` on the base type.
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
            self.llvm_usize,
            name,
        )
    }
    /// Allocates an instance of [`NDIndexValue`] as if by calling `alloca` on the base type.
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
            self.llvm_usize,
            name,
        )
    }

    /// Serialize a list of [`RustNDIndex`] as a newly allocated LLVM array of [`NDIndexValue`].
    #[must_use]
    pub fn construct_ndindices<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        in_ndindices: &[RustNDIndex<'ctx>],
    ) -> ArraySliceValue<'ctx> {
        // Allocate the LLVM ndindices.
        let num_ndindices = self.llvm_usize.const_int(in_ndindices.len() as u64, false);
        let ndindices = self.array_alloca_var(generator, ctx, num_ndindices, None);

        // Initialize all of them.
        for (i, in_ndindex) in in_ndindices.iter().enumerate() {
            let pndindex = unsafe {
                ndindices.ptr_offset_unchecked(
                    ctx,
                    generator,
                    &ctx.i64.const_int(u64::try_from(i).unwrap(), false),
                    None,
                )
            };

            in_ndindex.write_to_ndindex(
                generator,
                ctx,
                NDIndexValue::from_pointer_value(pndindex, self.llvm_usize, None),
            );
        }

        ndindices
    }

    #[must_use]
    pub fn map_struct_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: StructValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_struct_value(
            generator,
            ctx,
            value,
            self.llvm_usize,
            name,
        )
    }

    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for NDIndexType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = NDIndexValue<'ctx>;

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

        let fields = NDIndexStructFields::new(ctx, llvm_usize);

        check_struct_type_matches_fields(fields, llvm_ty, "NDIndex", &[])
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

impl<'ctx> StructProxyType<'ctx> for NDIndexType<'ctx> {
    type StructFields = NDIndexStructFields<'ctx>;

    fn get_fields(&self) -> Self::StructFields {
        Self::fields(self.ty.get_context(), self.llvm_usize)
    }
}

impl<'ctx> From<NDIndexType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDIndexType<'ctx>) -> Self {
        value.as_base_type()
    }
}
