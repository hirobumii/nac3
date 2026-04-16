use std::marker::PhantomData;

use anyhow::anyhow;
use inkwell::{
    IntPredicate,
    types::BasicTypeEnum,
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    types::{ProxyType, ProxyTypeBase, Value},
};

/// An array-like value that can be indexed by memory offset.
pub trait ArrayLikeIndexer<'ctx, Index = IntValue<'ctx>> {
    /// Returns the type of the items in the array.
    fn item_type(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx>;

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
        ctx.builder
            .build_load(self.item_type(ctx), ptr, name.unwrap_or_default())?
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
        ctx.builder
            .build_load(self.item_type(ctx), ptr, name.unwrap_or_default())?
            .try_into()
            .map_err(|e| anyhow!("{e:?}"))
    }

    /// Loads the value at the `idx`-th index without bounds checking, converting it into a typed [`Value`].
    fn typed_get_unchecked<V: TryFrom<BasicValueEnum<'ctx>, Error: core::fmt::Debug>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<super::Value<'ctx, Self>>
    where
        Self: ProxyTypeBase<'ctx, Value = V> + Copy,
    {
        Ok(self.map_value(self.get_unchecked(ctx, idx, name)?, name))
    }

    /// Loads the value at the `idx`-th index with bounds checking, converting it into a typed [`Value`].
    fn typed_get<V: TryFrom<BasicValueEnum<'ctx>, Error: core::fmt::Debug>>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<super::Value<'ctx, Self>>
    where
        Self: ProxyTypeBase<'ctx, Value = V> + Copy,
    {
        Ok(self.map_value(self.get(ctx, idx, name)?, name))
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
        ctx.builder.build_store(ptr, value)?;
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
        ctx.builder.build_store(ptr, value)?;
        Ok(())
    }

    /// Stores the [`Value`] at the `idx`-th index without bounds checking.
    fn typed_set_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        value: super::Value<'ctx, Self>,
        name: Option<&str>,
    ) -> anyhow::Result<()>
    where
        Self: ProxyTypeBase<'ctx, Value: BasicValue<'ctx>> + Copy,
    {
        self.set_unchecked(ctx, idx, value.value, name)
    }

    /// Stores the [`Value`] at the `idx`-th index with bounds checking.
    fn typed_set(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &Index,
        value: super::Value<'ctx, Self>,
        name: Option<&str>,
    ) -> anyhow::Result<()>
    where
        Self: ProxyTypeBase<'ctx, Value: BasicValue<'ctx>> + Copy,
    {
        self.set(ctx, idx, value.value, name)
    }
}

#[derive(Clone, Copy)]
pub struct ArraySliceType<'ctx, T: ProxyType<'ctx> + Copy = BasicTypeEnum<'ctx>> {
    pub item_ty: T,
    _data: PhantomData<&'ctx ()>,
}

impl<'ctx, T: ProxyType<'ctx> + Copy> ProxyTypeBase<'ctx> for ArraySliceType<'ctx, T> {
    type Value = (PointerValue<'ctx>, IntValue<'ctx>);
}

pub type ArraySliceValue<'ctx, T = BasicTypeEnum<'ctx>> = Value<'ctx, ArraySliceType<'ctx, T>>;

impl<'ctx, T: ProxyType<'ctx> + Copy> ArraySliceValue<'ctx, T> {
    /// Creates a new `ArraySliceValue`.
    #[must_use]
    pub const fn new(
        item_ty: T,
        ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        Self { ty: ArraySliceType { item_ty, _data: PhantomData }, value: (ptr, len), name }
    }

    /// Copies data from the source pointer into this array slice.
    pub fn memcpy_from(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: PointerValue<'ctx>,
    ) -> anyhow::Result<()> {
        let size = ctx.sizeof(self.ty.item_ty.llvm_ty(ctx));
        let size = ctx.size_t.const_int(size, false);
        let align = ctx.target.get_target_data().get_abi_alignment(&self.ty.item_ty.llvm_ty(ctx));
        let bytes = ctx.builder.build_int_mul(self.value.1, size, "")?;
        ctx.builder.build_memcpy(self.value.0, align, src, align, bytes)?;
        Ok(())
    }

    pub fn cast<U: ProxyType<'ctx> + Copy>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        target_type: U,
        new_size: Option<IntValue<'ctx>>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, ArraySliceType<'ctx, U>>> {
        let new_size = if let Some(new_size) = new_size {
            new_size
        } else {
            let bytes = ctx.builder.build_int_mul(
                self.value.1,
                ctx.size_t.const_int(ctx.sizeof(self.ty.item_ty.llvm_ty(ctx)), false),
                "",
            )?;
            ctx.builder.build_int_unsigned_div(
                bytes,
                ctx.size_t.const_int(ctx.sizeof(target_type.llvm_ty(ctx)), false),
                "",
            )?
        };

        Ok(ArraySliceType { item_ty: target_type, _data: PhantomData }
            .map_value((self.value.0, new_size), name))
    }

    pub fn const_cast<U: ProxyType<'ctx> + Copy>(
        &self,
        ctx: &ModuleContext<'ctx>,
        target_type: U,
        new_size: Option<IntValue<'ctx>>,
        name: Option<&'ctx str>,
    ) -> Value<'ctx, ArraySliceType<'ctx, U>> {
        assert!(
            self.value.1.is_constant_int(),
            "const_cast can only be used on array slices with compile-time constant size"
        );

        let new_size = new_size.unwrap_or_else(|| {
            let size = self.value.1.get_zero_extended_constant().unwrap();
            let bytes = size * ctx.sizeof(self.ty.item_ty.llvm_ty(ctx));
            let new_item_size = ctx.sizeof(target_type.llvm_ty(ctx));
            ctx.size_t.const_int(bytes / new_item_size, false)
        });

        ArraySliceType { item_ty: target_type, _data: PhantomData }
            .map_value((self.value.0, new_size), name)
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> ArrayLikeIndexer<'ctx> for ArraySliceValue<'ctx, T> {
    fn item_type(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.ty.item_ty.llvm_ty(ctx)
    }

    fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        let var_name = name.or(self.name).map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            Ok(ctx.builder.build_in_bounds_gep(
                self.ty.item_ty.llvm_ty(ctx),
                self.value.0,
                &[*idx],
                var_name.as_str(),
            )?)
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
