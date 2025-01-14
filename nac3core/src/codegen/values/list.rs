use inkwell::{
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};

use super::{
    ArrayLikeIndexer, ArrayLikeValue, ProxyValue, UntypedArrayLikeAccessor, UntypedArrayLikeMutator,
};
use crate::codegen::{
    types::{structure::StructField, ListType, ProxyType},
    {CodeGenContext, CodeGenerator},
};

/// Proxy type for accessing a `list` value in LLVM.
#[derive(Copy, Clone)]
pub struct ListValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> ListValue<'ctx> {
    /// Checks whether `value` is an instance of `list`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_representable(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        ListType::is_representable(value.get_type(), llvm_usize)
    }

    /// Creates an [`ListValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr, llvm_usize).is_ok());

        ListValue { value: ptr, llvm_usize, name }
    }

    fn items_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(&ctx.ctx).items
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn pptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.items_field(ctx).ptr_by_gep(ctx, self.value, self.name)
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        self.items_field(ctx).set(ctx, self.value, data, self.name);
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

    fn len_field(&self, ctx: &CodeGenContext<'ctx, '_>) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields(&ctx.ctx).len
    }

    /// Stores the `size` of this `list` into this instance.
    pub fn store_size(&self, ctx: &CodeGenContext<'ctx, '_>, size: IntValue<'ctx>) {
        debug_assert_eq!(size.get_type(), ctx.get_size_type());

        self.len_field(ctx).set(ctx, self.value, size, self.name);
    }

    /// Returns the size of this `list` as a value.
    pub fn load_size(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> IntValue<'ctx> {
        self.len_field(ctx).get(ctx, self.value, name)
    }

    /// Returns an instance of [`ListValue`] with the `items` pointer cast to `i8*`.
    #[must_use]
    pub fn as_i8_list(&self, ctx: &CodeGenContext<'ctx, '_>) -> ListValue<'ctx> {
        let llvm_i8 = ctx.ctx.i8_type();
        let llvm_list_i8 = <Self as ProxyValue>::Type::new(ctx, &llvm_i8);

        Self::from_pointer_value(
            ctx.builder.build_pointer_cast(self.value, llvm_list_i8.as_base_type(), "").unwrap(),
            self.llvm_usize,
            self.name,
        )
    }
}

impl<'ctx> ProxyValue<'ctx> for ListValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = ListType<'ctx>;

    fn get_type(&self) -> Self::Type {
        ListType::from_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<ListValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ListValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for accessing the `data` array of an `list` instance in LLVM.
#[derive(Copy, Clone)]
pub struct ListDataProxy<'ctx, 'a>(&'a ListValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for ListDataProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.value.get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        let var_name = self.0.name.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder
            .build_load(self.0.pptr_to_data(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.0.load_size(ctx, None)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for ListDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name.map(|v| format!("{v}.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(self.base_ptr(ctx, generator), &[*idx], var_name.as_str())
                .unwrap()
        }
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        debug_assert_eq!(idx.get_type(), ctx.get_size_type());

        let size = self.size(ctx, generator);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "list index out of range",
            [None, None, None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) }
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx> for ListDataProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx> for ListDataProxy<'ctx, '_> {}
