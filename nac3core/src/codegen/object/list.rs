use crate::{
    codegen::{model::*, CodeGenContext, CodeGenerator},
    typecheck::typedef::{iter_type_vars, Type, TypeEnum},
};

use super::any::AnyObject;

/// Fields of [`List`]
pub struct ListFields<'ctx, F: FieldTraversal<'ctx>, Item: Model<'ctx>> {
    /// Array pointer to content
    pub items: F::Out<Ptr<Item>>,
    /// Number of items in the array
    pub len: F::Out<Int<SizeT>>,
}

/// A list in NAC3.
#[derive(Debug, Clone, Copy, Default)]
pub struct List<Item> {
    /// Model of the list items
    pub item: Item,
}

impl<'ctx, Item: Model<'ctx>> StructKind<'ctx> for List<Item> {
    type Fields<F: FieldTraversal<'ctx>> = ListFields<'ctx, F, Item>;

    fn traverse_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields {
            items: traversal.add("items", Ptr(self.item)),
            len: traversal.add_auto("len"),
        }
    }
}

/// A NAC3 Python List object.
#[derive(Debug, Clone, Copy)]
pub struct ListObject<'ctx> {
    /// Typechecker type of the list items
    pub item_type: Type,
    pub instance: Instance<'ctx, Ptr<Struct<List<Any<'ctx>>>>>,
}

impl<'ctx> ListObject<'ctx> {
    /// Create a [`ListObject`] from an LLVM value and its typechecker [`Type`].
    pub fn from_object<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        object: AnyObject<'ctx>,
    ) -> Self {
        // Check typechecker type and extract `item_type`
        let item_type = match &*ctx.unifier.get_ty(object.ty) {
            TypeEnum::TObj { obj_id, params, .. }
                if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
            {
                iter_type_vars(params).next().unwrap().ty // Extract `item_type`
            }
            _ => {
                panic!("Expecting type to be a list, but got {}", ctx.unifier.stringify(object.ty))
            }
        };

        let plist = Ptr(Struct(List { item: Any(ctx.get_llvm_type(generator, item_type)) }));

        // Create object
        let value = plist.check_value(generator, ctx.ctx, object.value).unwrap();
        ListObject { item_type, instance: value }
    }

    /// Get the `len()` of this list.
    pub fn len<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        self.instance.get(generator, ctx, |f| f.len)
    }
}
