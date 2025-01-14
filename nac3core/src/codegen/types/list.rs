use inkwell::{
    context::{AsContextRef, Context},
    types::{AnyTypeEnum, BasicType, BasicTypeEnum, IntType, PointerType},
    values::{IntValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use itertools::Itertools;

use nac3core_derive::StructFields;

use super::ProxyType;
use crate::{
    codegen::{
        types::structure::{
            check_struct_type_matches_fields, FieldIndexCounter, StructField, StructFields,
        },
        values::{ListValue, ProxyValue},
        CodeGenContext, CodeGenerator,
    },
    typecheck::typedef::{iter_type_vars, Type, TypeEnum},
};

/// Proxy type for a `list` type in LLVM.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ListType<'ctx> {
    ty: PointerType<'ctx>,
    item: Option<BasicTypeEnum<'ctx>>,
    llvm_usize: IntType<'ctx>,
}

#[derive(PartialEq, Eq, Clone, Copy, StructFields)]
pub struct ListStructFields<'ctx> {
    /// Array pointer to content.
    #[value_type(i8_type().ptr_type(AddressSpace::default()))]
    pub items: StructField<'ctx, PointerValue<'ctx>>,

    /// Number of items in the array.
    #[value_type(usize)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

impl<'ctx> ListStructFields<'ctx> {
    #[must_use]
    pub fn new_typed(item: BasicTypeEnum<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        let mut counter = FieldIndexCounter::default();

        ListStructFields {
            items: StructField::create(
                &mut counter,
                "items",
                item.ptr_type(AddressSpace::default()),
            ),
            len: StructField::create(&mut counter, "len", llvm_usize),
        }
    }
}

impl<'ctx> ListType<'ctx> {
    /// Checks whether `llvm_ty` represents a `list` type, returning [Err] if it does not.
    pub fn is_representable(
        llvm_ty: PointerType<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let ctx = llvm_ty.get_context();

        let llvm_ty = llvm_ty.get_element_type();
        let AnyTypeEnum::StructType(llvm_ty) = llvm_ty else {
            return Err(format!("Expected struct type for `list` type, got {llvm_ty}"));
        };

        let fields = ListStructFields::new(ctx, llvm_usize);

        check_struct_type_matches_fields(
            fields,
            llvm_ty,
            "list",
            &[(fields.items.name(), &|ty| {
                if ty.is_pointer_type() {
                    Ok(())
                } else {
                    Err(format!("Expected T* for `list.items`, got {ty}"))
                }
            })],
        )
    }

    /// Returns an instance of [`StructFields`] containing all field accessors for this type.
    #[must_use]
    fn fields(item: BasicTypeEnum<'ctx>, llvm_usize: IntType<'ctx>) -> ListStructFields<'ctx> {
        ListStructFields::new_typed(item, llvm_usize)
    }

    /// See [`ListType::fields`].
    // TODO: Move this into e.g. StructProxyType
    #[must_use]
    pub fn get_fields(&self, _ctx: &impl AsContextRef<'ctx>) -> ListStructFields<'ctx> {
        Self::fields(self.item.unwrap_or(self.llvm_usize.into()), self.llvm_usize)
    }

    /// Creates an LLVM type corresponding to the expected structure of a `List`.
    #[must_use]
    fn llvm_type(
        ctx: &'ctx Context,
        element_type: Option<BasicTypeEnum<'ctx>>,
        llvm_usize: IntType<'ctx>,
    ) -> PointerType<'ctx> {
        let element_type = element_type.map_or(llvm_usize.into(), |ty| ty.as_basic_type_enum());

        let field_tys =
            Self::fields(element_type, llvm_usize).into_iter().map(|field| field.1).collect_vec();

        ctx.struct_type(&field_tys, false).ptr_type(AddressSpace::default())
    }

    fn new_impl(
        ctx: &'ctx Context,
        element_type: Option<BasicTypeEnum<'ctx>>,
        llvm_usize: IntType<'ctx>,
    ) -> Self {
        let llvm_list = Self::llvm_type(ctx, element_type, llvm_usize);

        Self { ty: llvm_list, item: element_type, llvm_usize }
    }

    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new(ctx: &CodeGenContext<'ctx, '_>, element_type: &impl BasicType<'ctx>) -> Self {
        Self::new_impl(ctx.ctx, Some(element_type.as_basic_type_enum()), ctx.get_size_type())
    }

    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        element_type: BasicTypeEnum<'ctx>,
    ) -> Self {
        Self::new_impl(ctx, Some(element_type.as_basic_type_enum()), generator.get_size_type(ctx))
    }

    /// Creates an instance of [`ListType`] with an unknown element type.
    #[must_use]
    pub fn new_untyped(ctx: &CodeGenContext<'ctx, '_>) -> Self {
        Self::new_impl(ctx.ctx, None, ctx.get_size_type())
    }

    /// Creates an instance of [`ListType`] with an unknown element type.
    #[must_use]
    pub fn new_untyped_with_generator<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
    ) -> Self {
        Self::new_impl(ctx, None, generator.get_size_type(ctx))
    }

    /// Creates an [`ListType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ty: Type,
    ) -> Self {
        // Check unifier type and extract `item_type`
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `list` type, but got {}", ctx.unifier.stringify(ty)),
        };

        let llvm_usize = ctx.get_size_type();
        let llvm_elem_type = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(ty) {
            None
        } else {
            Some(ctx.get_llvm_type(generator, elem_type))
        };

        Self::new_impl(ctx.ctx, llvm_elem_type, llvm_usize)
    }

    /// Creates an [`ListType`] from a [`PointerType`].
    #[must_use]
    pub fn from_type(ptr_ty: PointerType<'ctx>, llvm_usize: IntType<'ctx>) -> Self {
        debug_assert!(Self::is_representable(ptr_ty, llvm_usize).is_ok());

        let ctx = ptr_ty.get_context();

        // We are just searching for the index off a field - Slot an arbitrary element type in.
        let item_field_idx =
            Self::fields(ctx.i8_type().into(), llvm_usize).index_of_field(|f| f.items);
        let item = unsafe {
            ptr_ty
                .get_element_type()
                .into_struct_type()
                .get_field_type_at_index_unchecked(item_field_idx)
                .into_pointer_type()
                .get_element_type()
        };
        let item = BasicTypeEnum::try_from(item).unwrap_or_else(|()| {
            panic!(
                "Expected BasicTypeEnum for list element type, got {}",
                ptr_ty.get_element_type().print_to_string()
            )
        });

        ListType { ty: ptr_ty, item: Some(item), llvm_usize }
    }

    /// Returns the type of the `size` field of this `list` type.
    #[must_use]
    pub fn size_type(&self) -> IntType<'ctx> {
        self.llvm_usize
    }

    /// Returns the element type of this `list` type.
    #[must_use]
    pub fn element_type(&self) -> Option<BasicTypeEnum<'ctx>> {
        self.item
    }

    /// Allocates an instance of [`ListValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca`].
    #[must_use]
    pub fn alloca(
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

    /// Allocates an instance of [`ListValue`] as if by calling `alloca` on the base type.
    ///
    /// See [`ProxyType::raw_alloca_var`].
    #[must_use]
    pub fn alloca_var<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(
            self.raw_alloca_var(generator, ctx, name),
            self.llvm_usize,
            name,
        )
    }

    /// Allocates a [`ListValue`] on the stack using `item` of this [`ListType`] instance.
    ///
    /// The returned list will contain:
    ///
    /// - `data`: Allocated with `len` number of elements.
    /// - `len`: Initialized to the value of `len` passed to this function.
    #[must_use]
    pub fn construct<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let len = ctx.builder.build_int_z_extend(len, self.llvm_usize, "").unwrap();

        // Generate a runtime assertion if allocating a non-empty list with unknown element type
        if ctx.registry.llvm_options.opt_level == OptimizationLevel::None && self.item.is_none() {
            let len_eqz = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, len, self.llvm_usize.const_zero(), "")
                .unwrap();

            ctx.make_assert(
                generator,
                len_eqz,
                "0:AssertionError",
                "Cannot allocate a non-empty list with unknown element type",
                [None, None, None],
                ctx.current_loc,
            );
        }

        let plist = self.alloca_var(generator, ctx, name);
        plist.store_size(ctx, len);

        let item = self.item.unwrap_or(self.llvm_usize.into());
        plist.create_data(ctx, item, None);

        plist
    }

    /// Convenience function for creating a list with zero elements.
    ///
    /// This function is preferred over [`ListType::construct`] if the length is known to always be
    /// 0, as this function avoids injecting an IR assertion for checking if a non-empty untyped
    /// list is being allocated.
    ///
    /// The returned list will contain:
    ///
    /// - `data`: Initialized to `(T*) 0`.
    /// - `len`: Initialized to `0`.
    #[must_use]
    pub fn construct_empty<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        let plist = self.alloca_var(generator, ctx, name);

        plist.store_size(ctx, self.llvm_usize.const_zero());
        plist.create_data(ctx, self.item.unwrap_or(self.llvm_usize.into()), None);

        plist
    }

    /// Converts an existing value into a [`ListValue`].
    #[must_use]
    pub fn map_value(
        &self,
        value: <<Self as ProxyType<'ctx>>::Value as ProxyValue<'ctx>>::Base,
        name: Option<&'ctx str>,
    ) -> <Self as ProxyType<'ctx>>::Value {
        <Self as ProxyType<'ctx>>::Value::from_pointer_value(value, self.llvm_usize, name)
    }
}

impl<'ctx> ProxyType<'ctx> for ListType<'ctx> {
    type Base = PointerType<'ctx>;
    type Value = ListValue<'ctx>;

    fn is_type<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: impl BasicType<'ctx>,
    ) -> Result<(), String> {
        if let BasicTypeEnum::PointerType(ty) = llvm_ty.as_basic_type_enum() {
            <Self as ProxyType<'ctx>>::is_representable(generator, ctx, ty)
        } else {
            Err(format!("Expected pointer type, got {llvm_ty:?}"))
        }
    }

    fn is_representable<G: CodeGenerator + ?Sized>(
        generator: &G,
        ctx: &'ctx Context,
        llvm_ty: Self::Base,
    ) -> Result<(), String> {
        Self::is_representable(llvm_ty, generator.get_size_type(ctx))
    }

    fn alloca_type(&self) -> impl BasicType<'ctx> {
        self.as_base_type().get_element_type().into_struct_type()
    }

    fn as_base_type(&self) -> Self::Base {
        self.ty
    }
}

impl<'ctx> From<ListType<'ctx>> for PointerType<'ctx> {
    fn from(value: ListType<'ctx>) -> Self {
        value.as_base_type()
    }
}
