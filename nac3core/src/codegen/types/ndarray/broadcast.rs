use std::borrow::Cow;

use inkwell::{
    types::{BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::codegen::{
    CodeGenContext, ModuleContext,
    expr::call_extern,
    stmt::gen_for_callback,
    types::{
        NDArrayValue, ProxyTypeBase, RefCountedArrayType, RefCountedArrayValue,
        TypedRefCountedValue, WithTypeinfo,
        array::ArrayLikeIndexer,
        builtin::BuiltinStruct,
        field,
        ndarray::{NDArrayOut, NDArrayType, RawNDArrayType, ScalarOrNDArray, iter::NDIterValue},
        refcounted_fields_for_struct,
        structure::StructField,
    },
};

// Not a public type; just for interacting with IRRT.
#[derive(Clone, Copy, StructFields)]
struct ShapeEntryStructFields<'ctx> {
    #[value_type(size_t)]
    ndims: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    shape: StructField<'ctx, PointerValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
struct ShapeEntryType<'ctx> {
    inner: BuiltinStruct<'ctx, ShapeEntryStructFields<'ctx>>,
}

impl<'ctx> WithTypeinfo<'ctx> for ShapeEntryType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_shape_entry")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        refcounted_fields_for_struct(ctx, Vec::new())
    }
}

impl<'ctx> ShapeEntryType<'ctx> {
    /// Creates an instance of [`ShapeEntryType`].
    #[must_use]
    fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "shape_entry") }
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
    pub fn broadcast_to(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        target_ndims: u64,
        target_shape: RefCountedArrayValue<'ctx, IntType<'ctx>>,
    ) -> anyhow::Result<Self> {
        assert!(self.ty.object.ndims <= target_ndims);
        assert_eq!(target_shape.ty.elem, ctx.size_t);

        let broadcast_ndarray =
            NDArrayType::create(ctx, self.inner_value(ctx)?.ty.dtype, target_ndims)
                .construct(ctx, None)?;
        broadcast_ndarray.shape(ctx)?.inner_value(ctx, None)?.memcpy_from(
            ctx,
            target_shape.inner_value(ctx, Some(ctx.size_t.const_int(target_ndims, false)))?.value.0,
        )?;

        call_extern!(ctx: void _ = "__nac3_ndarray_broadcast_to"(self.value, broadcast_ndarray.value))?;
        Ok(broadcast_ndarray)
    }
}

/// A result produced by [`broadcast`].
#[derive(Clone)]
pub struct BroadcastAllResult<'ctx> {
    /// The statically known `ndims` of the broadcast result.
    pub ndims: u64,

    /// The broadcasting shape.
    pub shape: RefCountedArrayValue<'ctx, IntType<'ctx>>,

    /// Broadcasted views on the inputs.
    ///
    /// All of them will have `shape` [`BroadcastAllResult::shape`] and
    /// `ndims` [`BroadcastAllResult::ndims`]. The length of the vector
    /// is the same as the input.
    pub ndarrays: Vec<TypedRefCountedValue<'ctx, RawNDArrayType<'ctx>>>,
}

/// Broadcast ndarrays according to
/// [`np.broadcast()`](https://numpy.org/doc/stable/reference/generated/numpy.broadcast.html).
///
/// Returns a [`BroadcastAllResult`] containing all the information of the result of the
/// broadcast operation.
pub fn broadcast<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    ndarrays: &[TypedRefCountedValue<'ctx, RawNDArrayType<'ctx>>],
) -> anyhow::Result<BroadcastAllResult<'ctx>> {
    let shape_entry_ty = ShapeEntryType::new(ctx);
    let shape_entries = ctx.size_t.const_int(ndarrays.len() as _, false);
    let arr =
        RefCountedArrayType::new(ctx, shape_entry_ty.inner.llvm_ty, Some(ndarrays.len() as _))
            .alloca(ctx, ctx.size_t.const_int(ndarrays.len() as _, false), None)?;

    // Store shapes into memory.
    for (i, ndarray) in ndarrays.iter().enumerate() {
        let idx = ctx.size_t.const_int(i as _, false);
        let pshape_entry = arr.inner_value(ctx, None)?.ptr_offset_unchecked(ctx, &idx, None)?;
        let shape_entry = shape_entry_ty.map_value(pshape_entry, None);
        let ndims = ndarray.inner_value(ctx)?.ty.ndims_val(ctx);
        let shape = ndarray.shape(ctx)?.value;
        shape_entry.store(ctx, field!(ndims), ndims)?;
        shape_entry.store(ctx, field!(shape), shape)?;
    }

    let ndims = ndarrays.iter().map(|ndarray| ndarray.ty.object.ndims).max().unwrap();
    let ndims_v = ctx.size_t.const_int(ndims, false);
    let new_shape = RefCountedArrayType::new(ctx, ctx.size_t, Some(ndims as u32))
        .allocate(ctx, ndims_v, None)?;

    call_extern!(ctx: void _ = "__nac3_ndarray_broadcast_shapes"(shape_entries, arr.value, ndims_v, new_shape.value))?;

    // new_shape_ptr is now initialized with the broadcast result shape.
    let new_ndarrays = ndarrays
        .iter()
        .map(|ndarray| ndarray.broadcast_to(ctx, ndims, new_shape))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(BroadcastAllResult { ndims, shape: new_shape, ndarrays: new_ndarrays })
}

/// Generate LLVM IR to broadcast `ndarray`s together, and starmap through them with `mapping`
/// elementwise.
///
/// `mapping` is an LLVM IR generator. The input of `mapping` is the list of elements when
/// iterating through the input `ndarrays` after broadcasting. The output of `mapping` is the
/// result of the elementwise operation.
///
/// `out` specifies whether the result should be a new ndarray or to be written an existing
/// ndarray.
pub fn broadcast_starmap<'ctx, 'a, MappingFn>(
    ctx: &mut CodeGenContext<'ctx, 'a>,
    ndarrays: &[NDArrayValue<'ctx>],
    out: NDArrayOut<'ctx>,
    mapping: MappingFn,
) -> anyhow::Result<NDArrayValue<'ctx>>
where
    MappingFn: FnOnce(
        &mut CodeGenContext<'ctx, 'a>,
        &[BasicValueEnum<'ctx>],
    ) -> anyhow::Result<BasicValueEnum<'ctx>>,
{
    // Broadcast inputs
    let broadcast_result = broadcast(ctx, ndarrays)?;
    let out_ndarray =
        out.resolve(ctx, broadcast_result.ndims, broadcast_result.shape.inner_value(ctx, None)?)?;

    // Map element-wise and store results into `mapped_ndarray`.
    let nditer = NDIterValue::new(ctx, out_ndarray)?;
    gen_for_callback(
        &mut (),
        ctx,
        Some("broadcast_starmap"),
        |(), ctx| {
            // Create NDIters for all broadcasted input ndarrays.
            let other_nditers = broadcast_result
                .ndarrays
                .iter()
                .map(|ndarray| NDIterValue::new(ctx, *ndarray))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok((nditer, other_nditers))
        },
        |(), ctx, (out_nditer, _in_nditers)| {
            // We can simply use `out_nditer`'s `has_element()`.
            // `in_nditers`' `has_element()`s should return the same value.
            out_nditer.inner_value(ctx)?.has_element(ctx)
        },
        |(), ctx, _hooks, (out_nditer, in_nditers)| {
            // Get all the scalars from the broadcasted input ndarrays, pass them to `mapping`,
            // and write to `out_ndarray`.
            let in_scalars = in_nditers
                .iter()
                .map(|nditer| nditer.inner_value(ctx)?.get_scalar(ctx))
                .collect::<anyhow::Result<Vec<_>>>()?;

            let result = mapping(ctx, &in_scalars)?;

            let p = out_nditer.inner_value(ctx)?.curr_ptr(ctx)?;
            ctx.builder.build_store(p, result)?;

            Ok(())
        },
        |(), ctx, (out_nditer, in_nditers)| {
            // Advance all iterators
            out_nditer.inner_value(ctx)?.next(ctx)?;
            for nditer in &in_nditers {
                nditer.inner_value(ctx)?.next(ctx)?;
            }
            Ok(())
        },
        |(), _| Ok(()),
    )?;

    Ok(out_ndarray)
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Starmap through a list of inputs using `mapping`, where an input could be an ndarray, a
    /// scalar.
    ///
    /// This function is very helpful when implementing NumPy functions that takes on either scalars
    /// or ndarrays or a mix of them as their inputs and produces either an ndarray with broadcast,
    /// or a scalar if all its inputs are all scalars.
    ///
    /// For example ,this function can be used to implement `np.add`, which has the following
    /// behaviors:
    ///
    /// - `np.add(3, 4) = 7` # (scalar, scalar) -> scalar
    /// - `np.add(3, np.array([4, 5, 6]))` # (scalar, ndarray) -> ndarray; the first `scalar` is
    ///   converted into an ndarray and broadcasted.
    /// - `np.add(np.array([[1], [2], [3]]), np.array([[4, 5, 6]]))` # (ndarray, ndarray) ->
    ///   ndarray; there is broadcasting.
    ///
    /// ## Details:
    ///
    /// If `inputs` are all [`ScalarOrNDArray::Scalar`], the output will be a
    /// [`ScalarOrNDArray::Scalar`] with type `ret_dtype`.
    ///
    /// Otherwise (if there are any [`ScalarOrNDArray::NDArray`] in `inputs`), all inputs will be
    /// 'as-ndarray'-ed into ndarrays, then all inputs (now all ndarrays) will be passed to
    /// [`broadcast_starmap`] and **create** a new ndarray with dtype `ret_dtype`.
    pub fn broadcasting_starmap<'a, MappingFn>(
        ctx: &mut CodeGenContext<'ctx, 'a>,
        inputs: &[Self],
        ret_dtype: BasicTypeEnum<'ctx>,
        mapping: MappingFn,
    ) -> anyhow::Result<Self>
    where
        MappingFn: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            &[BasicValueEnum<'ctx>],
        ) -> anyhow::Result<BasicValueEnum<'ctx>>,
    {
        // Check if all inputs are Scalars
        let all_scalars: Option<Vec<_>> = inputs
            .iter()
            .map(|i| match i {
                ScalarOrNDArray::Scalar(s) => Some(*s),
                ScalarOrNDArray::NDArray(_) => None,
            })
            .collect();

        if let Some(scalars) = all_scalars {
            let value = mapping(ctx, &scalars)?;
            Ok(ScalarOrNDArray::Scalar(value))
        } else {
            // Promote all input to ndarrays and map through them.
            let inputs = inputs
                .iter()
                .map(|input| input.to_ndarray(ctx))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let ret = NDArrayOut::NewNDArray { dtype: ret_dtype };
            let ndarray = broadcast_starmap(ctx, &inputs, ret, mapping)?;
            Ok(ScalarOrNDArray::NDArray(ndarray))
        }
    }
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Map through this ndarray with an elementwise function.
    pub fn map<'a, Mapping>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        out: NDArrayOut<'ctx>,
        mapping: Mapping,
    ) -> anyhow::Result<Self>
    where
        Mapping: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            BasicValueEnum<'ctx>,
        ) -> anyhow::Result<BasicValueEnum<'ctx>>,
    {
        broadcast_starmap(ctx, &[*self], out, |ctx, scalars| mapping(ctx, scalars[0]))
    }
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Map through this [`ScalarOrNDArray`] with an elementwise function.
    ///
    /// If this is a scalar, `mapping` will directly act on the scalar. This function will return a
    /// [`ScalarOrNDArray::Scalar`] of that result.
    ///
    /// If this is an ndarray, `mapping` will be applied to the elements of the ndarray. A new
    /// ndarray of the results will be created and returned as a [`ScalarOrNDArray::NDArray`].
    pub fn map<'a, Mapping>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        ret_dtype: BasicTypeEnum<'ctx>,
        mapping: Mapping,
    ) -> anyhow::Result<Self>
    where
        Mapping: FnOnce(
            &mut CodeGenContext<'ctx, 'a>,
            BasicValueEnum<'ctx>,
        ) -> anyhow::Result<BasicValueEnum<'ctx>>,
    {
        ScalarOrNDArray::broadcasting_starmap(ctx, &[*self], ret_dtype, |ctx, scalars| {
            mapping(ctx, scalars[0])
        })
    }
}
