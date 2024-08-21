use super::NDArrayObject;
use crate::codegen::{
    irrt::call_nac3_ndarray_index,
    model::*,
    object::utils::slice::{RustSlice, Slice},
    CodeGenContext, CodeGenerator,
};

pub type NDIndexType = Byte;

/// Fields of [`NDIndex`]
#[derive(Debug, Clone, Copy)]
pub struct NDIndexFields<'ctx, F: FieldTraversal<'ctx>> {
    pub type_: F::Output<Int<NDIndexType>>,
    pub data: F::Output<Ptr<Int<Byte>>>,
}

/// An IRRT representation of an ndarray subscript index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NDIndex;

impl<'ctx> StructKind<'ctx> for NDIndex {
    type Fields<F: FieldTraversal<'ctx>> = NDIndexFields<'ctx, F>;

    fn iter_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields { type_: traversal.add_auto("type"), data: traversal.add_auto("data") }
    }
}

// A convenience enum representing a [`NDIndex`].
#[derive(Debug, Clone)]
pub enum RustNDIndex<'ctx> {
    SingleElement(Instance<'ctx, Int<Int32>>),
    Slice(RustSlice<'ctx, Int32>),
    NewAxis,
    Ellipsis,
}

impl<'ctx> RustNDIndex<'ctx> {
    /// Get the value to set `NDIndex::type` for this variant.
    fn get_type_id(&self) -> u64 {
        // Defined in IRRT, must be in sync
        match self {
            RustNDIndex::SingleElement(_) => 0,
            RustNDIndex::Slice(_) => 1,
            RustNDIndex::NewAxis => 2,
            RustNDIndex::Ellipsis => 3,
        }
    }

    /// Serialize this [`RustNDIndex`] by writing it into an LLVM [`NDIndex`].
    fn write_to_ndindex<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        dst_ndindex_ptr: Instance<'ctx, Ptr<Struct<NDIndex>>>,
    ) {
        // Set `dst_ndindex_ptr->type`
        dst_ndindex_ptr.gep(ctx, |f| f.type_).store(
            ctx,
            Int(NDIndexType::default()).const_int(generator, ctx.ctx, self.get_type_id(), false),
        );

        // Set `dst_ndindex_ptr->data`
        match self {
            RustNDIndex::SingleElement(in_index) => {
                let index_ptr = Int(Int32).alloca(generator, ctx);
                index_ptr.store(ctx, *in_index);

                dst_ndindex_ptr
                    .gep(ctx, |f| f.data)
                    .store(ctx, index_ptr.pointer_cast(generator, ctx, Int(Byte)));
            }
            RustNDIndex::Slice(in_rust_slice) => {
                let user_slice_ptr = Struct(Slice(Int32)).alloca(generator, ctx);
                in_rust_slice.write_to_slice(generator, ctx, user_slice_ptr);

                dst_ndindex_ptr
                    .gep(ctx, |f| f.data)
                    .store(ctx, user_slice_ptr.pointer_cast(generator, ctx, Int(Byte)));
            }
            RustNDIndex::NewAxis | RustNDIndex::Ellipsis => {}
        }
    }

    /// Serialize a list of `RustNDIndex` as a newly allocated LLVM array of `NDIndex`.
    pub fn make_ndindices<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        in_ndindices: &[RustNDIndex<'ctx>],
    ) -> (Instance<'ctx, Int<SizeT>>, Instance<'ctx, Ptr<Struct<NDIndex>>>) {
        let ndindex_model = Struct(NDIndex);

        // Allocate the LLVM ndindices.
        let num_ndindices =
            Int(SizeT).const_int(generator, ctx.ctx, in_ndindices.len() as u64, false);
        let ndindices = ndindex_model.array_alloca(generator, ctx, num_ndindices.value);

        // Initialize all of them.
        for (i, in_ndindex) in in_ndindices.iter().enumerate() {
            let pndindex = ndindices.offset_const(ctx, i64::try_from(i).unwrap());
            in_ndindex.write_to_ndindex(generator, ctx, pndindex);
        }

        (num_ndindices, ndindices)
    }
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Get the expected `ndims` after indexing with `indices`.
    #[must_use]
    fn deduce_ndims_after_indexing_with(&self, indices: &[RustNDIndex<'ctx>]) -> u64 {
        let mut ndims = self.ndims;
        for index in indices {
            match index {
                RustNDIndex::SingleElement(_) => {
                    ndims -= 1; // Single elements decrements ndims
                }
                RustNDIndex::NewAxis => {
                    ndims += 1; // `np.newaxis` / `none` adds a new axis
                }
                RustNDIndex::Ellipsis | RustNDIndex::Slice(_) => {}
            }
        }
        ndims
    }

    /// Index into the ndarray, and return a newly-allocated view on this ndarray.
    ///
    /// This function behaves like NumPy's ndarray indexing, but if the indices index
    /// into a single element, an unsized ndarray is returned.
    #[must_use]
    pub fn index<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        indices: &[RustNDIndex<'ctx>],
    ) -> Self {
        let dst_ndims = self.deduce_ndims_after_indexing_with(indices);
        let dst_ndarray = NDArrayObject::alloca(generator, ctx, self.dtype, dst_ndims);

        let (num_indices, indices) = RustNDIndex::make_ndindices(generator, ctx, indices);
        call_nac3_ndarray_index(
            generator,
            ctx,
            num_indices,
            indices,
            self.instance,
            dst_ndarray.instance,
        );

        dst_ndarray
    }
}

pub mod util {
    use itertools::Itertools;
    use nac3parser::ast::{Expr, ExprKind};

    use crate::{
        codegen::{model::*, object::utils::slice::util::gen_slice, CodeGenContext, CodeGenerator},
        typecheck::typedef::Type,
    };

    use super::RustNDIndex;

    /// Generate LLVM code to transform an ndarray subscript expression to
    /// its list of [`RustNDIndex`]
    ///
    /// i.e.,
    /// ```python
    /// my_ndarray[::3, 1, :2:]
    ///            ^^^^^^^^^^^ Then these into a three `RustNDIndex`es
    /// ```
    pub fn gen_ndarray_subscript_ndindices<'ctx, G: CodeGenerator>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        subscript: &Expr<Option<Type>>,
    ) -> Result<Vec<RustNDIndex<'ctx>>, String> {
        // TODO: Support https://numpy.org/doc/stable/user/basics.indexing.html#dimensional-indexing-tools

        // Annoying notes about `slice`
        //  - `my_array[5]`
        //    - slice is a `Constant`
        //  - `my_array[:5]`
        //    - slice is a `Slice`
        //  - `my_array[:]`
        //    - slice is a `Slice`, but lower upper step would all be `Option::None`
        //  - `my_array[:, :]`
        //    - slice is now a `Tuple` of two `Slice`-s
        //
        // In summary:
        //  - when there is a comma "," within [], `slice` will be a `Tuple` of the entries.
        //  - when there is not comma "," within [] (i.e., just a single entry), `slice` will be that entry itself.
        //
        // So we first "flatten" out the slice expression
        let index_exprs = match &subscript.node {
            ExprKind::Tuple { elts, .. } => elts.iter().collect_vec(),
            _ => vec![subscript],
        };

        // Process all index expressions
        let mut rust_ndindices: Vec<RustNDIndex> = Vec::with_capacity(index_exprs.len()); // Not using iterators here because `?` is used here.
        for index_expr in index_exprs {
            // NOTE: Currently nac3core's slices do not have an object representation,
            // so the code/implementation looks awkward - we have to do pattern matching on the expression
            let ndindex = if let ExprKind::Slice { lower, upper, step } = &index_expr.node {
                // Handle slices
                let slice = gen_slice(generator, ctx, lower, upper, step)?;
                RustNDIndex::Slice(slice)
            } else {
                // Treat and handle everything else as a single element index.
                let index = generator.gen_expr(ctx, index_expr)?.unwrap().to_basic_value_enum(
                    ctx,
                    generator,
                    ctx.primitives.int32, // Must be int32, this checks for illegal values
                )?;
                let index = Int(Int32).check_value(generator, ctx.ctx, index).unwrap();

                RustNDIndex::SingleElement(index)
            };
            rust_ndindices.push(ndindex);
        }
        Ok(rust_ndindices)
    }
}
