use itertools::Itertools;

use crate::codegen::{
    irrt::{call_nac3_ndarray_broadcast_shapes, call_nac3_ndarray_broadcast_to},
    model::*,
    CodeGenContext, CodeGenerator,
};

use super::NDArrayObject;

/// Fields of [`ShapeEntry`]
pub struct ShapeEntryFields<'ctx, F: FieldTraversal<'ctx>> {
    pub ndims: F::Out<Int<SizeT>>,
    pub shape: F::Out<Ptr<Int<SizeT>>>,
}

/// An IRRT structure used in broadcasting.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShapeEntry;

impl<'ctx> StructKind<'ctx> for ShapeEntry {
    type Fields<F: FieldTraversal<'ctx>> = ShapeEntryFields<'ctx, F>;

    fn traverse_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields { ndims: traversal.add_auto("ndims"), shape: traversal.add_auto("shape") }
    }
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Create a broadcast view on this ndarray with a target shape.
    ///
    /// The input shape will be checked to make sure that it contains no negative values.
    ///
    /// * `target_ndims` - The ndims type after broadcasting to the given shape.
    ///   The caller has to figure this out for this function.
    /// * `target_shape` - An array pointer pointing to the target shape.
    #[must_use]
    pub fn broadcast_to<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        target_ndims: u64,
        target_shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> Self {
        let broadcast_ndarray = NDArrayObject::alloca(generator, ctx, self.dtype, target_ndims);
        broadcast_ndarray.copy_shape_from_array(generator, ctx, target_shape);

        call_nac3_ndarray_broadcast_to(generator, ctx, self.instance, broadcast_ndarray.instance);
        broadcast_ndarray
    }
}
/// A result produced by [`broadcast_all_ndarrays`]
#[derive(Debug, Clone)]
pub struct BroadcastAllResult<'ctx> {
    /// The statically known `ndims` of the broadcast result.
    pub ndims: u64,
    /// The broadcasting shape.
    pub shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    /// Broadcasted views on the inputs.
    ///
    /// All of them will have `shape` [`BroadcastAllResult::shape`] and
    /// `ndims` [`BroadcastAllResult::ndims`]. The length of the vector
    /// is the same as the input.
    pub ndarrays: Vec<NDArrayObject<'ctx>>,
}

/// Helper function to call `call_nac3_ndarray_broadcast_shapes`
fn broadcast_shapes<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    in_shape_entries: &[(Instance<'ctx, Ptr<Int<SizeT>>>, u64)], // (shape, shape's length/ndims)
    broadcast_ndims: u64,
    broadcast_shape: Instance<'ctx, Ptr<Int<SizeT>>>,
) {
    // Prepare input shape entries to be passed to `call_nac3_ndarray_broadcast_shapes`.
    let num_shape_entries =
        Int(SizeT).const_int(generator, ctx.ctx, u64::try_from(in_shape_entries.len()).unwrap());
    let shape_entries = Struct(ShapeEntry).array_alloca(generator, ctx, num_shape_entries.value);
    for (i, (in_shape, in_ndims)) in in_shape_entries.iter().enumerate() {
        let pshape_entry = shape_entries.offset_const(ctx, i as u64);

        let in_ndims = Int(SizeT).const_int(generator, ctx.ctx, *in_ndims);
        pshape_entry.set(ctx, |f| f.ndims, in_ndims);

        pshape_entry.set(ctx, |f| f.shape, *in_shape);
    }

    let broadcast_ndims = Int(SizeT).const_int(generator, ctx.ctx, broadcast_ndims);
    call_nac3_ndarray_broadcast_shapes(
        generator,
        ctx,
        num_shape_entries,
        shape_entries,
        broadcast_ndims,
        broadcast_shape,
    );
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Broadcast all ndarrays according to `np.broadcast()` and return a [`BroadcastAllResult`]
    /// containing all the information of the result of the broadcast operation.
    pub fn broadcast<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarrays: &[Self],
    ) -> BroadcastAllResult<'ctx> {
        assert!(!ndarrays.is_empty());

        // Infer the broadcast output ndims.
        let broadcast_ndims_int = ndarrays.iter().map(|ndarray| ndarray.ndims).max().unwrap();

        let broadcast_ndims = Int(SizeT).const_int(generator, ctx.ctx, broadcast_ndims_int);
        let broadcast_shape = Int(SizeT).array_alloca(generator, ctx, broadcast_ndims.value);

        let shape_entries = ndarrays
            .iter()
            .map(|ndarray| (ndarray.instance.get(generator, ctx, |f| f.shape), ndarray.ndims))
            .collect_vec();
        broadcast_shapes(generator, ctx, &shape_entries, broadcast_ndims_int, broadcast_shape);

        // Broadcast all the inputs to shape `dst_shape`.
        let broadcast_ndarrays: Vec<_> = ndarrays
            .iter()
            .map(|ndarray| {
                ndarray.broadcast_to(generator, ctx, broadcast_ndims_int, broadcast_shape)
            })
            .collect_vec();

        BroadcastAllResult {
            ndims: broadcast_ndims_int,
            shape: broadcast_shape,
            ndarrays: broadcast_ndarrays,
        }
    }
}
