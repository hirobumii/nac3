use inkwell::{
    AddressSpace,
    context::ContextRef,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
};

use super::ProxyType;
use crate::codegen::{
    CodeGenContext, CodeGenerator, ModuleContext,
    types::structure::{FieldIndexCounter, StructField, StructFields},
    values::EnumerateValue,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

/// Proxy type for an `enumerate` object in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct EnumerateType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct EnumerateStructIterable<'ctx> {
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub items: StructField<'ctx, PointerValue<'ctx>>,

    #[value_type(usize)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct EnumerateStructFields<'ctx> {
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub iterable: StructField<'ctx, PointerValue<'ctx>>,

    #[value_type(i32_type())]
    pub start: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> EnumerateStructIterable<'ctx> {
    #[must_use]
    pub fn new_typed(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let mut counter = FieldIndexCounter::default();

        EnumerateStructIterable {
            items: StructField::create(
                &mut counter,
                "items",
                ctx.i8_type().ptr_type(AddressSpace::default()),
            ),
            len: StructField::create(&mut counter, "len", llvm_usize),
        }
    }
}
impl<'ctx> EnumerateStructFields<'ctx> {
    #[must_use]
    fn fields(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> EnumerateStructIterable<'ctx> {
        EnumerateStructIterable::new_typed(ctx, llvm_usize)
    }

    #[must_use]
    pub fn new_typed(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let mut counter = FieldIndexCounter::default();
        let iterable = Self::fields(ctx, llvm_usize);
        let iterable_ty = ctx
            .struct_type(
                &iterable.into_iter().map(|field| field.1.as_basic_type_enum()).collect_vec(),
                false,
            )
            .ptr_type(AddressSpace::default());

        EnumerateStructFields {
            iterable: StructField::create(&mut counter, "iterable", iterable_ty),
            start: StructField::create(&mut counter, "start", ctx.i32_type()),
        }
    }
}

impl<'ctx> EnumerateType<'ctx> {
    #[must_use]
    fn fields(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> EnumerateStructFields<'ctx> {
        EnumerateStructFields::new_typed(ctx, llvm_usize)
    }

    #[must_use]
    fn llvm_type(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> PointerType<'ctx> {
        let fields = Self::fields(ctx, llvm_usize);
        let field_types = fields.into_iter().map(|field| field.1).collect_vec();
        let enumerate_struct_ty = ctx.struct_type(&field_types, false);
        enumerate_struct_ty.ptr_type(AddressSpace::default())
    }

    fn new_impl(ctx: ContextRef<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let llvm_enumerate = Self::llvm_type(ctx, llvm_usize);
        EnumerateType { ty: llvm_enumerate, llvm_usize }
    }

    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, ctx.size_t)
    }

    /// Map an existing pointer value to `EnumerateValue`
    #[must_use]
    pub fn map_pointer_value(
        &self,
        value: PointerValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }

    /// Creates an [`EnumerateType`] from a [`PointerType`].
    #[must_use]
    pub fn from_pointer_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::has_same_repr(ptr_ty, llvm_usize).is_ok());

        EnumerateType { ty: ptr_ty, llvm_usize }
    }

    #[must_use]
    pub fn alloca<G: CodeGenerator + ?Sized>(
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
}

impl<'ctx> ProxyType<'ctx> for EnumerateType<'ctx> {
    type ABI = PointerType<'ctx>;
    type Base = PointerType<'ctx>;
    type Value = EnumerateValue<'ctx>;

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

    fn has_same_repr(ty: Self::Base, _llvm_usize: IntType<'ctx>) -> Result<(), String> {
        let llvm_struct_ty = ty.get_element_type();
        let AnyTypeEnum::StructType(sty) = llvm_struct_ty else {
            return Err(format!("Expected struct type for `enumerate`, got {llvm_struct_ty:?}"));
        };

        // expected exactly two fields: pointer, i32
        let fields = sty.get_field_types();
        if fields.len() != 2 {
            return Err(format!("Expected 2 fields for `enumerate` type, got {}", fields.len()));
        }

        // first field must be a pointer (opaque)
        let first = &fields[0];
        match first {
            BasicTypeEnum::PointerType(_) => {}
            other => {
                return Err(format!(
                    "Expected pointer type for first field of `enumerate`, got {other:?}"
                ));
            }
        }

        // second field must be i32
        let second = &fields[1];
        let Ok(second_int) = IntType::try_from(second.as_basic_type_enum()) else {
            return Err(format!("Expected int type for second field (start), got {second:?}"));
        };
        if second_int.get_bit_width() != 32 {
            return Err(format!("Expected 32-bit start type, got {}", second_int.get_bit_width()));
        }

        Ok(())
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

impl<'ctx> From<EnumerateType<'ctx>> for PointerType<'ctx> {
    fn from(value: EnumerateType<'ctx>) -> Self {
        value.as_base_type()
    }
}
