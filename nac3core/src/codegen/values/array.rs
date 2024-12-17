use inkwell::{
    types::AnyTypeEnum,
    values::{BasicValueEnum, IntValue, PointerValue},
    IntPredicate,
};

use crate::codegen::{CodeGenContext, CodeGenerator};

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
    Box<dyn Fn(&mut CodeGenContext<'ctx, '_>, BasicValueEnum<'ctx>) -> T + 'ctx>;
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
        let var_name = name.or(self.2).map(|v| format!("{v}.addr")).unwrap_or_default();

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
