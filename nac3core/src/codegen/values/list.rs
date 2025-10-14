use inkwell::{
    AddressSpace, IntPredicate,
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue, StructValue},
};

use super::{
    ArrayLikeIndexer, ArrayLikeValue, ProxyValue, UntypedArrayLikeAccessor,
    UntypedArrayLikeMutator, structure::StructProxyValue,
};
use crate::codegen::{
    CodeGenContext,
    stmt::gen_var,
    types::{
        ListType, ProxyType,
        structure::{StructField, StructProxyType},
    },
};

/// Proxy type for accessing a `list` value in LLVM.
#[derive(Copy, Clone)]
pub struct ListValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> ListValue<'ctx> {
    /// Creates an [`ListValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_struct_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval =
            gen_var(ctx, val.get_type().into(), name.map(|name| format!("{name}.addr")).as_deref())
                .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, llvm_usize, name)
    }

    /// Creates an [`ListValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        ListValue { value: ptr, llvm_usize, name }
    }

    fn items_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().items
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &mut CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        self.items_field().store(ctx, self.value, data, self.name);
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    ///
    /// If `size` is [None], the size stored in the field of this instance is used instead. If
    /// `size` is resolved to `0` at runtime, `(T*) 0` will be assigned to `data`.
    pub fn create_data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        elem_ty: BasicTypeEnum<'ctx>,
        size: Option<IntValue<'ctx>>,
    ) {
        let size = size.unwrap_or_else(|| self.load_size(ctx, None));

        let data = ctx
            .builder
            .build_select(
                ctx.builder
                    .build_int_compare(IntPredicate::NE, size, self.llvm_usize.const_zero(), "")
                    .unwrap(),
                ctx.builder.build_array_alloca(elem_ty, size, "").unwrap(),
                elem_ty.ptr_type(AddressSpace::default()).const_zero(),
                "",
            )
            .map(BasicValueEnum::into_pointer_value)
            .unwrap();
        self.store_data(ctx, data);
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    #[must_use]
    pub fn data(&self) -> ListDataProxy<'ctx, '_> {
        ListDataProxy(self)
    }

    fn len_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().len
    }

    /// Stores the `size` of this `list` into this instance.
    pub fn store_size(&self, ctx: &mut CodeGenContext<'ctx, '_>, size: IntValue<'ctx>) {
        debug_assert_eq!(size.get_type(), ctx.size_t);

        self.len_field().store(ctx, self.value, size, self.name);
    }

    /// Returns the size of this `list` as a value.
    pub fn load_size(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> IntValue<'ctx> {
        self.len_field().load(ctx, self.value, name)
    }

    /// Returns an instance of [`ListValue`] with the `items` pointer cast to `i8*`.
    #[must_use]
    pub fn as_i8_list(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ListValue<'ctx> {
        let llvm_list_i8 = <Self as ProxyValue>::Type::new(ctx, &ctx.i8);

        Self::from_pointer_value(
            ctx.builder.build_pointer_cast(self.value, llvm_list_i8.as_abi_type(), "").unwrap(),
            self.llvm_usize,
            self.name,
        )
    }
}

impl<'ctx> ProxyValue<'ctx> for ListValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = ListType<'ctx>;

    fn get_type(&self) -> Self::Type {
        ListType::from_pointer_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for ListValue<'ctx> {}

impl<'ctx> From<ListValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ListValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for accessing the `data` array of an `list` instance in LLVM.
#[derive(Copy, Clone)]
pub struct ListDataProxy<'ctx, 'a>(&'a ListValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for ListDataProxy<'ctx, '_> {
    fn element_type(&self, _: &CodeGenContext<'ctx, '_>) -> AnyTypeEnum<'ctx> {
        self.0.value.get_type().get_element_type()
    }

    fn base_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.0.items_field().load(ctx, self.0.value, self.0.name)
    }

    fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.0.load_size(ctx, None)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for ListDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name.map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(self.base_ptr(ctx), &[*idx], var_name.as_str()).unwrap()
        }
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        debug_assert_eq!(idx.get_type(), ctx.size_t);

        let size = self.size(ctx);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        ctx.make_assert(
            in_range,
            "0:IndexError",
            "list index out of range",
            [None, None, None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, idx, name) }
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx> for ListDataProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx> for ListDataProxy<'ctx, '_> {}
