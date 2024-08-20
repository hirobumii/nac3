use inkwell::values::StructValue;
use itertools::Itertools;

use super::any::AnyObject;
use crate::{
    codegen::{model::*, CodeGenContext, CodeGenerator},
    typecheck::typedef::{Type, TypeEnum},
};

/// A NAC3 tuple object.
///
/// NOTE: This struct has no copy trait.
#[derive(Debug, Clone)]
pub struct TupleObject<'ctx> {
    /// The type of the tuple.
    pub tys: Vec<Type>,
    /// The underlying LLVM struct value of this tuple.
    pub value: StructValue<'ctx>,
}

impl<'ctx> TupleObject<'ctx> {
    pub fn from_object(ctx: &mut CodeGenContext<'ctx, '_>, object: AnyObject<'ctx>) -> Self {
        // TODO: Keep `is_vararg_ctx` from TTuple?

        // Sanity check on object type.
        let TypeEnum::TTuple { ty: tys, .. } = &*ctx.unifier.get_ty(object.ty) else {
            panic!(
                "Expected type to be a TypeEnum::TTuple, got {}",
                ctx.unifier.stringify(object.ty)
            );
        };

        // Check number of fields
        let value = object.value.into_struct_value();
        let value_num_fields = value.get_type().count_fields() as usize;
        assert!(
            value_num_fields == tys.len(),
            "Tuple type has {} item(s), but the LLVM struct value has {} field(s)",
            tys.len(),
            value_num_fields
        );

        TupleObject { tys: tys.clone(), value }
    }

    /// Convenience function. Create a [`TupleObject`] from an iterator of objects.
    pub fn from_objects<I, G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        objects: I,
    ) -> Self
    where
        I: IntoIterator<Item = AnyObject<'ctx>>,
    {
        let (values, tys): (Vec<_>, Vec<_>) =
            objects.into_iter().map(|object| (object.value, object.ty)).unzip();

        let llvm_tys = tys.iter().map(|ty| ctx.get_llvm_type(generator, *ty)).collect_vec();
        let llvm_tuple_ty = ctx.ctx.struct_type(&llvm_tys, false);

        let pllvm_tuple = ctx.builder.build_alloca(llvm_tuple_ty, "tuple").unwrap();
        for (i, val) in values.into_iter().enumerate() {
            let pval = ctx.builder.build_struct_gep(pllvm_tuple, i as u32, "value").unwrap();
            ctx.builder.build_store(pval, val).unwrap();
        }

        let value = ctx.builder.build_load(pllvm_tuple, "").unwrap().into_struct_value();
        TupleObject { tys, value }
    }

    #[must_use]
    pub fn num_elements(&self) -> usize {
        self.tys.len()
    }

    /// Get the `len()` of this tuple.
    #[must_use]
    pub fn len<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        Int(SizeT).const_int(generator, ctx.ctx, self.num_elements() as u64, false)
    }

    /// Get the `i`-th (0-based) object in this tuple.
    pub fn index(&self, ctx: &mut CodeGenContext<'ctx, '_>, i: usize) -> AnyObject<'ctx> {
        assert!(
            i < self.num_elements(),
            "Tuple object with length {} have index {i}",
            self.num_elements()
        );

        let value = ctx.builder.build_extract_value(self.value, i as u32, "tuple[{i}]").unwrap();
        let ty = self.tys[i];
        AnyObject { ty, value }
    }
}
