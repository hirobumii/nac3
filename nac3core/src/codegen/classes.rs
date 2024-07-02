use crate::codegen::{
    irrt::{call_ndarray_calc_size, call_ndarray_flatten_index},
    llvm_intrinsics::call_int_umin,
    stmt::gen_for_callback_incrementing,
    CodeGenContext, CodeGenerator,
};
use inkwell::context::Context;
use inkwell::types::{ArrayType, BasicType, StructType};
use inkwell::values::{ArrayValue, BasicValue, StructValue};
use inkwell::{
    types::{AnyTypeEnum, BasicTypeEnum, IntType, PointerType},
    values::{BasicValueEnum, IntValue, PointerValue},
    AddressSpace, IntPredicate,
};

/// A LLVM type that is used to represent a non-primitive type in NAC3.
pub trait ProxyType<'ctx>: Into<Self::Base> {
    /// The LLVM type of which values of this type possess. This is usually a
    /// [LLVM pointer type][PointerType].
    type Base: BasicType<'ctx>;

    /// The underlying LLVM type used to represent values. This is usually the element type of
    /// [`Base`] if it is a pointer, otherwise this is the same type as `Base`.
    type Underlying: BasicType<'ctx>;

    /// The type of values represented by this type.
    type Value: ProxyValue<'ctx>;

    /// Creates a new value of this type.
    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value;

    /// Creates a new array value of this type.
    fn new_array_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> ArraySliceValue<'ctx> {
        generator
            .gen_array_var_alloc(ctx, self.as_underlying_type().as_basic_type_enum(), size, name)
            .unwrap()
    }

    /// Creates a [`value`][ProxyValue] with this as its type.
    fn create_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value;

    /// Returns the [base type][Self::Base] of this proxy.
    fn as_base_type(&self) -> Self::Base;

    /// Returns the [underlying type][Self::Underlying] of this proxy.
    fn as_underlying_type(&self) -> Self::Underlying;
}

/// A LLVM type that is used to represent a non-primitive value in NAC3.
pub trait ProxyValue<'ctx>: Into<Self::Base> {
    /// The type of LLVM values represented by this instance. This is usually the
    /// [LLVM pointer type][PointerValue].
    type Base: BasicValue<'ctx>;

    /// The underlying type of LLVM values represented by this instance. This is usually the element
    /// type of [`Base`] if it is a pointer, otherwise this is the same type as `Base`.
    type Underlying: BasicValue<'ctx>;

    /// The type of this value.
    type Type: ProxyType<'ctx>;

    /// Returns the [type][ProxyType] of this value.
    fn get_type(&self) -> Self::Type;

    /// Returns the [base value][Self::Base] of this proxy.
    fn as_base_value(&self) -> Self::Base;

    /// Loads this value into its [underlying representation][Self::Underlying]. Usually involves a
    /// `getelementptr` if [`Self::Base`] is a [pointer value][PointerValue].
    fn as_underlying_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Underlying;
}

/// An LLVM value that is array-like, i.e. it contains a contiguous, sequenced collection of
/// elements.
pub trait ArrayLikeValue<'ctx> {
    /// Returns the element type of this array-like value.
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx>;

    /// Returns the base pointer to the array.
    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> PointerValue<'ctx>;

    /// Returns the size of this array-like value.
    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> IntValue<'ctx>;

    /// Returns a [`ArraySliceValue`] representing this value.
    fn as_slice_value<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> ArraySliceValue<'ctx> {
        ArraySliceValue::from_ptr_val(
            self.base_ptr(ctx, generator),
            self.size(ctx, generator),
            None,
        )
    }
}

/// An array-like value that can be indexed by memory offset.
pub trait ArrayLikeIndexer<'ctx, Index = IntValue<'ctx>>: ArrayLikeValue<'ctx> {
    /// # Safety
    ///
    /// This function should be called with a valid index.
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx>;

    /// Returns the pointer to the data at the `idx`-th index.
    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx>;
}

/// An array-like value that can have its array elements accessed as a [`BasicValueEnum`].
pub trait UntypedArrayLikeAccessor<'ctx, Index = IntValue<'ctx>>:
    ArrayLikeIndexer<'ctx, Index>
{
    /// # Safety
    ///
    /// This function should be called with a valid index.
    unsafe fn get_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) };
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// Returns the data at the `idx`-th index.
    fn get<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset(ctx, generator, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }
}

/// An array-like value that can have its array elements mutated as a [`BasicValueEnum`].
pub trait UntypedArrayLikeMutator<'ctx, Index = IntValue<'ctx>>:
    ArrayLikeIndexer<'ctx, Index>
{
    /// # Safety
    ///
    /// This function should be called with a valid index.
    unsafe fn set_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        value: BasicValueEnum<'ctx>,
    ) {
        let ptr = unsafe { self.ptr_offset_unchecked(ctx, generator, idx, None) };
        ctx.builder.build_store(ptr, value).unwrap();
    }

    /// Sets the data at the `idx`-th index.
    fn set<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        value: BasicValueEnum<'ctx>,
    ) {
        let ptr = self.ptr_offset(ctx, generator, idx, None);
        ctx.builder.build_store(ptr, value).unwrap();
    }
}

/// An array-like value that can have its array elements accessed as an arbitrary type `T`.
pub trait TypedArrayLikeAccessor<'ctx, T, Index = IntValue<'ctx>>:
    UntypedArrayLikeAccessor<'ctx, Index>
{
    /// Casts an element from [`BasicValueEnum`] into `T`.
    fn downcast_to_type(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> T;

    /// # Safety
    ///
    /// This function should be called with a valid index.
    unsafe fn get_typed_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> T {
        let value = unsafe { self.get_unchecked(ctx, generator, idx, name) };
        self.downcast_to_type(ctx, value)
    }

    /// Returns the data at the `idx`-th index.
    fn get_typed<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> T {
        let value = self.get(ctx, generator, idx, name);
        self.downcast_to_type(ctx, value)
    }
}

/// An array-like value that can have its array elements mutated as an arbitrary type `T`.
pub trait TypedArrayLikeMutator<'ctx, T, Index = IntValue<'ctx>>:
    UntypedArrayLikeMutator<'ctx, Index>
{
    /// Casts an element from T into [`BasicValueEnum`].
    fn upcast_from_type(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: T,
    ) -> BasicValueEnum<'ctx>;

    /// # Safety
    ///
    /// This function should be called with a valid index.
    unsafe fn set_typed_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        value: T,
    ) {
        let value = self.upcast_from_type(ctx, value);
        unsafe { self.set_unchecked(ctx, generator, idx, value) }
    }

    /// Sets the data at the `idx`-th index.
    fn set_typed<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        value: T,
    ) {
        let value = self.upcast_from_type(ctx, value);
        self.set(ctx, generator, idx, value);
    }
}

/// Type alias for a function that casts a [`BasicValueEnum`] into a `T`.
type ValueDowncastFn<'ctx, T> =
    Box<dyn Fn(&mut CodeGenContext<'ctx, '_>, BasicValueEnum<'ctx>) -> T>;
/// Type alias for a function that casts a `T` into a [`BasicValueEnum`].
type ValueUpcastFn<'ctx, T> = Box<dyn Fn(&mut CodeGenContext<'ctx, '_>, T) -> BasicValueEnum<'ctx>>;

/// An adapter for constraining untyped array values as typed values.
pub struct TypedArrayLikeAdapter<'ctx, T, Adapted: ArrayLikeValue<'ctx> = ArraySliceValue<'ctx>> {
    adapted: Adapted,
    downcast_fn: ValueDowncastFn<'ctx, T>,
    upcast_fn: ValueUpcastFn<'ctx, T>,
}

impl<'ctx, T, Adapted> TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: ArrayLikeValue<'ctx>,
{
    /// Creates a [`TypedArrayLikeAdapter`].
    ///
    /// * `adapted` - The value to be adapted.
    /// * `downcast_fn` - The function converting a [`BasicValueEnum`] into a `T`.
    /// * `upcast_fn` - The function converting a T into a [`BasicValueEnum`].
    pub fn from(
        adapted: Adapted,
        downcast_fn: ValueDowncastFn<'ctx, T>,
        upcast_fn: ValueUpcastFn<'ctx, T>,
    ) -> Self {
        TypedArrayLikeAdapter { adapted, downcast_fn, upcast_fn }
    }
}

impl<'ctx, T, Adapted> ArrayLikeValue<'ctx> for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: ArrayLikeValue<'ctx>,
{
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.adapted.element_type(ctx, generator)
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> PointerValue<'ctx> {
        self.adapted.base_ptr(ctx, generator)
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> IntValue<'ctx> {
        self.adapted.size(ctx, generator)
    }
}

impl<'ctx, T, Index, Adapted> ArrayLikeIndexer<'ctx, Index>
    for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: ArrayLikeIndexer<'ctx, Index>,
{
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        unsafe { self.adapted.ptr_offset_unchecked(ctx, generator, idx, name) }
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        self.adapted.ptr_offset(ctx, generator, idx, name)
    }
}

impl<'ctx, T, Index, Adapted> UntypedArrayLikeAccessor<'ctx, Index>
    for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: UntypedArrayLikeAccessor<'ctx, Index>,
{
}
impl<'ctx, T, Index, Adapted> UntypedArrayLikeMutator<'ctx, Index>
    for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: UntypedArrayLikeMutator<'ctx, Index>,
{
}

impl<'ctx, T, Index, Adapted> TypedArrayLikeAccessor<'ctx, T, Index>
    for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: UntypedArrayLikeAccessor<'ctx, Index>,
{
    fn downcast_to_type(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> T {
        (self.downcast_fn)(ctx, value)
    }
}

impl<'ctx, T, Index, Adapted> TypedArrayLikeMutator<'ctx, T, Index>
    for TypedArrayLikeAdapter<'ctx, T, Adapted>
where
    Adapted: UntypedArrayLikeMutator<'ctx, Index>,
{
    fn upcast_from_type(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: T,
    ) -> BasicValueEnum<'ctx> {
        (self.upcast_fn)(ctx, value)
    }
}

/// An LLVM value representing an array slice, consisting of a pointer to the data and the size of
/// the slice.
#[derive(Copy, Clone)]
pub struct ArraySliceValue<'ctx>(PointerValue<'ctx>, IntValue<'ctx>, Option<&'ctx str>);

impl<'ctx> ArraySliceValue<'ctx> {
    /// Creates an [`ArraySliceValue`] from a [`PointerValue`] and its size.
    #[must_use]
    pub fn from_ptr_val(
        ptr: PointerValue<'ctx>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        ArraySliceValue(ptr, size, name)
    }
}

impl<'ctx> From<ArraySliceValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ArraySliceValue<'ctx>) -> Self {
        value.0
    }
}

impl<'ctx> ArrayLikeValue<'ctx> for ArraySliceValue<'ctx> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        self.0
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        _: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.1
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for ArraySliceValue<'ctx> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
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
        debug_assert_eq!(idx.get_type(), generator.get_size_type(ctx.ctx));

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

impl<'ctx> UntypedArrayLikeAccessor<'ctx> for ArraySliceValue<'ctx> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx> for ArraySliceValue<'ctx> {}

/// Proxy type for a `list` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ListType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> ListType<'ctx> {
    /// Checks whether `llvm_ty` represents a `list` type, returning [Err] if it does not.
    pub fn is_type(llvm_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        let llvm_list_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_list_ty) = llvm_list_ty else {
            return Err(format!("Expected struct type for `list` type, got {llvm_list_ty}"));
        };
        if llvm_list_ty.count_fields() != 2 {
            return Err(format!(
                "Expected 2 fields in `list`, got {}",
                llvm_list_ty.count_fields()
            ));
        }

        let list_size_ty = llvm_list_ty.get_field_type_at_index(0).unwrap();
        let Ok(_) = PointerType::try_from(list_size_ty) else {
            return Err(format!("Expected pointer type for `list.0`, got {list_size_ty}"));
        };

        let list_data_ty = llvm_list_ty.get_field_type_at_index(1).unwrap();
        let Ok(list_data_ty) = IntType::try_from(list_data_ty) else {
            return Err(format!("Expected int type for `list.1`, got {list_data_ty}"));
        };
        if list_data_ty.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!(
                "Expected {}-bit int type for `list.1`, got {}-bit int",
                llvm_usize.get_bit_width(),
                list_data_ty.get_bit_width()
            ));
        }

        Ok(())
    }

    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        element_type: BasicTypeEnum<'ctx>,
    ) -> Self {
        let llvm_usize = generator.get_size_type(ctx);
        let llvm_list = ctx
            .struct_type(
                &[element_type.ptr_type(AddressSpace::default()).into(), llvm_usize.into()],
                false,
            )
            .ptr_type(AddressSpace::default());

        ListType::from_type(llvm_list, llvm_usize)
    }

    /// Creates an [`ListType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_type(ptr_ty, llvm_usize).is_ok());

        ListType { ty: ptr_ty, llvm_usize }
    }

    /// Returns the type of the `size` field of this `list` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(1)
            .map(BasicTypeEnum::into_int_type)
            .unwrap()
    }

    /// Returns the element type of this `list` type.
    #[must_use]
    pub fn element_type(&self) -> AnyTypeEnum<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(0)
            .map(BasicTypeEnum::into_pointer_type)
            .map(PointerType::get_element_type)
            .unwrap()
    }
}

impl<'ctx> ProxyType<'ctx> for ListType<'ctx> {
    type Base = PointerType<'ctx>;
    type Underlying = StructType<'ctx>;
    type Value = ListValue<'ctx>;

    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        self.create_value(
            generator.gen_var_alloc(ctx, self.as_underlying_type().into(), name).unwrap(),
            name,
        )
    }

    fn create_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        debug_assert_eq!(value.get_type(), self.as_base_type());

        ListValue { value, llvm_usize: self.llvm_usize, name }
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_underlying_type(&self) -> Self::Underlying {
        self.as_base_type().get_element_type().into_struct_type()
    }
}

impl<'ctx> From<ListType<'ctx>> for PointerType<'ctx> {
    fn from(value: ListType<'ctx>) -> Self {
        value.as_base_type()
    }
}

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
    pub fn is_instance(value: PointerValue<'ctx>, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        ListType::is_type(value.get_type(), llvm_usize)
    }

    /// Creates an [`ListValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        <Self as ProxyValue<'ctx>>::Type::from_type(ptr.get_type(), llvm_usize)
            .create_value(ptr, name)
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn pptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Returns the pointer to the field storing the size of this `list`.
    fn ptr_to_size(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.size.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        ctx.builder.build_store(self.pptr_to_data(ctx), data).unwrap();
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    ///
    /// If `size` is [None], the size stored in the field of this instance is used instead.
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

    /// Stores the `size` of this `list` into this instance.
    pub fn store_size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        size: IntValue<'ctx>,
    ) {
        debug_assert_eq!(size.get_type(), generator.get_size_type(ctx.ctx));

        let psize = self.ptr_to_size(ctx);
        ctx.builder.build_store(psize, size).unwrap();
    }

    /// Returns the size of this `list` as a value.
    pub fn load_size(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let psize = self.ptr_to_size(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.size")))
            .unwrap_or_default();

        ctx.builder
            .build_load(psize, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for ListValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Underlying = StructValue<'ctx>;
    type Type = ListType<'ctx>;

    fn get_type(&self) -> Self::Type {
        ListType::from_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_underlying_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Underlying {
        ctx.builder
            .build_load(self.as_base_value(), name.unwrap_or_default())
            .map(BasicValueEnum::into_struct_value)
            .unwrap()
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
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
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
        debug_assert_eq!(idx.get_type(), generator.get_size_type(ctx.ctx));

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

/// Proxy type for a `range` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RangeType<'ctx> {
    ty: PointerType<'ctx>,
}

impl<'ctx> RangeType<'ctx> {
    /// Checks whether `llvm_ty` represents a `range` type, returning [Err] if it does not.
    pub fn is_type(llvm_ty: PointerType<'ctx>) -> Result<(), String> {
        let llvm_range_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::ArrayType(llvm_range_ty) = llvm_range_ty else {
            return Err(format!("Expected array type for `range` type, got {llvm_range_ty}"));
        };
        if llvm_range_ty.len() != 3 {
            return Err(format!(
                "Expected 3 elements for `range` type, got {}",
                llvm_range_ty.len()
            ));
        }

        let llvm_range_elem_ty = llvm_range_ty.get_element_type();
        let Ok(llvm_range_elem_ty) = IntType::try_from(llvm_range_elem_ty) else {
            return Err(format!(
                "Expected int type for `range` element type, got {llvm_range_elem_ty}"
            ));
        };
        if llvm_range_elem_ty.get_bit_width() != 32 {
            return Err(format!(
                "Expected 32-bit int type for `range` element type, got {}",
                llvm_range_elem_ty.get_bit_width()
            ));
        }

        Ok(())
    }

    /// Creates an instance of [`RangeType`].
    #[must_use]
    pub fn new(ctx: &'ctx Context) -> Self {
        let llvm_i32 = ctx.i32_type();
        let llvm_range = llvm_i32.array_type(3).ptr_type(AddressSpace::default());

        RangeType::from_type(llvm_range)
    }

    /// Creates an [`RangeType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>) -> Self {
        debug_assert!(Self::is_type(ptr_ty).is_ok());

        RangeType { ty: ptr_ty }
    }

    /// Returns the type of all fields of this `range` type.
    #[must_use]
    pub fn value_type(&self) -> IntType<'ctx> {
        self.as_base_type().get_element_type().into_array_type().get_element_type().into_int_type()
    }
}

impl<'ctx> ProxyType<'ctx> for RangeType<'ctx> {
    type Base = PointerType<'ctx>;
    type Underlying = ArrayType<'ctx>;
    type Value = RangeValue<'ctx>;

    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        self.create_value(
            generator.gen_var_alloc(ctx, self.as_underlying_type().into(), name).unwrap(),
            name,
        )
    }

    fn create_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        debug_assert_eq!(value.get_type(), self.as_base_type());

        RangeValue { value, name }
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_underlying_type(&self) -> Self::Underlying {
        self.as_base_type().get_element_type().into_array_type()
    }
}

impl<'ctx> From<RangeType<'ctx>> for PointerType<'ctx> {
    fn from(value: RangeType<'ctx>) -> Self {
        value.as_base_type()
    }
}

/// Proxy type for accessing a `range` value in LLVM.
#[derive(Copy, Clone)]
pub struct RangeValue<'ctx> {
    value: PointerValue<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> RangeValue<'ctx> {
    /// Checks whether `value` is an instance of `range`, returning [Err] if `value` is not an instance.
    pub fn is_instance(value: PointerValue<'ctx>) -> Result<(), String> {
        RangeType::is_type(value.get_type())
    }

    /// Creates an [`RangeValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(ptr: PointerValue<'ctx>, name: Option<&'ctx str>) -> Self {
        debug_assert!(Self::is_instance(ptr).is_ok());

        <Self as ProxyValue<'ctx>>::Type::from_type(ptr.get_type()).create_value(ptr, name)
    }

    fn ptr_to_start(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.start.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(0, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_end(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.end.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    fn ptr_to_step(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.step.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(2, false)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the `start` value into this instance.
    pub fn store_start(&self, ctx: &CodeGenContext<'ctx, '_>, start: IntValue<'ctx>) {
        debug_assert_eq!(start.get_type().get_bit_width(), 32);

        let pstart = self.ptr_to_start(ctx);
        ctx.builder.build_store(pstart, start).unwrap();
    }

    /// Returns the `start` value of this `range`.
    pub fn load_start(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pstart = self.ptr_to_start(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.start")))
            .unwrap_or_default();

        ctx.builder
            .build_load(pstart, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Stores the `end` value into this instance.
    pub fn store_end(&self, ctx: &CodeGenContext<'ctx, '_>, end: IntValue<'ctx>) {
        debug_assert_eq!(end.get_type().get_bit_width(), 32);

        let pend = self.ptr_to_end(ctx);
        ctx.builder.build_store(pend, end).unwrap();
    }

    /// Returns the `end` value of this `range`.
    pub fn load_end(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pend = self.ptr_to_end(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.end")))
            .unwrap_or_default();

        ctx.builder.build_load(pend, var_name.as_str()).map(BasicValueEnum::into_int_value).unwrap()
    }

    /// Stores the `step` value into this instance.
    pub fn store_step(&self, ctx: &CodeGenContext<'ctx, '_>, step: IntValue<'ctx>) {
        debug_assert_eq!(step.get_type().get_bit_width(), 32);

        let pstep = self.ptr_to_step(ctx);
        ctx.builder.build_store(pstep, step).unwrap();
    }

    /// Returns the `step` value of this `range`.
    pub fn load_step(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pstep = self.ptr_to_step(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.name.map(|v| format!("{v}.step")))
            .unwrap_or_default();

        ctx.builder
            .build_load(pstep, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

impl<'ctx> ProxyValue<'ctx> for RangeValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Underlying = ArrayValue<'ctx>;
    type Type = RangeType<'ctx>;

    fn get_type(&self) -> Self::Type {
        RangeType::from_type(self.value.get_type())
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_underlying_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Underlying {
        ctx.builder
            .build_load(self.as_base_value(), name.unwrap_or_default())
            .map(BasicValueEnum::into_array_value)
            .unwrap()
    }
}

impl<'ctx> From<RangeValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: RangeValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for a `ndarray` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct NDArrayType<'ctx> {
    ty: PointerType<'ctx>,
    llvm_usize: IntType<'ctx>,
}

impl<'ctx> NDArrayType<'ctx> {
    /// Checks whether `llvm_ty` represents a `ndarray` type, returning [Err] if it does not.
    pub fn is_type(llvm_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Result<(), String> {
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
        debug_assert!(Self::is_type(ptr_ty, llvm_usize).is_ok());

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
    pub fn element_type(&self) -> BasicTypeEnum<'ctx> {
        self.as_base_type()
            .get_element_type()
            .into_struct_type()
            .get_field_type_at_index(2)
            .unwrap()
    }
}

impl<'ctx> ProxyType<'ctx> for NDArrayType<'ctx> {
    type Base = PointerType<'ctx>;
    type Underlying = StructType<'ctx>;
    type Value = NDArrayValue<'ctx>;

    fn new_value<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        self.create_value(
            generator.gen_var_alloc(ctx, self.as_underlying_type().into(), name).unwrap(),
            name,
        )
    }

    fn create_value(
        &self,
        value: <Self::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> Self::Value {
        debug_assert_eq!(value.get_type(), self.as_base_type());

        NDArrayValue { value, llvm_usize: self.llvm_usize, name }
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }

    fn as_underlying_type(&self) -> Self::Underlying {
        self.as_base_type().get_element_type().into_struct_type()
    }
}

impl<'ctx> From<NDArrayType<'ctx>> for PointerType<'ctx> {
    fn from(value: NDArrayType<'ctx>) -> Self {
        value.as_base_type()
    }
}

/// Proxy type for accessing an `NDArray` value in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Checks whether `value` is an instance of `NDArray`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_instance(value: PointerValue<'ctx>, llvm_usize: IntType<'ctx>) -> Result<(), String> {
        NDArrayType::is_type(value.get_type(), llvm_usize)
    }

    /// Creates an [`NDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        <Self as ProxyValue<'ctx>>::Type::from_type(ptr.get_type(), llvm_usize)
            .create_value(ptr, name)
    }

    /// Returns the pointer to the field storing the number of dimensions of this `NDArray`.
    fn ptr_to_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.ndims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the number of dimensions `ndims` into this instance.
    pub fn store_ndims<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
        ndims: IntValue<'ctx>,
    ) {
        debug_assert_eq!(ndims.get_type(), generator.get_size_type(ctx.ctx));

        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_store(pndims, ndims).unwrap();
    }

    /// Returns the number of dimensions of this `NDArray` as a value.
    pub fn load_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_load(pndims, "").map(BasicValueEnum::into_int_value).unwrap()
    }

    /// Returns the double-indirection pointer to the `dims` array, as if by calling `getelementptr`
    /// on the field.
    fn ptr_to_dims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.dims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the array of dimension sizes `dims` into this instance.
    fn store_dim_sizes(&self, ctx: &CodeGenContext<'ctx, '_>, dims: PointerValue<'ctx>) {
        ctx.builder.build_store(self.ptr_to_dims(ctx), dims).unwrap();
    }

    /// Convenience method for creating a new array storing dimension sizes with the given `size`.
    pub fn create_dim_sizes(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        llvm_usize: IntType<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_dim_sizes(ctx, ctx.builder.build_array_alloca(llvm_usize, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the size of each dimension of this `NDArray`.
    #[must_use]
    pub fn dim_sizes(&self) -> NDArrayDimsProxy<'ctx, '_> {
        NDArrayDimsProxy(self)
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn ptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.name.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.as_base_value(),
                    &[llvm_i32.const_zero(), llvm_i32.const_int(2, true)],
                    var_name.as_str(),
                )
                .unwrap()
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        ctx.builder.build_store(self.ptr_to_data(ctx), data).unwrap();
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    pub fn create_data(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        elem_ty: BasicTypeEnum<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_data(ctx, ctx.builder.build_array_alloca(elem_ty, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the data of this `NDArray`.
    #[must_use]
    pub fn data(&self) -> NDArrayDataProxy<'ctx, '_> {
        NDArrayDataProxy(self)
    }
}

impl<'ctx> ProxyValue<'ctx> for NDArrayValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Underlying = StructValue<'ctx>;
    type Type = NDArrayType<'ctx>;

    fn get_type(&self) -> Self::Type {
        NDArrayType::from_type(self.as_base_value().get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_underlying_value(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> Self::Underlying {
        ctx.builder
            .build_load(self.as_base_value(), name.unwrap_or_default())
            .map(BasicValueEnum::into_struct_value)
            .unwrap()
    }
}

impl<'ctx> From<NDArrayValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDArrayValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

/// Proxy type for accessing the `dims` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDimsProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayDimsProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.dim_sizes().base_ptr(ctx, generator).get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        let var_name = self.0.name.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder
            .build_load(self.0.ptr_to_dims(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> IntValue<'ctx> {
        self.0.load_ndims(ctx)
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
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
        let size = self.size(ctx, generator);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, size, "").unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for axis 0 with size {1}",
            [Some(*idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) }
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {}

impl<'ctx> TypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    fn downcast_to_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        value.into_int_value()
    }
}

impl<'ctx> TypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDimsProxy<'ctx, '_> {
    fn upcast_from_type(
        &self,
        _: &mut CodeGenContext<'ctx, '_>,
        value: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        value.into()
    }
}

/// Proxy type for accessing the `data` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDataProxy<'ctx, 'a>(&'a NDArrayValue<'ctx>);

impl<'ctx> ArrayLikeValue<'ctx> for NDArrayDataProxy<'ctx, '_> {
    fn element_type<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> AnyTypeEnum<'ctx> {
        self.0.data().base_ptr(ctx, generator).get_type().get_element_type()
    }

    fn base_ptr<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _: &G,
    ) -> PointerValue<'ctx> {
        let var_name = self.0.name.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder
            .build_load(self.0.ptr_to_data(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    fn size<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &G,
    ) -> IntValue<'ctx> {
        call_ndarray_calc_size(generator, ctx, &self.as_slice_value(ctx, generator), (None, None))
    }
}

impl<'ctx> ArrayLikeIndexer<'ctx> for NDArrayDataProxy<'ctx, '_> {
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        idx: &IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.base_ptr(ctx, generator),
                    &[*idx],
                    name.unwrap_or_default(),
                )
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
        let data_sz = self.size(ctx, generator);
        let in_range = ctx.builder.build_int_compare(IntPredicate::ULT, *idx, data_sz, "").unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds with size {1}",
            [Some(*idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        unsafe { self.ptr_offset_unchecked(ctx, generator, idx, name) }
    }
}

impl<'ctx> UntypedArrayLikeAccessor<'ctx, IntValue<'ctx>> for NDArrayDataProxy<'ctx, '_> {}
impl<'ctx> UntypedArrayLikeMutator<'ctx, IntValue<'ctx>> for NDArrayDataProxy<'ctx, '_> {}

impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> ArrayLikeIndexer<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
    unsafe fn ptr_offset_unchecked<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let indices_elem_ty = indices
            .ptr_offset(ctx, generator, &llvm_usize.const_zero(), None)
            .get_type()
            .get_element_type();
        let Ok(indices_elem_ty) = IntType::try_from(indices_elem_ty) else {
            panic!("Expected list[int32] but got {indices_elem_ty}")
        };
        assert_eq!(
            indices_elem_ty.get_bit_width(),
            32,
            "Expected list[int32] but got list[int{}]",
            indices_elem_ty.get_bit_width()
        );

        let index = call_ndarray_flatten_index(generator, ctx, *self.0, indices);

        unsafe {
            ctx.builder
                .build_in_bounds_gep(
                    self.base_ptr(ctx, generator),
                    &[index],
                    name.unwrap_or_default(),
                )
                .unwrap()
        }
    }

    fn ptr_offset<G: CodeGenerator + ?Sized>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut G,
        indices: &Index,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let indices_size = indices.size(ctx, generator);
        let nidx_leq_ndims = ctx
            .builder
            .build_int_compare(IntPredicate::SLE, indices_size, self.0.load_ndims(ctx), "")
            .unwrap();
        ctx.make_assert(
            generator,
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        let indices_len = indices.size(ctx, generator);
        let ndarray_len = self.0.load_ndims(ctx);
        let len = call_int_umin(ctx, indices_len, ndarray_len, None);
        gen_for_callback_incrementing(
            generator,
            ctx,
            llvm_usize.const_zero(),
            (len, false),
            |generator, ctx, _, i| {
                let (dim_idx, dim_sz) = unsafe {
                    (
                        indices.get_unchecked(ctx, generator, &i, None).into_int_value(),
                        self.0.dim_sizes().get_typed_unchecked(ctx, generator, &i, None),
                    )
                };
                let dim_idx = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(dim_idx, dim_sz.get_type(), "")
                    .unwrap();

                let dim_lt =
                    ctx.builder.build_int_compare(IntPredicate::SLT, dim_idx, dim_sz, "").unwrap();

                ctx.make_assert(
                    generator,
                    dim_lt,
                    "0:IndexError",
                    "index {0} is out of bounds for axis 0 with size {1}",
                    [Some(dim_idx), Some(dim_sz), None],
                    ctx.current_loc,
                );

                Ok(())
            },
            llvm_usize.const_int(1, false),
        )
        .unwrap();

        unsafe { self.ptr_offset_unchecked(ctx, generator, indices, name) }
    }
}

impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> UntypedArrayLikeAccessor<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
}
impl<'ctx, Index: UntypedArrayLikeAccessor<'ctx>> UntypedArrayLikeMutator<'ctx, Index>
    for NDArrayDataProxy<'ctx, '_>
{
}
