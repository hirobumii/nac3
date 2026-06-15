use std::borrow::Cow;

use inkwell::{
    types::{ArrayType, BasicType, BasicTypeEnum, StructType},
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        allocator::AllocationScope,
        expr::call_extern,
        irrt::get_usize_dependent_function_name,
        llvm_intrinsics,
        stmt::gen_if_callback,
        type_aligned_allocate,
        types::{
            ArraySliceValue, BuiltinStruct, ProxyType, ProxyTypeBase, RefType, TypeinfoValue,
            Value, WithTypeinfo, structure::StructField,
        },
    },
    toplevel::{DefinitionId, helper::PrimDef},
    typecheck::typedef::{Type, TypeEnum, Unifier},
};

/// Returns `true` if the given `obj_id` corresponds to a type that uses reference counting.
#[must_use]
pub fn is_obj_id_refcounted(obj_id: DefinitionId) -> bool {
    const NON_REFCOUNTED: &[PrimDef] = &[
        PrimDef::Float,
        PrimDef::Bool,
        PrimDef::Str,
        PrimDef::Int32,
        PrimDef::Int64,
        PrimDef::UInt32,
        PrimDef::UInt64,
        PrimDef::Range,
        PrimDef::Exception,
        PrimDef::Enumerate,
        PrimDef::Tuple,
        PrimDef::None,
    ];
    !NON_REFCOUNTED.iter().any(|p| p.id() == obj_id)
}

/// Returns whether the given unifier type is a reference-counted composite type.
///
/// Reference-counted types are heap-allocated composites: `list`, `ndarray`, `option`, and
/// user-defined classes.
#[must_use]
pub fn is_refcounted_type(unifier: &mut Unifier, ty: Type) -> bool {
    match &*unifier.get_ty(ty) {
        TypeEnum::TObj { obj_id, .. } => is_obj_id_refcounted(*obj_id),
        _ => false,
    }
}

/// The structure fields for the `ObjectHeader` struct.
#[derive(Clone, Copy, StructFields)]
pub struct ObjectHeaderStructFields<'ctx> {
    /// The reference count of this object.
    #[value_type(i32)]
    pub refcount: StructField<'ctx, IntValue<'ctx>>,

    /// The offset of the `typeinfo` global structure from `__nac3_global_begin`.
    #[value_type(i32)]
    pub typeinfo_offset: StructField<'ctx, IntValue<'ctx>>,
}

/// Proxy type representing the header of a reference-counted object.
#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(PointerValue<'ctx>, ctx.ptr)]
pub struct ObjectHeaderType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ObjectHeaderStructFields<'ctx>>,
}

impl<'ctx> ObjectHeaderType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_object_header") }
    }

    /// Returns the LLVM type used for allocating this type.
    ///
    /// Note that `ObjectHeaderType` should never be allocated directly, it should always be
    /// allocated as part of a refcounted object; This only serves as a convenience function for
    /// returning the struct layout of the object header.
    pub fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.llvm_ty.into()
    }

    /// Returns whether `ty` is the named LLVM struct type for `ObjectHeader`
    /// (`%__nac3_object_header`).
    ///
    /// Useful for detecting NAC3 value types (currently tuples) that prepend an `ObjectHeader`
    /// to their payload, e.g. when bridging to a C ABI at the FFI boundary.
    #[must_use]
    pub fn is_layout_match(ty: BasicTypeEnum<'ctx>) -> bool {
        let BasicTypeEnum::StructType(s) = ty else { return false };
        s.get_name().is_some_and(|n| n.to_bytes() == b"__nac3_object_header")
    }
}

pub type ObjectHeaderValue<'ctx> = Value<'ctx, ObjectHeaderType<'ctx>>;

impl<'ctx> ObjectHeaderValue<'ctx> {
    /// Initializes the reference count metadata with the following values:
    ///
    /// - `refcount`: Initialized to 1 if `is_refcounted` is `true`, or 0 if `false`.
    /// - `typeinfo`: The `typeinfo` instance for this object.
    pub fn init(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        is_refcounted: bool,
        typeinfo: TypeinfoValue<'ctx>,
    ) -> anyhow::Result<()> {
        const FUNC_NAME: &str = "__nac3_object_header_init";

        let value = self.value;

        call_extern!(ctx: void _ = FUNC_NAME(value, ctx.i1.const_int(u64::from(is_refcounted), false), typeinfo.value))?;
        Ok(())
    }

    /// Returns an `i1` indicating whether this object is reference-counted.
    pub fn is_refcounted(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<IntValue<'ctx>> {
        const FUNC_NAME: &str = "__nac3_is_object_refcounted";

        let value = self.value;

        call_extern!(ctx: (ctx.i1) _ = FUNC_NAME(value))
    }

    /// Increments the reference count of this object by one.
    pub fn increment_refcount(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let value = self.value;

        let func_name = get_usize_dependent_function_name(ctx, "__nac3_refcount_incr");

        call_extern!(ctx: void _ = func_name(value))?;
        Ok(())
    }

    /// Similar to [`ObjectHeaderValue::increment_refcount`], additionally checking if the value is
    /// `null` before incrementing.
    pub fn safe_increment_refcount(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<()> {
        gen_if_callback(
            &mut (),
            ctx,
            |(), ctx| Ok(ctx.builder.build_is_not_null(self.value, "")?),
            |(), ctx| {
                self.increment_refcount(ctx)?;
                Ok(())
            },
            |(), _| Ok(()),
        )
    }

    /// Decrements the reference count of this object by one.
    ///
    /// When the reference count reaches zero, the object will be automatically deallocated.
    pub fn decrement_refcount(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let value = self.value;

        let func_name = get_usize_dependent_function_name(ctx, "__nac3_refcount_decr");

        call_extern!(ctx: void _ = func_name(value))?;
        Ok(())
    }

    /// Similar to [`ObjectHeaderValue::decrement_refcount`], additionally checking if the value is
    /// `null` before incrementing.
    pub fn safe_decrement_refcount(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<()> {
        gen_if_callback(
            &mut (),
            ctx,
            |(), ctx| Ok(ctx.builder.build_is_not_null(self.value, "")?),
            |(), ctx| {
                self.decrement_refcount(ctx)?;
                Ok(())
            },
            |(), _| Ok(()),
        )
    }

    /// Returns the reference count of this object.
    pub fn refcount(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        let struct_ty = self.ty.alloca_ty(ctx);
        self.ty.inner.fields.refcount.load(ctx, struct_ty, self.value, self.name)
    }
}

/// A trait indicating that a type is reference-counted.
pub trait RefCountedType<'ctx> {}

/// A trait indicating that a value is reference-counted.
pub trait RefCountedValue<'ctx> {
    /// Returns an opaque variant of this refcounted value.
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx>;

    /// Gets the object header of this value.
    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx>;

    /// Returns a pointer to the inner data of this refcounted object.
    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>>;

    /// Returns a loaded value of the inner data of this refcounted object.
    fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        inner_ty: BasicTypeEnum<'ctx>,
        name: &str,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        Ok(ctx.builder.build_load(inner_ty, self.inner_ptr(ctx)?, name)?)
    }
}

/// Proxy type representing a reference-counted type with an opaque inner structure.
#[derive(Clone, Copy)]
pub struct OpaqueRefCountedType<'ctx> {
    _phantom: std::marker::PhantomData<&'ctx ()>,
}

impl<'ctx> OpaqueRefCountedType<'ctx> {
    /// Creates a new instance of this type.
    pub const fn new(_ctx: &ModuleContext<'ctx>) -> Self {
        Self { _phantom: std::marker::PhantomData }
    }
}

impl<'ctx> ProxyTypeBase<'ctx> for OpaqueRefCountedType<'ctx> {
    type Value = PointerValue<'ctx>;

    fn alloca(
        &self,
        _ctx: &mut CodeGenContext<'ctx, '_>,
        _name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        unreachable!("OpaqueRefCountedType cannot be allocated directly");
    }

    fn allocate(
        &self,
        _ctx: &mut CodeGenContext<'ctx, '_>,
        _name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        unreachable!("OpaqueRefCountedType cannot be allocated directly");
    }

    fn map_value(&self, value: Self::Value, name: Option<&'ctx str>) -> Value<'ctx, Self>
    where
        Self: Sized + Copy,
    {
        Value { ty: *self, value, name }
    }
}

impl<'ctx> ProxyType<'ctx> for OpaqueRefCountedType<'ctx> {
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        ctx.ptr.into()
    }
}

impl<'ctx> RefCountedType<'ctx> for OpaqueRefCountedType<'ctx> {}

pub type OpaqueRefCountedValue<'ctx> = Value<'ctx, OpaqueRefCountedType<'ctx>>;

impl<'ctx> RefCountedValue<'ctx> for OpaqueRefCountedValue<'ctx> {
    fn as_opaque(&self, _ctx: &ModuleContext<'ctx>) -> Self {
        *self
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        let sizeof_header = ctx.builder.build_int_truncate_or_bit_cast(
            ObjectHeaderType::new(ctx).alloca_ty(ctx).size_of().unwrap(),
            ctx.size_t,
            "",
        )?;
        Ok(unsafe { ctx.builder.build_gep(ctx.i8, self.value, &[sizeof_header], "")? })
    }
}

/// A reference-counted type with a known inner structure represented by `T`.
#[derive(Clone, Copy, ProxyType)]
#[llvm_ty(PointerValue<'ctx>, ctx.ptr)]
pub struct TypedRefCountedType<'ctx, T: RefType<'ctx> + Copy> {
    inner: StructType<'ctx>,
    pub object: T,
}

impl<'ctx, T: RefType<'ctx> + Copy> TypedRefCountedType<'ctx, T> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>, object_ty: T) -> Self {
        let header = ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type();
        let object = object_ty.alloca_ty(ctx);

        Self { inner: ctx.ctx.struct_type(&[header.into(), object], false), object: object_ty }
    }

    /// Returns the LLVM type used for allocating this type.
    pub fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.into()
    }

    /// Allocates an instance of this type.
    pub fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        scope: AllocationScope,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        let alloca = self.alloca_ty(ctx);
        let ptr = ctx.build_allocate(scope, alloca, name)?;
        let value = self.map_value(ptr, name);

        #[cfg(feature = "malloc")]
        let is_refcounted = matches!(scope, AllocationScope::Default | AllocationScope::Heap);
        #[cfg(not(feature = "malloc"))]
        let is_refcounted = false;
        let typeinfo = self.object.typeinfo(ctx);
        value.header(ctx).init(ctx, is_refcounted, typeinfo)?;

        // Zero-initialize the inner data so that pointer fields (e.g. refcounted children
        // in class fields) start as null rather than garbage.
        let inner_ptr = value.inner_ptr(ctx)?;
        let inner_ty = self.object.alloca_ty(ctx);
        let inner_size = ctx.builder.build_int_truncate_or_bit_cast(
            inner_ty.size_of().unwrap(),
            ctx.size_t,
            "",
        )?;
        llvm_intrinsics::call_memset(ctx, inner_ptr, ctx.i8.const_zero(), inner_size)?;

        Ok(value)
    }
}

impl<'ctx, T: RefType<'ctx> + Copy> RefCountedType<'ctx> for TypedRefCountedType<'ctx, T> {}

pub type TypedRefCountedValue<'ctx, T> = Value<'ctx, TypedRefCountedType<'ctx, T>>;

impl<'ctx, T: RefType<'ctx> + Copy> TypedRefCountedValue<'ctx, T> {
    /// Returns a loaded value of the inner data of this refcounted object.
    pub fn inner_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<Value<'ctx, T>> {
        Ok(self.ty.object.map_value(self.inner_ptr(ctx)?, self.name))
    }
}

impl<'ctx, T: RefType<'ctx> + Copy> RefCountedValue<'ctx> for TypedRefCountedValue<'ctx, T> {
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx> {
        OpaqueRefCountedType::new(ctx).map_value(self.value, self.name)
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        let sizeof_header = ctx.builder.build_int_truncate_or_bit_cast(
            ObjectHeaderType::new(ctx).alloca_ty(ctx).size_of().unwrap(),
            ctx.size_t,
            "",
        )?;
        Ok(unsafe { ctx.builder.build_gep(ctx.i8, self.value, &[sizeof_header], "")? })
    }

    fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        _inner_ty: BasicTypeEnum<'ctx>,
        _name: &str,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        Ok(self.inner_value(ctx)?.value.into())
    }
}

/// Type representing a reference-counted array with element type `T`.
#[derive(Clone, Copy)]
pub struct RefCountedArrayType<'ctx, T: ProxyType<'ctx> + Copy> {
    inner: StructType<'ctx>,

    /// The array type used to represent the `data` of this array.
    ///
    /// This is `[N x ty]` if the static size `N` is known at compile time, or `[0 x ty]` if the
    /// size is only known at runtime.
    pub array: ArrayType<'ctx>,

    /// The element type of this array.
    pub elem: T,

    /// Compile-time-known number of elements, or `None` for dynamically-sized arrays.
    ///
    /// This is stored separately from `array.len()` because LLVM's `ArrayType::len()` returns 0
    /// for both `[0 x T]` (static size 0, e.g. 0-dim ndarray shape) and dynamically-sized arrays,
    /// making the two cases indistinguishable via the LLVM type alone.
    static_size: Option<u32>,

    /// If `true`, elements are inline objects with an object header (e.g. tuples).
    inline_refcounted_elements: bool,
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedArrayType<'ctx, T> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>, elem_ty: T, static_size: Option<u32>) -> Self {
        let elem_llvm_ty = elem_ty.llvm_ty(ctx);
        let object = elem_llvm_ty.array_type(static_size.unwrap_or_default());

        // If the element type is a struct whose first field is an object header, the elements of
        // the array are assumed to be inlined refcounted objects (e.g. tuples)
        let header_ty = ObjectHeaderType::new(ctx).alloca_ty(ctx);
        let inline_refcounted_elements = if let BasicTypeEnum::StructType(st) = elem_llvm_ty {
            st.count_fields() >= 2
                && unsafe { st.get_field_type_at_index_unchecked(0) } == header_ty
        } else {
            false
        };

        // Pad the count field out to 8 bytes so the element array always begins at a fixed,
        // 8-byte-aligned offset (16 bytes from the object base) regardless of `size_t`'s width
        let count_pad_bytes = 8u32.saturating_sub(ctx.size_t.get_bit_width() / 8);
        let count_pad_ty = ctx.ctx.i8_type().array_type(count_pad_bytes);

        Self {
            inner: ctx.ctx.struct_type(
                &[
                    header_ty.into_struct_type().into(),
                    ctx.ctx
                        .struct_type(
                            &[ctx.size_t.into(), count_pad_ty.into(), object.as_basic_type_enum()],
                            false,
                        )
                        .into(),
                ],
                false,
            ),
            array: object,
            elem: elem_ty,
            static_size,
            inline_refcounted_elements,
        }
    }

    fn allocate_impl(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        scope: AllocationScope,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        assert_eq!(size.get_type(), ctx.size_t);

        let llvm_dyn_array_ty = Self::new(ctx, self.elem, None);

        let value_ptr = if let Some(n) = self.static_size.filter(|_| size.is_constant_int()) {
            assert_eq!(
                size.get_zero_extended_constant(),
                Some(u64::from(n)),
                "Expected size {} to match static size {} of RefCountedArrayType",
                size.get_zero_extended_constant().unwrap(),
                n
            );

            let alloca = self.alloca_ty(ctx);
            ctx.build_allocate(scope, alloca, name)?
        } else {
            let align_ty = self.inner;

            let sizeof_elem = ctx.builder.build_int_truncate_or_bit_cast(
                self.array.get_element_type().size_of().unwrap(),
                ctx.size_t,
                "",
            )?;
            let sizeof_zero_elem = ctx.builder.build_int_truncate_or_bit_cast(
                llvm_dyn_array_ty.inner.llvm_ty(ctx).into_struct_type().size_of().unwrap(),
                ctx.size_t,
                "",
            )?;

            // sizeof(arr) = sizeof(ObjectHeader) + sizeof(elem) * n
            let alloc_size = ctx.builder.build_int_add(
                sizeof_zero_elem,
                ctx.builder.build_int_mul(sizeof_elem, size, "")?,
                "",
            )?;

            let ptr = type_aligned_allocate(ctx, scope, align_ty, alloc_size, name)?;
            ptr.value.0
        };

        let value = self.map_value(value_ptr, name);

        // Whether this array is refcounted or not depends on if the array is allocated on the heap or not
        #[cfg(feature = "malloc")]
        let is_refcounted = matches!(scope, AllocationScope::Default | AllocationScope::Heap);
        #[cfg(not(feature = "malloc"))]
        let is_refcounted = false;
        let typeinfo = self.typeinfo(ctx);
        value.header(ctx).init(ctx, is_refcounted, typeinfo)?;

        // Store the size into the array metadata
        let psize = value.inner_ptr(ctx)?;

        // Store the number of refcounted elements in the array for recursive reference count
        // updates
        //
        // Note: Stack-allocated arrays containing refcounted objects should still hold a strong
        // reference to their elements to prevent unintentional deallocation
        if self.array.get_element_type().is_pointer_type() {
            ctx.builder.build_store(psize, size)?;
        } else {
            ctx.builder.build_store(psize, ctx.size_t.const_zero())?;
        }

        // Zero-initialize the array if this array stores pointers to avoid unintentional access of
        // uninitialized values
        if self.array.get_element_type().is_pointer_type() {
            let sizeof_elem = ctx.builder.build_int_truncate_or_bit_cast(
                self.array.get_element_type().size_of().unwrap(),
                ctx.size_t,
                "",
            )?;
            llvm_intrinsics::call_memset(
                ctx,
                value.inner_value(ctx, Some(size))?.value.0,
                ctx.i8.const_zero(),
                ctx.builder.build_int_mul(sizeof_elem, size, "")?,
            )?;
        }

        Ok(value)
    }

    /// Allocates an instance of this type on the stack with the given size.
    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        self.allocate_impl(
            ctx,
            if self.static_size.is_none() {
                AllocationScope::StackCurrentLoc
            } else {
                AllocationScope::StackStartOfFunc
            },
            size,
            name,
        )
    }

    /// Allocates an instance of this type in the
    /// [default allocation scope][AllocationScope::Default] with the given size.
    pub fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        self.allocate_impl(ctx, AllocationScope::Default, size, name)
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> ProxyTypeBase<'ctx> for RefCountedArrayType<'ctx, T> {
    type Value = PointerValue<'ctx>;

    fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        let n = self.static_size.expect("Cannot allocate RefCountedArrayType with unknown size");

        self.alloca(ctx, ctx.size_t.const_int(u64::from(n), false), name)
    }

    fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        let n = self.static_size.expect("Cannot allocate RefCountedArrayType with unknown size");

        self.allocate(ctx, ctx.size_t.const_int(u64::from(n), false), name)
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> ProxyType<'ctx> for RefCountedArrayType<'ctx, T> {
    fn llvm_ty(&self, ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        ctx.ptr.into()
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> WithTypeinfo<'ctx> for RefCountedArrayType<'ctx, T> {
    fn typename(&self) -> Cow<'static, str> {
        if self.inline_refcounted_elements {
            // Inline element arrays need unique typeinfo per element type (different strides).
            let elem_str = self.array.get_element_type().print_to_string();
            let sanitized: String = elem_str
                .to_str()
                .unwrap_or("unknown")
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();
            Cow::Owned(format!("__nac3_array_inline_{sanitized}"))
        } else {
            Cow::Borrowed("__nac3_array")
        }
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        if self.inline_refcounted_elements {
            // REFCOUNT_ARRAY_INLINE_MAGIC (0xFFFFFFFE) + stride (byte size of each element)
            let data_layout = ctx.target.get_target_data();
            let elem_size = data_layout.get_store_size(&self.array.get_element_type());
            vec![ctx.i32.const_int(0xFFFF_FFFE, false), ctx.i32.const_int(elem_size, false)]
        } else {
            // REFCOUNT_ARRAY_MAGIC (0xFFFFFFFF) — pointer-element arrays
            vec![ctx.i32.const_all_ones()]
        }
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefType<'ctx> for RefCountedArrayType<'ctx, T> {
    fn alloca_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        assert!(
            self.static_size.is_some(),
            "RefCountedArrayType with an unknown size cannot be allocated"
        );

        self.inner.into()
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedType<'ctx> for RefCountedArrayType<'ctx, T> {}

pub type RefCountedArrayValue<'ctx, T> = Value<'ctx, RefCountedArrayType<'ctx, T>>;

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedArrayValue<'ctx, T> {
    /// Returns the data portion of this array as an [`ArraySliceValue`].
    ///
    /// The caller must provide the length of the array as `len` if the size of this array type is
    /// not known at compile-time.
    pub fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        len: Option<IntValue<'ctx>>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, T>> {
        let len = len.unwrap_or_else(|| {
            let n = self.ty.static_size.expect(
                "inner_value(None) called on a dynamically-sized RefCountedArrayType; \
                     pass Some(runtime_len) or create with a static_size",
            );
            ctx.size_t.const_int(u64::from(n), false)
        });
        let pdata = unsafe {
            ctx.builder.build_gep(
                self.ty.inner.get_field_type_at_index_unchecked(1),
                self.inner_ptr(ctx)?,
                // Field 2 of the inner struct is the element array (field 1 is the count-padding).
                &[ctx.size_t.const_zero(), ctx.i32.const_int(2, false)],
                "",
            )?
        };
        Ok(ArraySliceValue::new(self.ty.elem, pdata, len, self.name))
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedValue<'ctx> for RefCountedArrayValue<'ctx, T> {
    fn as_opaque(&self, ctx: &ModuleContext<'ctx>) -> OpaqueRefCountedValue<'ctx> {
        OpaqueRefCountedType::new(ctx).map_value(self.value, self.name)
    }

    fn header(&self, ctx: &ModuleContext<'ctx>) -> ObjectHeaderValue<'ctx> {
        ObjectHeaderType::new(ctx).map_value(self.value, self.name)
    }

    fn inner_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<PointerValue<'ctx>> {
        let sizeof_header = ctx.builder.build_int_truncate_or_bit_cast(
            ObjectHeaderType::new(ctx).alloca_ty(ctx).size_of().unwrap(),
            ctx.size_t,
            "",
        )?;
        Ok(unsafe { ctx.builder.build_gep(ctx.i8, self.value, &[sizeof_header], "")? })
    }
}
