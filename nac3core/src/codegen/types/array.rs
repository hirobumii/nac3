use anyhow::anyhow;
use inkwell::{
    AddressSpace, IntPredicate,
    types::{BasicType, BasicTypeEnum},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};

use crate::codegen::{
    CodeGenContext, typed_load, typed_store,
    types::{ProxyTypeBase, Value},
};

/// An array-like value that can be indexed by memory offset.
pub trait ArrayLikeIndexer<'ctx, Index = IntValue<'ctx>> {
    /// Returns the type of the items in the array.
    fn item_type(&self) -> BasicTypeEnum<'ctx>;

    /// Returns the pointer to the data at the `idx`-th index.
    fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>>;

    /// Returns the pointer to the data at the `idx`-th index. Raise an error
    /// if the index is out of bounds.
    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>>;

    /// Loads the value at the `idx`-th index without bounds checking.
    fn get_unchecked<V: TryFrom<BasicValueEnum<'ctx>, Error: core::fmt::Debug>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&str>,
    ) -> anyhow::Result<V> {
        let ptr = self.ptr_offset_unchecked(ctx, idx, name)?;
        typed_load(ctx.builder, ptr, self.item_type(), name.unwrap_or_default())?
            .try_into()
            .map_err(|e| anyhow!("{e:?}"))
    }

    /// Loads the value at the `idx`-th index with bounds checking.
    fn get<V: TryFrom<BasicValueEnum<'ctx>, Error: core::fmt::Debug>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&str>,
    ) -> anyhow::Result<V> {
        let ptr = self.ptr_offset(ctx, idx, name)?;
        typed_load(ctx.builder, ptr, self.item_type(), name.unwrap_or_default())?
            .try_into()
            .map_err(|e| anyhow!("{e:?}"))
    }

    /// Stores the `value` at the `idx`-th index without bounds checking.
    fn set_unchecked<V: BasicValue<'ctx>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        value: V,
        name: Option<&str>,
    ) -> anyhow::Result<()> {
        let ptr = self.ptr_offset_unchecked(ctx, idx, name)?;
        typed_store(ctx.builder, ptr, value.as_basic_value_enum())?;
        Ok(())
    }

    /// Stores the `value` at the `idx`-th index with bounds checking.
    fn set<V: BasicValue<'ctx>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        value: V,
        name: Option<&str>,
    ) -> anyhow::Result<()> {
        let ptr = self.ptr_offset(ctx, idx, name)?;
        typed_store(ctx.builder, ptr, value.as_basic_value_enum())?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct ArraySliceType<'ctx> {
    pub item_ty: BasicTypeEnum<'ctx>,
}
impl<'ctx> ProxyTypeBase<'ctx> for ArraySliceType<'ctx> {
    type Value = (PointerValue<'ctx>, IntValue<'ctx>);
}

pub type ArraySliceValue<'ctx> = Value<'ctx, ArraySliceType<'ctx>>;

impl<'ctx> ArraySliceValue<'ctx> {
    /// Creates a new `ArraySliceValue`.
    #[must_use]
    pub const fn new(
        item_ty: BasicTypeEnum<'ctx>,
        ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        Self { ty: ArraySliceType { item_ty }, value: (ptr, len), name }
    }

    /// Copies data from the source pointer into this array slice.
    pub fn memcpy_from(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: PointerValue<'ctx>,
    ) -> anyhow::Result<()> {
        let size = ctx.sizeof(self.ty.item_ty);
        let size = ctx.size_t.const_int(size, false);
        let align = ctx.target.get_target_data().get_abi_alignment(&self.ty.item_ty);
        let bytes = ctx.builder.build_int_mul(self.value.1, size, "")?;
        ctx.builder.build_memcpy(self.value.0, align, src, align, bytes)?;
        Ok(())
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for ArraySliceValue<'ctx> {
    fn item_type(&self) -> BasicTypeEnum<'ctx> {
        self.ty.item_ty
    }

    fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        let var_name = name.or(self.name).map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            let ptr = ctx.builder.build_pointer_cast(
                self.value.0,
                self.ty.item_ty.ptr_type(AddressSpace::default()),
                "",
            )?;
            let r = ctx.builder.build_in_bounds_gep(ptr, &[*idx], var_name.as_str())?;
            Ok(ctx.builder.build_pointer_cast(r, ctx.ptr, name.unwrap_or(""))?)
        }
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        debug_assert_eq!(idx.get_type(), ctx.size_t);

        let size = self.value.1;
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "")?;
        ctx.make_assert(
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for size {1}",
            [Some(*idx), Some(size), None],
            ctx.current_loc,
        )?;

        self.ptr_offset_unchecked(ctx, idx, name)
    }
}
