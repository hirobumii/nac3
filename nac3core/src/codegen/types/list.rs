use inkwell::{
    AddressSpace, IntPredicate,
    module::Linkage,
    values::{IntValue, PointerValue},
};
use itertools::Itertools as _;
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        AllocationScope, CodeGenContext, ModuleContext,
        types::{
            ProxyType, ProxyTypeBase, RefCountedArrayType, RefType, StringType,
            TypedRefCountedType, TypedRefCountedValue, TypeinfoType, TypeinfoValue, Value,
            WithTypeinfo, array::ArraySliceValue, builtin::BuiltinStruct, field,
            structure::StructField,
        },
    },
    typecheck::typedef::{Type, TypeEnum, iter_type_vars},
};

#[derive(Clone, Copy, StructFields)]
pub struct ListStructFields<'ctx> {
    /// Array pointer to content.
    #[value_type(ptr)]
    pub items: StructField<'ctx, PointerValue<'ctx>>,

    /// Number of items in the array.
    #[value_type(size_t)]
    pub len: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct ListType<'ctx> {
    pub inner: BuiltinStruct<'ctx, ListStructFields<'ctx>>,
    pub item_ty: Type,
}

impl<'ctx> ListType<'ctx> {
    /// Creates an instance of [`ListType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, item_ty: Type) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "list"), item_ty }
    }

    /// Creates an [`ListType`] from a [unifier type][Type].
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        // Check unifier type and extract `item_type`
        let elem_type = match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty
            }

            _ => panic!("Expected `list` type, but got {}", ctx.unifier.stringify(ty)),
        };

        Self::new(ctx, elem_type)
    }

    /// Allocates a new list with the given length.
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        len: IntValue<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<TypedRefCountedValue<'ctx, Self>> {
        let list = TypedRefCountedType::new(ctx, *self).allocate(
            ctx,
            AllocationScope::Default,
            true,
            name,
        )?;

        let len = ctx.builder.build_int_z_extend(len, ctx.size_t, "")?;
        list.inner_value(ctx)?.store(ctx, field!(len), len)?;

        let len_eqz =
            ctx.builder.build_int_compare(IntPredicate::EQ, len, ctx.size_t.const_zero(), "")?;
        let null = ctx.ptr.const_null();

        let data = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.item_ty) {
            // Generate a runtime assertion if allocating a non-empty list with unknown element type
            if ctx.registry.codegen_options.debug {
                ctx.make_assert(
                    len_eqz,
                    "0:AssertionError",
                    "Cannot allocate a non-empty list with unknown element type",
                    [None, None, None],
                    ctx.current_loc,
                )?;
            }
            null
        } else {
            let ty = ctx.get_llvm_type(self.item_ty);
            let array = RefCountedArrayType::new(ctx, ty, None).allocate(ctx, len, None)?.value;
            ctx.builder.build_select(len_eqz, null, array, "")?.into_pointer_value()
        };

        list.inner_value(ctx)?.store(ctx, field!(items), data)?;
        Ok(list)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for ListType<'ctx> {
    fn typeinfo(ctx: &ModuleContext<'ctx>) -> TypeinfoValue<'ctx> {
        const NAME: &str = "__nac3_list";

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

            let refcounted_field_offsets = ctx
                .module
                .get_global(&format!("refcounted_fields array for {NAME}"))
                .unwrap_or_else(|| {
                    let refcounted_field_offsets = ctx.module.add_global(
                        ctx.i32.array_type(1),
                        None,
                        "refcounted_fields array for __nac3_list",
                    );
                    refcounted_field_offsets.set_linkage(Linkage::WeakAny);
                    refcounted_field_offsets
                        .set_initializer(&ctx.i32.const_array(&[ctx.i32.const_zero()]));
                    // refcounted_field_offsets.set_initializer(&ctx.i32.const_array(&[
                    //     ctx.i32.const_int(1, false),
                    //     unsafe {
                    //         let zero = self.as_base_type().const_null();
                    //         let begin_ptr = zero
                    //             .const_in_bounds_gep(&[ctx.size_t.const_zero(), ctx.i32.const_zero()])
                    //             .const_to_int(ctx.size_t)
                    //             .const_cast(ctx.i32, false);
                    //         let field_idx = self.get_fields().index_of_field(|f| f.items);
                    //         let field_ptr = zero
                    //             .const_in_bounds_gep(&[
                    //                 ctx.size_t.const_zero(),
                    //                 ctx.i32.const_int(u64::from(field_idx), false),
                    //             ])
                    //             .const_to_int(ctx.size_t)
                    //             .const_cast(ctx.i32, false);

                    //         field_ptr.const_sub(begin_ptr)
                    //     },
                    // ]));
                    refcounted_field_offsets.set_constant(true);

                    refcounted_field_offsets
                });

            let llvm_typeinfo = TypeinfoType::new(ctx).alloca_ty(ctx).into_struct_type();

            let value = ctx.module.add_global(llvm_typeinfo, None, &format!("typeinfo for {NAME}"));
            value.set_linkage(Linkage::WeakAny);
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
            value.set_constant(true);
            value
        });
        TypeinfoType::new(ctx).map_value(global.as_pointer_value(), None)
    }
}

pub type ListValue<'ctx> = Value<'ctx, ListType<'ctx>>;

impl<'ctx> ListValue<'ctx> {
    /// Returns the data of this list as an [`ArraySliceValue`].
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        let item_ty = if let TypeEnum::TVar { .. } = &*ctx.unifier.get_ty_immutable(self.ty.item_ty)
        {
            // Use a placeholder type.
            ctx.i8.into()
        } else {
            ctx.get_llvm_type(self.ty.item_ty)
        };

        let refcounted_array = RefCountedArrayType::new(ctx, item_ty, None)
            .map_value(self.load(ctx, field!(items))?, None);
        Ok(ArraySliceValue::new(
            item_ty,
            refcounted_array.inner_value(ctx)?.value.0,
            self.load(ctx, field!(len))?,
            self.name,
        ))
    }

    /// Creates an empty list with the given item type.
    ///
    /// This is special because `item_ty` can be anything, including completely
    /// unbounded type variables.
    pub fn new_empty(
        ctx: &mut CodeGenContext<'ctx, '_>,
        item_ty: Type,
        name: Option<&'static str>,
    ) -> anyhow::Result<TypedRefCountedValue<'ctx, ListType<'ctx>>> {
        let list_ty = ListType::new(ctx, item_ty);
        list_ty.construct(ctx, ctx.size_t.const_zero(), name)
    }
}
