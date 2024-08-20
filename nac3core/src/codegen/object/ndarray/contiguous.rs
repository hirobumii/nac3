use crate::{
    codegen::{model::*, CodeGenContext, CodeGenerator},
    typecheck::typedef::Type,
};

use super::NDArrayObject;

/// Fields of [`ContiguousNDArray`]
pub struct ContiguousNDArrayFields<'ctx, F: FieldTraversal<'ctx>, Item: Model<'ctx>> {
    pub ndims: F::Out<Int<SizeT>>,
    pub shape: F::Out<Ptr<Int<SizeT>>>,
    pub data: F::Out<Ptr<Item>>,
}

/// An ndarray without strides and non-opaque `data` field in NAC3.
#[derive(Debug, Clone, Copy)]
pub struct ContiguousNDArray<M> {
    /// [`Model`] of the items.
    pub item: M,
}

impl<'ctx, Item: Model<'ctx>> StructKind<'ctx> for ContiguousNDArray<Item> {
    type Fields<F: FieldTraversal<'ctx>> = ContiguousNDArrayFields<'ctx, F, Item>;

    fn traverse_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields {
            ndims: traversal.add_auto("ndims"),
            shape: traversal.add_auto("shape"),
            data: traversal.add("data", Ptr(self.item)),
        }
    }
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Create a [`ContiguousNDArray`] from the contents of this ndarray.
    ///
    /// This function may or may not be expensive depending on if this ndarray has contiguous data.
    ///
    /// If this ndarray is not C-contiguous, this function will allocate memory on the stack for the `data` field of
    /// the returned [`ContiguousNDArray`] and copy contents of this ndarray to there.
    ///
    /// If this ndarray is C-contiguous, contents of this ndarray will not be copied. The created [`ContiguousNDArray`]
    /// will share memory with this ndarray.
    ///
    /// The `item_model` sets the [`Model`] of the returned [`ContiguousNDArray`]'s `Item` model for type-safety, and
    /// should match the `ctx.get_llvm_type()` of this ndarray's `dtype`. Otherwise this function panics. Use model [`Any`]
    /// if you don't care/cannot know the [`Model`] in advance.
    pub fn make_contiguous_ndarray<G: CodeGenerator + ?Sized, Item: Model<'ctx>>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        item_model: Item,
    ) -> Instance<'ctx, Ptr<Struct<ContiguousNDArray<Item>>>> {
        // Sanity check on `self.dtype` and `item_model`.
        let dtype_llvm = ctx.get_llvm_type(generator, self.dtype);
        item_model.check_type(generator, ctx.ctx, dtype_llvm).unwrap();

        let cdarray_model = Struct(ContiguousNDArray { item: item_model });

        let current_bb = ctx.builder.get_insert_block().unwrap();
        let then_bb = ctx.ctx.insert_basic_block_after(current_bb, "then_bb");
        let else_bb = ctx.ctx.insert_basic_block_after(then_bb, "else_bb");
        let end_bb = ctx.ctx.insert_basic_block_after(else_bb, "end_bb");

        // Allocate and setup the resulting [`ContiguousNDArray`].
        let result = cdarray_model.alloca(generator, ctx);

        // Set ndims and shape.
        let ndims = self.ndims_llvm(generator, ctx.ctx);
        result.set(ctx, |f| f.ndims, ndims);

        let shape = self.instance.get(generator, ctx, |f| f.shape);
        result.set(ctx, |f| f.shape, shape);

        let is_contiguous = self.is_c_contiguous(generator, ctx);
        ctx.builder.build_conditional_branch(is_contiguous.value, then_bb, else_bb).unwrap();

        // Inserting into then_bb; This ndarray is contiguous.
        ctx.builder.position_at_end(then_bb);
        let data = self.instance.get(generator, ctx, |f| f.data);
        let data = data.pointer_cast(generator, ctx, item_model);
        result.set(ctx, |f| f.data, data);
        ctx.builder.build_unconditional_branch(end_bb).unwrap();

        // Inserting into else_bb; This ndarray is not contiguous. Do a full-copy on `data`.
        // `make_copy` produces an ndarray with contiguous `data`.
        ctx.builder.position_at_end(else_bb);
        let copied_ndarray = self.make_copy(generator, ctx);
        let data = copied_ndarray.instance.get(generator, ctx, |f| f.data);
        let data = data.pointer_cast(generator, ctx, item_model);
        result.set(ctx, |f| f.data, data);
        ctx.builder.build_unconditional_branch(end_bb).unwrap();

        // Reposition to end_bb for continuation
        ctx.builder.position_at_end(end_bb);

        result
    }

    /// Create an [`NDArrayObject`] from a [`ContiguousNDArray`].
    ///
    /// The operation is super cheap. The newly created [`NDArrayObject`] will share the
    /// same memory as the [`ContiguousNDArray`].
    ///
    /// `ndims` has to be provided as [`NDArrayObject`] requires a statically known `ndims` value, despite
    /// the fact that the information should be contained within the [`ContiguousNDArray`].
    pub fn from_contiguous_ndarray<G: CodeGenerator + ?Sized, Item: Model<'ctx>>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        carray: Instance<'ctx, Ptr<Struct<ContiguousNDArray<Item>>>>,
        dtype: Type,
        ndims: u64,
    ) -> Self {
        // Sanity check on `dtype` and `contiguous_array`'s `Item` model.
        let dtype_llvm = ctx.get_llvm_type(generator, dtype);
        carray.model.0 .0.item.check_type(generator, ctx.ctx, dtype_llvm).unwrap();

        // TODO: Debug assert `ndims == carray.ndims` to catch bugs.

        // Allocate the resulting ndarray.
        let ndarray = NDArrayObject::alloca(generator, ctx, dtype, ndims);

        // Copy shape and update strides
        let shape = carray.get(generator, ctx, |f| f.shape);
        ndarray.copy_shape_from_array(generator, ctx, shape);
        ndarray.set_strides_contiguous(generator, ctx);

        // Share data
        let data = carray.get(generator, ctx, |f| f.data).pointer_cast(generator, ctx, Int(Byte));
        ndarray.instance.set(ctx, |f| f.data, data);

        ndarray
    }
}
