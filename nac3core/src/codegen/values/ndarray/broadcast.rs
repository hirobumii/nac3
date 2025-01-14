use inkwell::{
    types::IntType,
    values::{IntValue, PointerValue},
};
use itertools::Itertools;

use crate::codegen::{
    irrt,
    types::{
        ndarray::{NDArrayType, ShapeEntryType},
        structure::StructField,
        ProxyType,
    },
    values::{
        ndarray::NDArrayValue, ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ProxyValue,
        TypedArrayLikeAccessor, TypedArrayLikeAdapter, TypedArrayLikeMutator,
    },
    CodeGenContext, CodeGenerator,
};

#[derive(Copy, Clone)]
pub struct ShapeEntryValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> ShapeEntryValue<'ctx> {
    /// Checks whether `value` is an instance of `ShapeEntry`, returning [Err] if `value` is
    /// not an instance.
    pub fn is_representable(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        <Self as ProxyValue<'ctx>>::Type::is_representable(value.get_type(), llvm_usize)
    }

    /// Creates an [`ShapeEntryValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_representable(ptr, llvm_usize).is_ok());

        Self { value: ptr, llvm_usize, name }
    }

    fn ndims_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields(self.value.get_type().get_context()).ndims
    }

    /// Stores the number of dimensions into this value.
    pub fn store_ndims(&self, ctx: &CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
        self.ndims_field().set(ctx, self.value, value, self.name);
    }

    fn shape_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields(self.value.get_type().get_context()).shape
    }

    /// Stores the shape into this value.
    pub fn store_shape(&self, ctx: &CodeGenContext<'ctx, '_>, value: PointerValue<'ctx>) {
        self.shape_field().set(ctx, self.value, value, self.name);
    }
}

impl<'ctx> ProxyValue<'ctx> for ShapeEntryValue<'ctx> {
    type Base = PointerValue<'ctx>;
    type Type = ShapeEntryType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }
}

impl<'ctx> From<ShapeEntryValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ShapeEntryValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

impl<'ctx> NDArrayValue<'ctx> {
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
        target_shape: &impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>,
    ) -> Self {
        assert!(self.ndims <= target_ndims);
        assert_eq!(target_shape.element_type(ctx, generator), self.llvm_usize.into());

        let broadcast_ndarray = NDArrayType::new(ctx, self.dtype, target_ndims)
            .construct_uninitialized(generator, ctx, None);
        broadcast_ndarray.copy_shape_from_array(
            generator,
            ctx,
            target_shape.base_ptr(ctx, generator),
        );

        irrt::ndarray::call_nac3_ndarray_broadcast_to(ctx, *self, broadcast_ndarray);
        broadcast_ndarray
    }
}

/// A result produced by [`broadcast_all_ndarrays`]
#[derive(Clone)]
pub struct BroadcastAllResult<'ctx, G: CodeGenerator + ?Sized> {
    /// The statically known `ndims` of the broadcast result.
    pub ndims: u64,

    /// The broadcasting shape.
    pub shape: TypedArrayLikeAdapter<'ctx, G, IntValue<'ctx>>,

    /// Broadcasted views on the inputs.
    ///
    /// All of them will have `shape` [`BroadcastAllResult::shape`] and
    /// `ndims` [`BroadcastAllResult::ndims`]. The length of the vector
    /// is the same as the input.
    pub ndarrays: Vec<NDArrayValue<'ctx>>,
}

/// Helper function to call [`irrt::ndarray::call_nac3_ndarray_broadcast_shapes`].
fn broadcast_shapes<'ctx, G, Shape>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    in_shape_entries: &[(ArraySliceValue<'ctx>, u64)], // (shape, shape's length/ndims)
    broadcast_ndims: u64,
    broadcast_shape: &Shape,
) where
    G: CodeGenerator + ?Sized,
    Shape: TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>>
        + TypedArrayLikeMutator<'ctx, G, IntValue<'ctx>>,
{
    let llvm_usize = ctx.get_size_type();
    let llvm_shape_ty = ShapeEntryType::new(ctx);

    assert!(in_shape_entries
        .iter()
        .all(|entry| entry.0.element_type(ctx, generator) == llvm_usize.into()));
    assert_eq!(broadcast_shape.element_type(ctx, generator), llvm_usize.into());

    // Prepare input shape entries to be passed to `call_nac3_ndarray_broadcast_shapes`.
    let num_shape_entries =
        llvm_usize.const_int(u64::try_from(in_shape_entries.len()).unwrap(), false);
    let shape_entries = llvm_shape_ty.array_alloca(ctx, num_shape_entries, None);
    for (i, (in_shape, in_ndims)) in in_shape_entries.iter().enumerate() {
        let pshape_entry = unsafe {
            shape_entries.ptr_offset_unchecked(
                ctx,
                generator,
                &llvm_usize.const_int(i as u64, false),
                None,
            )
        };
        let shape_entry = llvm_shape_ty.map_value(pshape_entry, None);

        let in_ndims = llvm_usize.const_int(*in_ndims, false);
        shape_entry.store_ndims(ctx, in_ndims);

        shape_entry.store_shape(ctx, in_shape.base_ptr(ctx, generator));
    }

    let broadcast_ndims = llvm_usize.const_int(broadcast_ndims, false);
    irrt::ndarray::call_nac3_ndarray_broadcast_shapes(
        generator,
        ctx,
        num_shape_entries,
        shape_entries,
        broadcast_ndims,
        broadcast_shape,
    );
}

impl<'ctx> NDArrayType<'ctx> {
    /// Broadcast all ndarrays according to
    /// [`np.broadcast()`](https://numpy.org/doc/stable/reference/generated/numpy.broadcast.html)
    /// and return a [`BroadcastAllResult`] containing all the information of the result of the
    /// broadcast operation.
    pub fn broadcast<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndarrays: &[NDArrayValue<'ctx>],
    ) -> BroadcastAllResult<'ctx, G> {
        assert!(!ndarrays.is_empty());

        let llvm_usize = ctx.get_size_type();

        // Infer the broadcast output ndims.
        let broadcast_ndims_int =
            ndarrays.iter().map(|ndarray| ndarray.get_type().ndims()).max().unwrap();
        assert!(self.ndims() >= broadcast_ndims_int);

        let broadcast_ndims = llvm_usize.const_int(broadcast_ndims_int, false);
        let broadcast_shape = ArraySliceValue::from_ptr_val(
            ctx.builder.build_array_alloca(llvm_usize, broadcast_ndims, "").unwrap(),
            broadcast_ndims,
            None,
        );
        let broadcast_shape = TypedArrayLikeAdapter::from(
            broadcast_shape,
            |_, _, val| val.into_int_value(),
            |_, _, val| val.into(),
        );

        let shape_entries = ndarrays
            .iter()
            .map(|ndarray| {
                (ndarray.shape().as_slice_value(ctx, generator), ndarray.get_type().ndims())
            })
            .collect_vec();
        broadcast_shapes(generator, ctx, &shape_entries, broadcast_ndims_int, &broadcast_shape);

        // Broadcast all the inputs to shape `dst_shape`.
        let broadcast_ndarrays = ndarrays
            .iter()
            .map(|ndarray| {
                ndarray.broadcast_to(generator, ctx, broadcast_ndims_int, &broadcast_shape)
            })
            .collect_vec();

        BroadcastAllResult {
            ndims: broadcast_ndims_int,
            shape: broadcast_shape,
            ndarrays: broadcast_ndarrays,
        }
    }
}
