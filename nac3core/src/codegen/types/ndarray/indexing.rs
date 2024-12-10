use inkwell::{
    context::{AsContextRef, Context},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use crate::codegen::{
    types::{
        structure::{check_struct_type_matches_fields, StructField, StructFields},
        ProxyType,
    },
    values::{
        ndarray::{NDIndexValue, RustNDIndex},
        ArrayLikeIndexer, ArraySliceValue, ProxyValue,
    },
    CodeGenContext, CodeGenerator,
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
    /// Checks whether `llvm_ty` represents a `ndindex` type, returning [Err] if it does not.
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

        let fields = NDIndexStructFields::new(ctx, llvm_usize);

        check_struct_type_matches_fields(fields, llvm_ty, "NDIndex", &[])
    }

    #[must_use]
    fn fields(
        ctx: impl AsContextRef<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> NDIndexStructFields<'ctx> {
        NDIndexStructFields::new(ctx, llvm_usize)
    }

    #[must_use]
    pub fn get_fields(&self) -> NDIndexStructFields<'ctx> {
        Self::fields(self.ty.get_context(), self.llvm_usize)
    }

    #[must_use]
    fn llvm_type(ctx: &'ctx Context, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let field_tys =
            Self::fields(ctx, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(generator: &G, ctx: &'ctx Context) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_ndindex = Self::llvm_type(ctx, llvm_usize);

        Self { ty: llvm_ndindex, llvm_usize }
    }

    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        Self { ty: ptr_ty, llvm_usize }
    }

    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca(generator, ctx, name),
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
        let ndindices = self.array_alloca(generator, ctx, num_ndindices, None);

        // Initialize all of them.
        for (i, in_ndindex) in in_ndindices.iter().enumerate() {
            let pndindex = unsafe {
                ndindices.ptr_offset_unchecked(
                    ctx,
                    generator,
                    &ctx.ctx.i64_type().const_int(u64::try_from(i).unwrap(), false),
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
    pub fn map_value(
        &self,
        value: <<Self as ProxyType<'ctx>>::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for NDIndexType<'ctx> {
    type Base = PointerType<'ctx>;
    type Value = NDIndexValue<'ctx>;

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

impl<'ctx> From<NDIndexType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDIndexType<'ctx>) -> Self {
        value.as_base_type()
    }
}
