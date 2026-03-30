use std::borrow::Cow;

use inkwell::{
    AddressSpace,
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
        type_aligned_allocate, typed_gep, typed_load, typed_store,
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
/// user-defined classes. This delegates to [`is_obj_id_refcounted`] after extracting
/// the `obj_id` from the unifier type.
#[must_use]
pub fn is_refcounted_type(unifier: &mut Unifier, ty: Type) -> bool {
    let ty_enum = unifier.get_ty(ty);
    match &*ty_enum {
        TypeEnum::TObj { obj_id, .. } => is_obj_id_refcounted(*obj_id),
        _ => false,
    }
}

#[derive(Clone, Copy, StructFields)]
pub struct ObjectHeaderStructFields<'ctx> {
    /// The reference count of this object.
    #[value_type(i32)]
    pub refcount: StructField<'ctx, IntValue<'ctx>>,

    /// The offset of the `typeinfo` global structure from `__nac3_global_begin`.
    #[value_type(i32)]
    pub typeinfo_offset: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct ObjectHeaderType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ObjectHeaderStructFields<'ctx>>,
}

impl<'ctx> ObjectHeaderType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "__nac3_object_header") }
    }
}

pub type ObjectHeaderValue<'ctx> = Value<'ctx, ObjectHeaderType<'ctx>>;

impl<'ctx> ObjectHeaderValue<'ctx> {
    /// Initializes the reference count metadata with the following values:
    ///
    /// - `refcount`: Initialized to `1`.
    /// - `field_metadata`: Initialized to the provided `field_metadata` pointer, which points to a
    ///   (usually global) array of `size_t` values, where the first value is the number of fields
    ///   with reference counts, followed by the byte offsets of each field.
    pub fn init(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        is_refcounted: bool,
        typeinfo: TypeinfoValue<'ctx>,
    ) -> anyhow::Result<()> {
        const FUNC_NAME: &str = "__nac3_object_header_init";

        let value = if self.value.get_type().get_element_type() == ctx.i8.into() {
            self.value
        } else {
            ctx.builder.build_pointer_cast(self.value, ctx.ptr, "").unwrap()
        };

        call_extern!(ctx: void _ = FUNC_NAME(value, ctx.i1.const_int(u64::from(is_refcounted), false), typeinfo.value))?;
        Ok(())
    }

    /// Returns an `i1` indicating whether this object is reference-counted.
    pub fn is_refcounted(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<IntValue<'ctx>> {
        const FUNC_NAME: &str = "__nac3_is_object_refcounted";

        let value = if self.value.get_type().get_element_type() == ctx.i8.into() {
            self.value
        } else {
            ctx.builder.build_pointer_cast(self.value, ctx.ptr, "").unwrap()
        };

        call_extern!(ctx: (ctx.i1) _ = FUNC_NAME(value))
    }

    /// Recursively increments the reference count of this object by one.
    pub fn increment_refcount(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let value = if self.value.get_type().get_element_type() == ctx.i8.into() {
            self.value
        } else {
            ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?
        };

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

    /// Recursively decrements the reference count of this object by one.
    ///
    /// When the reference count reaches zero, the object will be automatically deallocated.
    pub fn decrement_refcount(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let value = if self.value.get_type().get_element_type() == ctx.i8.into() {
            self.value
        } else {
            ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?
        };

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
        typed_load(ctx.builder, self.inner_ptr(ctx)?, inner_ty, name)
    }
}

#[derive(Clone, Copy)]
pub struct OpaqueRefCountedType<'ctx> {
    inner: StructType<'ctx>,
}

impl<'ctx> OpaqueRefCountedType<'ctx> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type() }
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
    fn llvm_ty(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.inner.ptr_type(AddressSpace::default()).into()
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
        let obj_header = ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?;
        Ok(unsafe {
            ctx.builder.build_gep(
                obj_header,
                &[ObjectHeaderType::new(ctx)
                    .alloca_ty(ctx)
                    .size_of()
                    .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                    .unwrap()],
                "",
            )?
        })
    }
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner)]
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

    // TODO(Derppening): Do we need `is_refcounted` param here, can we just derive it from `scope`?
    pub fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        scope: AllocationScope,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        T: RefType<'ctx> + WithTypeinfo<'ctx> + Copy,
    {
        let alloca = self.alloca_ty(ctx);
        let ptr = ctx.build_allocate(scope, alloca, name)?;
        let value = self.map_value(ptr, name);

        #[cfg(feature = "malloc")]
        let is_refcounted = matches!(scope, AllocationScope::Default | AllocationScope::Heap);
        #[cfg(not(feature = "malloc"))]
        let is_refcounted = false;
        value.header(ctx).init(ctx, is_refcounted, self.object.typeinfo(ctx))?;

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
        let obj_header = ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?;
        Ok(unsafe {
            ctx.builder.build_gep(
                obj_header,
                &[ObjectHeaderType::new(ctx)
                    .alloca_ty(ctx)
                    .size_of()
                    .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                    .unwrap()],
                "",
            )?
        })
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

#[derive(Clone, Copy)]
pub struct RefCountedArrayType<'ctx, T: ProxyType<'ctx> + Copy> {
    inner: StructType<'ctx>,
    pub array: ArrayType<'ctx>,
    pub elem: T,

    /// If `true`, elements are inline objects with `ObjectHeader`s (e.g., tuples).
    inline_refcounted_elements: bool,
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedArrayType<'ctx, T> {
    /// Creates a new instance of this type.
    ///
    /// Automatically detects whether elements are inline objects with `ObjectHeader`s (e.g.,
    /// tuples). When the element LLVM type is a struct whose first field is an `ObjectHeader`,
    /// the array's typeinfo will use `REFCOUNT_ARRAY_INLINE_MAGIC` so the IRRT iterates by
    /// stride and passes element addresses directly to `refcount_decr`.
    pub fn new(ctx: &ModuleContext<'ctx>, elem_ty: T, static_size: Option<u32>) -> Self {
        let elem_llvm_ty = elem_ty.llvm_ty(ctx);
        let object = elem_llvm_ty.array_type(static_size.unwrap_or_default());

        // Auto-detect inline elements: if the element type is a struct whose first field
        // is an ObjectHeader, elements are stored inline with their own headers (e.g. tuples).
        let header_ty = ObjectHeaderType::new(ctx).alloca_ty(ctx);
        let inline_refcounted_elements = if let BasicTypeEnum::StructType(st) = elem_llvm_ty {
            st.count_fields() >= 2
                && unsafe { st.get_field_type_at_index_unchecked(0) } == header_ty
        } else {
            false
        };

        Self {
            inner: ctx.ctx.struct_type(
                &[
                    header_ty.into_struct_type().into(),
                    ctx.ctx
                        .struct_type(&[ctx.size_t.into(), object.as_basic_type_enum()], false)
                        .into(),
                ],
                false,
            ),
            array: object,
            elem: elem_ty,
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

        let value_ptr = if !self.array.is_empty() && size.is_constant_int() {
            assert_eq!(
                size.get_zero_extended_constant(),
                Some(u64::from(self.array.len())),
                "Expected size {} to match static size {} of RefCountedArrayType",
                size.get_zero_extended_constant().unwrap(),
                self.array.len()
            );

            let alloca = self.alloca_ty(ctx);
            let ptr = ctx.build_allocate(scope, alloca, name)?;

            // Pretend to be a dynamically-sized array for consistent types
            ctx.builder.build_pointer_cast(
                ptr,
                llvm_dyn_array_ty.llvm_ty(ctx).into_pointer_type(),
                "",
            )?
        } else {
            let align_ty = self.inner;

            let sizeof_elem = self
                .array
                .get_element_type()
                .size_of()
                .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                .unwrap();
            let sizeof_zero_elem = llvm_dyn_array_ty
                .inner
                .llvm_ty(ctx)
                .into_struct_type()
                .size_of()
                .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                .unwrap();

            // sizeof(arr) = sizeof(ObjectHeader) + sizeof(elem) * n
            let alloc_size = ctx.builder.build_int_add(
                sizeof_zero_elem,
                ctx.builder.build_int_mul(sizeof_elem, size, "")?,
                "",
            )?;

            let ptr = type_aligned_allocate(ctx, scope, align_ty, alloc_size, name)?;
            ctx.builder.build_pointer_cast(
                ptr.value.0,
                llvm_dyn_array_ty.llvm_ty(ctx).into_pointer_type(),
                "",
            )?
        };

        let value = self.map_value(value_ptr, name);

        // Whether this array is refcounted or not depends on if the array is allocated on the heap or not
        #[cfg(feature = "malloc")]
        let is_refcounted = matches!(scope, AllocationScope::Default | AllocationScope::Heap);
        #[cfg(not(feature = "malloc"))]
        let is_refcounted = false;
        value.header(ctx).init(ctx, is_refcounted, self.typeinfo(ctx))?;

        // Store the size into the array metadata
        let inner = value.inner_ptr(ctx)?;
        let psize = ctx.builder.build_pointer_cast(
            inner,
            llvm_dyn_array_ty.llvm_ty(ctx).into_pointer_type(),
            "",
        )?;

        // Store the number of refcounted elements in the array for recursive reference count
        // updates
        //
        // Note: Stack-allocated arrays containing refcounted objects should still hold a strong
        // reference to their elements to prevent unintentional deallocation
        if self.array.get_element_type().is_pointer_type() {
            typed_store(ctx.builder, psize, size)?;
        } else {
            typed_store(ctx.builder, psize, ctx.size_t.const_zero())?;
        }

        // Zero-initialize the array if this array stores pointers to avoid unintentional access of
        // uninitialized values
        if self.array.get_element_type().is_pointer_type() {
            llvm_intrinsics::call_memset(
                ctx,
                ctx.builder.build_pointer_cast(value.inner_value(ctx)?.value.0, ctx.ptr, "")?,
                ctx.i8.const_zero(),
                ctx.builder.build_int_mul(
                    self.array
                        .get_element_type()
                        .size_of()
                        .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                        .unwrap(),
                    size,
                    "",
                )?,
            )?;
        }

        Ok(value)
    }

    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        self.allocate_impl(
            ctx,
            if self.array.is_empty() {
                AllocationScope::StackCurrentLoc
            } else {
                AllocationScope::StackStartOfFunc
            },
            size,
            name,
        )
    }

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

    #[allow(deprecated)]
    fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        assert_ne!(self.array.len(), 0, "Cannot allocate RefCountedArrayType with unknown size");

        self.alloca(ctx, ctx.size_t.const_int(u64::from(self.array.len()), false), name)
    }

    fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        Self: RefType<'ctx> + Copy,
    {
        assert_ne!(self.array.len(), 0, "Cannot allocate RefCountedArrayType with unknown size");

        self.allocate(ctx, ctx.size_t.const_int(u64::from(self.array.len()), false), name)
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

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
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
        assert_ne!(
            self.array.len(),
            0,
            "RefCountedArrayType with an unknown size cannot be allocated"
        );

        self.inner.into()
    }
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedType<'ctx> for RefCountedArrayType<'ctx, T> {}

pub type RefCountedArrayValue<'ctx, T> = Value<'ctx, RefCountedArrayType<'ctx, T>>;

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedArrayValue<'ctx, T> {
    pub fn len(&self, ctx: &CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        let psize = unsafe {
            typed_gep(
                ctx.builder,
                &self.ty.inner.get_field_type_at_index_unchecked(1),
                self.inner_ptr(ctx)?,
                &[ctx.size_t.const_zero(), ctx.i32.const_zero()],
                "",
            )?
        };
        Ok(typed_load(ctx.builder, psize, ctx.size_t.into(), "")?.into_int_value())
    }

    /// Returns the data portion of this array as an [`ArraySliceValue`].
    ///
    /// The length used for bounds checking comes from the array's internal metadata field, which
    /// tracks the number of refcounted elements (0 for non-pointer element types). Use
    /// [`inner_value_with_len`](Self::inner_value_with_len) when you have the actual element
    /// count (e.g., from a list's `len` field) and need correct bounds checking.
    pub fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, T>> {
        self.inner_value_with_len(ctx, self.len(ctx)?)
    }

    /// Returns the data portion of this array as an [`ArraySliceValue`] with an explicit length
    /// for bounds checking.
    pub fn inner_value_with_len(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, T>> {
        let pdata = unsafe {
            typed_gep(
                ctx.builder,
                &self.ty.inner.get_field_type_at_index_unchecked(1),
                self.inner_ptr(ctx)?,
                &[ctx.size_t.const_zero(), ctx.i32.const_int(1, false)],
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
        let obj_header = if self.value.get_type() == ctx.ptr {
            self.value
        } else {
            ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?
        };
        Ok(unsafe {
            ctx.builder.build_gep(
                obj_header,
                &[ObjectHeaderType::new(ctx)
                    .alloca_ty(ctx)
                    .size_of()
                    .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                    .unwrap()],
                "",
            )?
        })
    }
}
