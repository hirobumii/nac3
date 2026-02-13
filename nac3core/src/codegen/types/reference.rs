use inkwell::{
    AddressSpace,
    module::Linkage,
    types::{AnyTypeEnum, ArrayType, BasicType, BasicTypeEnum, StructType},
    values::{BasicValue, BasicValueEnum, IntValue, PointerValue},
};
use itertools::Itertools as _;
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    allocator::AllocationScope,
    expr::call_extern,
    irrt::get_usize_dependent_function_name,
    llvm_intrinsics,
    stmt::gen_if_callback,
    type_aligned_allocate, typed_load, typed_store,
    types::{
        ArraySliceValue, BuiltinStruct, ProxyType, ProxyTypeBase, RefType, StringType,
        TypeinfoType, TypeinfoValue, Value, WithTypeinfo, structure::StructField,
    },
};

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
pub trait RefCountedType<'ctx> {
    /// Maps an existing reference-counted value to a typed value, returning `None` if the value
    /// does not have the expected structure of a reference-counted type.
    ///
    /// The expected structure of a reference-counting type is either:
    ///
    /// - `%nac3_object_header*` (for opaque reference-counted types), or
    /// - A struct type whose first field is a `%nac3_object_header`.
    // TODO(Derppening): Remove once all refcounted types have an object header
    fn map_refcounted_value(
        &self,
        value: <Self as ProxyTypeBase<'ctx>>::Value,
        name: Option<&'ctx str>,
    ) -> Option<Value<'ctx, Self>>
    where
        Self: ProxyTypeBase<'ctx, Value = PointerValue<'ctx>> + Copy,
    {
        let BasicTypeEnum::PointerType(ptr_ty) = value.as_basic_value_enum().get_type() else {
            return None;
        };
        let AnyTypeEnum::StructType(struct_ty) = ptr_ty.get_element_type() else {
            return None;
        };

        if struct_ty.get_name().is_some_and(|name| name.to_str().unwrap() == "__nac3_object_header")
        {
            return Some(self.map_value(value, name));
        }

        let Some(BasicTypeEnum::StructType(field_ty)) = struct_ty.get_field_type_at_index(0) else {
            return None;
        };

        if field_ty.get_name().is_some_and(|name| name.to_str().unwrap() == "__nac3_object_header")
        {
            Some(self.map_value(value, name))
        } else {
            None
        }
    }
}

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
    pub fn new(ctx: &mut CodeGenContext<'ctx, '_>, object_ty: T) -> Self {
        let header = ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type();
        let object = object_ty.alloca_ty(ctx);

        Self { inner: ctx.ctx.struct_type(&[header.into(), object], false), object: object_ty }
    }

    pub fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        scope: AllocationScope,
        is_refcounted: bool,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>>
    where
        T: RefType<'ctx> + WithTypeinfo<'ctx> + Copy,
    {
        let alloca = self.alloca_ty(ctx);
        let ptr = ctx.build_allocate(scope, alloca, name)?;
        // TODO(Derppening): Uncomment once all refcounted types have an object header

        let value = self
            .map_refcounted_value(ptr, name)
            .unwrap_or_else(|| panic!("{} is not a refcounted value", ptr.get_type()));

        value.header(ctx).init(ctx, is_refcounted, T::typeinfo(ctx))?;

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
    elem: T,
}

impl<'ctx, T: ProxyType<'ctx> + Copy> RefCountedArrayType<'ctx, T> {
    /// Creates a new instance of this type.
    pub fn new(ctx: &ModuleContext<'ctx>, elem_ty: T, static_size: Option<u32>) -> Self {
        let object = elem_ty.llvm_ty(ctx).array_type(static_size.unwrap_or_default());

        Self {
            inner: ctx.ctx.struct_type(
                &[
                    ObjectHeaderType::new(ctx).alloca_ty(ctx).into_struct_type().into(),
                    ctx.ctx
                        .struct_type(&[ctx.size_t.into(), object.as_basic_type_enum()], false)
                        .into(),
                ],
                false,
            ),
            array: object,
            elem: elem_ty,
        }
    }

    fn allocate_impl(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        scope: AllocationScope,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        let llvm_dyn_array_ty = Self::new(ctx, self.elem, None);

        let value_ptr = if size.is_constant_int() {
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
            let align_ty =
                self.llvm_ty(ctx).into_pointer_type().get_element_type().into_struct_type();

            let sizeof_elem = self
                .array
                .get_element_type()
                .size_of()
                .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                .unwrap();
            let sizeof_zero_elem = llvm_dyn_array_ty
                .llvm_ty(ctx)
                .into_pointer_type()
                .get_element_type()
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

        // Zero-initialize the array if this array stores pointers to avoid unintentional access of
        // uninitialized values
        if self.array.get_element_type().is_pointer_type() {
            llvm_intrinsics::call_memset_generic_array(
                ctx,
                value.inner_ptr(ctx)?,
                ctx.i8.const_zero(),
                size,
            )?;
        }

        value.header(ctx).init(ctx, true, Self::typeinfo(ctx))?;

        // Store the size into the array metadata
        let inner = value.inner_ptr(ctx)?;
        let inner = ctx.builder.build_pointer_cast(
            inner,
            llvm_dyn_array_ty.llvm_ty(ctx).into_pointer_type(),
            "",
        )?;
        let psize = unsafe {
            ctx.builder.build_in_bounds_gep(
                inner,
                &[ctx.size_t.const_zero(), ctx.i32.const_zero()],
                "",
            )?
        };
        typed_store(ctx.builder, psize, size)?;

        Ok(value)
    }

    pub fn alloca(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        self.allocate_impl(ctx, AllocationScope::StackStartOfFunc, size, name)
    }

    fn allocate(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<Value<'ctx, Self>> {
        self.allocate_impl(ctx, AllocationScope::Default, size, name)
    }

    /// Maps an existing reference-counted value to a typed value, returning `None` if the value
    /// does not have the expected structure of a reference-counted type.
    ///
    /// The expected structure of a reference-counting type is either:
    ///
    /// - `%nac3_object_header*` (for opaque reference-counted types), or
    /// - A struct type whose first field is a `%nac3_object_header`.
    pub fn map_refcounted_value<V>(
        &self,
        value: <Self as ProxyTypeBase<'ctx>>::Value,
        name: Option<&'ctx str>,
    ) -> Option<Value<'ctx, Self>>
    where
        Self: ProxyTypeBase<'ctx, Value = V> + Copy,
        V: BasicValue<'ctx>,
    {
        let BasicTypeEnum::PointerType(ptr_ty) = value.as_basic_value_enum().get_type() else {
            return None;
        };
        let AnyTypeEnum::StructType(struct_ty) = ptr_ty.get_element_type() else {
            return None;
        };

        if struct_ty.get_name().is_some_and(|name| name.to_str().unwrap() == "__nac3_object_header")
        {
            return Some(self.map_value(value, name));
        }

        let Some(BasicTypeEnum::StructType(field_ty)) = struct_ty.get_field_type_at_index(0) else {
            return None;
        };

        if field_ty.get_name().is_some_and(|name| name.to_str().unwrap() == "__nac3_object_header")
        {
            Some(self.map_value(value, name))
        } else {
            None
        }
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
    fn typeinfo(ctx: &ModuleContext<'ctx>) -> TypeinfoValue<'ctx> {
        const NAME: &str = "__nac3_array";

        let global = ctx.module.get_global(&format!("typeinfo for {NAME}")).unwrap_or_else(|| {
            let name_data =
                ctx.module.get_global(&format!("typename array for {NAME}")).unwrap_or_else(|| {
                    let name_data = ctx.module.add_global(
                        ctx.i8.array_type(NAME.len() as u32),
                        None,
                        &format!("typename array for {NAME}"),
                    );
                    name_data.set_linkage(Linkage::WeakAny);
                    name_data.set_initializer(
                        &ctx.i8.const_array(
                            &NAME
                                .as_bytes()
                                .iter()
                                .map(|&b| ctx.i8.const_int(u64::from(b), false))
                                .collect_vec(),
                        ),
                    );
                    name_data.set_constant(true);

                    name_data
                });

            let name =
                ctx.module.get_global(&format!("typename for {NAME}")).unwrap_or_else(|| {
                    let llvm_str = StringType::new(ctx).llvm_ty(ctx).into_struct_type();
                    let name =
                        ctx.module.add_global(llvm_str, None, &format!("typename for {NAME}"));
                    name.set_linkage(Linkage::WeakAny);
                    name.set_initializer(&llvm_str.const_named_struct(&[
                        name_data.as_pointer_value().into(),
                        ctx.size_t.const_int(NAME.len() as u64, false).into(),
                    ]));
                    name.set_constant(true);

                    name
                });

            let refcounted_field_offsets = ctx.module.add_global(
                ctx.i32.array_type(1),
                None,
                &format!("refcounted_fields array for {NAME}"),
            );
            refcounted_field_offsets.set_linkage(Linkage::WeakAny);
            refcounted_field_offsets
                .set_initializer(&ctx.i32.const_array(&[ctx.i32.const_all_ones()]));
            refcounted_field_offsets.set_constant(true);

            let llvm_typeinfo = TypeinfoType::new(ctx).alloca_ty(ctx).into_struct_type();

            let value = ctx.module.add_global(llvm_typeinfo, None, &format!("typeinfo for {NAME}"));
            value.set_initializer(
                &llvm_typeinfo.const_named_struct(&[
                    name.as_pointer_value()
                        .const_cast(ctx.i8.ptr_type(AddressSpace::default()))
                        .into(),
                    refcounted_field_offsets
                        .as_pointer_value()
                        .const_cast(ctx.i32.ptr_type(AddressSpace::default()))
                        .into(),
                ]),
            );
            value.set_linkage(Linkage::WeakAny);
            value.set_constant(true);

            value
        });
        TypeinfoType::new(ctx).map_value(global.as_pointer_value(), None)
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
        let inner_ptr = self.inner_ptr(ctx)?;
        let inner_ptr = ctx.builder.build_pointer_cast(
            inner_ptr,
            RefCountedArrayType::new(ctx, self.ty.elem, None).llvm_ty(ctx).into_pointer_type(),
            "",
        )?;
        Ok(unsafe {
            ctx.build_gep_and_load(
                inner_ptr,
                &[ctx.size_t.const_zero(), ctx.i32.const_zero()],
                None,
            )?
            .into_int_value()
        })
    }

    pub fn inner_value(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, T>> {
        let data = unsafe {
            ctx.builder.build_in_bounds_gep(
                self.inner_ptr(ctx)?,
                &[ctx.size_t.const_zero(), ctx.i32.const_int(1, false)],
                "",
            )?
        };
        let data = ctx.builder.build_pointer_cast(
            data,
            self.ty.elem.llvm_ty(ctx).ptr_type(AddressSpace::default()),
            "",
        )?;

        Ok(ArraySliceValue::new(self.ty.elem, data, self.len(ctx)?, self.name))
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
        let obj_header = ctx.builder.build_pointer_cast(self.value, ctx.ptr, "")?;
        Ok(unsafe {
            ctx.builder.build_gep(
                obj_header,
                &[ObjectHeaderType::new(ctx)
                    .llvm_ty(ctx)
                    .size_of()
                    .map(|sizeof| sizeof.const_cast(ctx.size_t, false))
                    .unwrap()],
                "",
            )?
        })
    }
}
