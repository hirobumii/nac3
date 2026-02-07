use inkwell::{
    IntPredicate,
    types::BasicTypeEnum,
    values::{BasicValueEnum, IntValue, PointerValue},
};
use itertools::Itertools as _;
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        expr::call_extern,
        irrt::get_usize_dependent_function_name,
        llvm_intrinsics::call_int_umin,
        stmt::{gen_array_var, gen_dyn_array_var, gen_for_callback_incrementing, gen_var},
        typed_load, typed_store,
        types::{
            ProxyTypeExt, Value,
            array::{ArrayLikeIndexer, ArraySliceValue},
            builtin::BuiltinStruct,
            field,
            structure::StructField,
            tuple::TupleValue,
        },
    },
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys},
    typecheck::typedef::{Type, TypeEnum},
};

mod array;
mod broadcast;
mod contiguous;
mod factory;
mod indexing;
mod iter;
mod matmul;
mod shape;
mod view;

pub use broadcast::{BroadcastAllResult, broadcast, broadcast_starmap};
pub use contiguous::{ContiguousNDArrayType, ContiguousNDArrayValue};
pub use indexing::{NDIndexType, NDIndexValue, RustNDIndex};
pub use iter::{NDIterType, NDIterValue};
pub use shape::parse_numpy_int_sequence;

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct NDArrayLikeType<'ctx, S> {
    pub inner: BuiltinStruct<'ctx, S>,
    pub dtype: BasicTypeEnum<'ctx>,
    pub ndims: u64,
}

impl<'ctx, S> NDArrayLikeType<'ctx, S> {
    /// Returns the number of dimensions as an `IntValue`.
    pub fn ndims_val(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        ctx.size_t.const_int(self.ndims, false)
    }
    /// Returns the item size in bytes as an `IntValue`.
    pub fn itemsize_val(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let size = ctx.sizeof(self.dtype);
        ctx.size_t.const_int(size, false)
    }
}
impl<'ctx, S> Value<'ctx, NDArrayLikeType<'ctx, S>> {
    /// Loads a slice of length `ndims` from the given field.
    pub(crate) fn load_ndims_slice(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        field: impl FnOnce(&NDArrayLikeType<'ctx, S>) -> StructField<'ctx, PointerValue<'ctx>>,
    ) -> ArraySliceValue<'ctx> {
        let ptr = self.load(ctx, field);
        ArraySliceValue::new(ctx.size_t.into(), ptr, self.ty.ndims_val(ctx), self.name)
    }
}

#[derive(Clone, Copy, StructFields)]
pub struct NDArrayStructFields<'ctx> {
    /// The size of each `NDArray` element in bytes.
    #[value_type(size_t)]
    pub itemsize: StructField<'ctx, IntValue<'ctx>>,
    /// Number of dimensions in the array.
    #[value_type(size_t)]
    pub ndims: StructField<'ctx, IntValue<'ctx>>,
    /// Pointer to an array containing the shape of the `NDArray`.
    // TODO: We currently store shape and strides as `size_t`, but np_shape returns `int32`.
    // Consider picking one.
    #[value_type(ptr)]
    pub shape: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array indicating the number of bytes between each element at a dimension
    #[value_type(ptr)]
    pub strides: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array containing the array data
    #[value_type(ptr)]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

pub type NDArrayType<'ctx> = NDArrayLikeType<'ctx, NDArrayStructFields<'ctx>>;

impl<'ctx> NDArrayType<'ctx> {
    /// Creates an instance of [`NDArrayType`].
    #[must_use]
    pub fn new(ctx: &ModuleContext<'ctx>, dtype: BasicTypeEnum<'ctx>, ndims: u64) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "ndarray"), dtype, ndims }
    }

    /// Decodes a [`Type`] into an [`NDArrayType`].
    ///
    /// Panics if `ty` is not an `NDArray` type.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);
        let llvm_dtype = ctx.get_llvm_type(dtype);
        let ndims = extract_ndims(&ctx.unifier, ndims);
        Self::new(ctx, llvm_dtype, ndims)
    }

    /// Creates a new `NDArrayValue`.
    ///
    /// The shape and strides arrays are allocated but uninitialized. The data array is not allocated.
    ///
    /// Once you properly set up the `shape` array, you can construct a fully usable ndarray with
    /// [`create_data`][NDArrayValue::create_data]. To construct a fully usable ndarray directly
    /// when the shape is known, use [`NDArrayType::with_shape`].
    #[must_use]
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        let ndarray = self.alloca(ctx, name);

        let size = self.itemsize_val(ctx);
        ndarray.store(ctx, field!(itemsize), size);
        let (ndims_int, ndims) = (self.ndims, self.ndims_val(ctx));
        ndarray.store(ctx, field!(ndims), ndims);

        let shape = gen_array_var(ctx, ctx.size_t, ndims_int, None).value.0;
        ndarray.store(ctx, field!(shape), shape);
        let strides = gen_array_var(ctx, ctx.size_t, ndims_int, None).value.0;
        ndarray.store(ctx, field!(strides), strides);

        ndarray
    }

    /// Creates a new, contiguous `NDArrayValue` with a given shape.
    ///
    /// The shape array is initialized to `shape`. The strides array is prepared accordingly.
    /// The data array is allocated but uninitialized.
    #[must_use]
    pub fn with_shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &[IntValue<'ctx>],
        name: Option<&'static str>,
    ) -> NDArrayValue<'ctx> {
        let ndarray = self.construct(ctx, name);
        let dst = ndarray.shape(ctx);
        for (i, &dim) in shape.iter().enumerate() {
            let i = ctx.size_t.const_int(i as _, false);
            dst.set_unchecked(ctx, &i, dim, name);
        }
        ndarray.create_data(ctx);
        ndarray
    }
}

pub type NDArrayValue<'ctx> = Value<'ctx, NDArrayType<'ctx>>;

impl<'ctx> NDArrayValue<'ctx> {
    /// Returns the shape of this array.
    #[must_use]
    pub fn shape(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ArraySliceValue<'ctx> {
        self.load_ndims_slice(ctx, field!(shape))
    }

    /// Returns the strides of this array.
    #[must_use]
    pub fn strides(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ArraySliceValue<'ctx> {
        self.load_ndims_slice(ctx, field!(strides))
    }

    /// Returns a new scalar `NDArrayValue` containing `value`.
    ///
    /// The returned value has 0 dimensions.
    #[must_use]
    pub fn new_scalar(
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
        name: Option<&'static str>,
    ) -> Self {
        let dtype = value.get_type();
        let ndarray = NDArrayType::new(ctx, dtype, 0).construct(ctx, name);
        let data = gen_var(ctx, value.get_type(), Some("map_unsized"));
        typed_store(&ctx.builder, data, value);
        let data = ctx.builder.build_pointer_cast(data, ctx.ptr, "").unwrap();
        ndarray.store(ctx, field!(data), data);
        ndarray
    }

    /// Computes the total number of (scalar) elements in this array.
    #[must_use]
    pub fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let shape = self.shape(ctx);
        let mut product = ctx.size_t.const_int(1, false);
        for i in 0..self.ty.ndims {
            let idx = ctx.size_t.const_int(i, false);
            let dim = shape.get_unchecked(ctx, &idx, None);
            product = ctx.builder.build_int_mul(product, dim, "").unwrap();
        }
        product
    }

    /// Allocates contiguous memory for the data array and assigns strides correspondingly.
    ///
    /// Assumes `shape` has been correctly prepared.
    pub fn create_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) {
        let size = self.size(ctx);
        let alloc = gen_dyn_array_var(ctx, self.ty.dtype, size, None).value.0;
        self.store(ctx, field!(data), alloc);
        self.set_strides_contiguous(ctx);
    }

    /// Assigns strides for a contiguous array.
    ///
    /// Assumes `shape` has been correctly prepared.
    pub fn set_strides_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) {
        let shape = self.shape(ctx);
        let strides = self.strides(ctx);

        let mut stride = self.ty.itemsize_val(ctx);
        for i in (0..self.ty.ndims).rev() {
            let idx = ctx.size_t.const_int(i, false);
            strides.set_unchecked(ctx, &idx, stride, self.name);
            let dim = shape.get_unchecked(ctx, &idx, None);
            stride = ctx.builder.build_int_mul(stride, dim, "").unwrap();
        }
    }

    /// Returns the length of the first dimension of the array.
    #[must_use]
    pub fn len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        assert!(self.ty.ndims >= 1);
        self.shape(ctx).get_unchecked(ctx, &ctx.size_t.const_zero(), self.name)
    }

    /// Returns the number of bytes consumed by the array data.
    #[must_use]
    pub fn nbytes(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let size = self.size(ctx);
        let itemsize = self.ty.itemsize_val(ctx);
        ctx.builder.build_int_mul(size, itemsize, "").unwrap()
    }

    /// Checks if the array is C-contiguous.
    #[must_use]
    pub fn is_c_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_is_c_contiguous");
        call_extern!(ctx: (ctx.i1) "is_c_contiguous" = name(self.value))
    }

    /// Creates a copy of this array.
    #[must_use]
    pub fn make_copy(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Self {
        let shape = self.shape(ctx);
        let clone =
            NDArrayOut::NewNDArray { dtype: self.ty.dtype }.resolve(ctx, self.ty.ndims, shape);
        clone.copy_data_from(ctx, self);
        clone
    }

    /// Copies data from `src` into this array.
    pub fn copy_data_from(&self, ctx: &mut CodeGenContext<'ctx, '_>, src: &Self) {
        let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_copy_data");
        call_extern!(ctx: void _ = name(src.value, self.value));
    }

    /// Copies the shape of `src` into this array.
    pub fn copy_shape_from(&self, ctx: &mut CodeGenContext<'ctx, '_>, src: &Self) {
        let shape = src.shape(ctx);
        self.shape(ctx).memcpy_from(ctx, shape.value.0);
    }

    fn read_shape_or_stride_as_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        arr: ArraySliceValue<'ctx>,
        name: &'static str,
    ) -> TupleValue<'ctx> {
        // let types = vec![ctx.size_t.into(); self.ty.ndims as usize];
        // let ty = TupleType::new(ctx, &types);
        let values = (0..self.ty.ndims)
            .map(|i| {
                let idx = ctx.size_t.const_int(i as _, false);
                let val = arr.get_unchecked::<IntValue<'ctx>>(ctx, &idx, None);
                ctx.builder.build_int_truncate_or_bit_cast(val, ctx.i32, "").unwrap()
            })
            .collect_vec();

        TupleValue::new(ctx, &values, Some(name))
    }

    /// Returns a `tuple` representing the shape of this array.
    #[must_use]
    pub fn make_shape_tuple(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> TupleValue<'ctx> {
        let shape = self.shape(ctx);
        self.read_shape_or_stride_as_tuple(ctx, shape, "shape")
    }

    /// Returns a `tuple` representing the strides of this array.
    #[must_use]
    pub fn make_strides_tuple(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> TupleValue<'ctx> {
        let strides = self.strides(ctx);
        self.read_shape_or_stride_as_tuple(ctx, strides, "strides")
    }

    /// If this ndarray is unsized, return its sole value as an [`BasicValueEnum`].
    /// Otherwise, do nothing and return the ndarray itself.
    #[must_use]
    pub fn split_unsized(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> ScalarOrNDArray<'ctx> {
        if self.ty.ndims == 0 {
            ScalarOrNDArray::Scalar(self.first_element(ctx))
        } else {
            ScalarOrNDArray::NDArray(*self)
        }
    }

    /// Fills the array with the given value.
    pub fn fill(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: BasicValueEnum<'ctx>) {
        // TODO: It is possible to optimize this by exploiting contiguous strides with memset.
        //       Probably best to implement in IRRT.
        self.foreach(ctx, |ctx, _, nditer| {
            let p = nditer.curr_ptr(ctx);
            typed_store(&ctx.builder, p, value);
            Ok(())
        })
        .unwrap();
    }

    /// Returns the first element of this ndarray.
    #[must_use]
    pub fn first_element(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> BasicValueEnum<'ctx> {
        let data = self.load(ctx, field!(data));
        typed_load(&ctx.builder, data, self.ty.dtype, "first_element")
    }
}

/// A version of `__nac3_ndarray_set_strides_by_shape` in Rust.
///
/// This function is used generating strides for globally defined contiguous ndarrays.
#[must_use]
pub fn make_contiguous_strides(shape: &[u64], itemsize: u64) -> Vec<u64> {
    let mut strides = vec![0; shape.len()];
    let mut stride = itemsize;
    for i in (0..shape.len()).rev() {
        strides[i] = stride;
        stride *= shape[i];
    }
    strides
}

impl<'ctx> ArrayLikeIndexer<'ctx, ArraySliceValue<'ctx>> for NDArrayValue<'ctx> {
    fn item_type(&self) -> BasicTypeEnum<'ctx> {
        self.ty.dtype
    }

    fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &ArraySliceValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let name = name.unwrap_or("pelement");
        let fn_name =
            get_usize_dependent_function_name(ctx, "__nac3_ndarray_get_pelement_by_indices");
        call_extern!(ctx: (ctx.ptr) name = fn_name(self.value, idx.value.0))
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &ArraySliceValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = ctx.size_t;

        let indices_len = idx.value.1;
        let ndims = self.ty.ndims_val(ctx);
        let nidx_leq_ndims =
            ctx.builder.build_int_compare(IntPredicate::SLE, indices_len, ndims, "").unwrap();
        ctx.make_assert(
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        let len = call_int_umin(ctx, indices_len, ndims, None);
        gen_for_callback_incrementing(
            &mut (),
            ctx,
            None,
            llvm_usize.const_zero(),
            (len, false),
            |(), ctx, _, i| {
                let (dim_idx, dim_sz) = (
                    idx.get_unchecked::<IntValue<'ctx>>(ctx, &i, None),
                    self.shape(ctx).get_unchecked::<IntValue<'ctx>>(ctx, &i, None),
                );
                let dim_idx = ctx
                    .builder
                    .build_int_z_extend_or_bit_cast(dim_idx, dim_sz.get_type(), "")
                    .unwrap();

                let dim_lt =
                    ctx.builder.build_int_compare(IntPredicate::SLT, dim_idx, dim_sz, "").unwrap();

                ctx.make_assert(
                    dim_lt,
                    "0:IndexError",
                    "index {0} is out of bounds for axis 0 with size {1}",
                    [Some(dim_idx), Some(dim_sz), None],
                    ctx.current_loc,
                );

                Ok(())
            },
            llvm_usize.const_int(1, false),
            |(), _| Ok(()),
        )
        .unwrap();

        self.ptr_offset_unchecked(ctx, idx, name)
    }
}

/// A convenience enum for implementing functions that acts on scalars or ndarrays or both.
#[derive(Clone, Copy)]
pub enum ScalarOrNDArray<'ctx> {
    Scalar(BasicValueEnum<'ctx>),
    NDArray(NDArrayValue<'ctx>),
}

/// A fancy assertion of `src_shape == dst_shape` for ndarray write operations.
pub fn assert_ndarray_can_be_written_by_out<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    src_shape: ArraySliceValue<'ctx>,
    dst_shape: ArraySliceValue<'ctx>,
) {
    let name =
        get_usize_dependent_function_name(ctx, "__nac3_ndarray_util_assert_output_shape_same");
    let ((src_ptr, src_len), (dst_ptr, dst_len)) = (src_shape.value, dst_shape.value);
    call_extern!(ctx: (ctx.size_t) _ = name(src_len, src_ptr, dst_len, dst_ptr));
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Split on `object` either into a scalar or an ndarray.
    ///
    /// If `object` is an ndarray, [`ScalarOrNDArray::NDArray`].
    ///
    /// For everything else, it is wrapped with [`ScalarOrNDArray::Scalar`].
    #[must_use]
    pub fn from_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        (object_ty, object): (Type, BasicValueEnum<'ctx>),
    ) -> Self {
        match &*ctx.unifier.get_ty(object_ty) {
            TypeEnum::TObj { obj_id, .. }
                if *obj_id == ctx.primitives.ndarray.obj_id(&ctx.unifier).unwrap() =>
            {
                let ndarray = NDArrayType::from_unifier_type(ctx, object_ty)
                    .map_value(object.into_pointer_value(), None);
                ScalarOrNDArray::NDArray(ndarray)
            }

            _ => ScalarOrNDArray::Scalar(object),
        }
    }

    /// Get the underlying [`BasicValueEnum<'ctx>`] of this [`ScalarOrNDArray`].
    #[must_use]
    pub fn to_basic_value_enum(self) -> BasicValueEnum<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(val) => val,
            ScalarOrNDArray::NDArray(val) => val.value.into(),
        }
    }

    /// If this is a scalar, create a scalar ndarray from it. Otherwise, return the ndarray itself.
    ///
    /// This is the opposite of [`ScalarOrNDArray::from_value`].
    #[must_use]
    pub fn to_ndarray(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> NDArrayValue<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(v) => NDArrayValue::new_scalar(ctx, *v, None),
            ScalarOrNDArray::NDArray(val) => *val,
        }
    }

    /// Get the dtype of the ndarray created if this were called with
    /// [`ScalarOrNDArray::to_ndarray`].
    #[must_use]
    pub fn get_dtype(&self) -> BasicTypeEnum<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(v) => v.get_type(),
            ScalarOrNDArray::NDArray(val) => val.ty.dtype,
        }
    }
}

/// An helper enum specifying how a function should produce its output.
///
/// Many functions in NumPy has an optional `out` parameter (e.g., `matmul`). If `out` is specified
/// with an ndarray, the result of a function will be written to `out`. If `out` is not specified, a
/// function will create a new ndarray and store the result in it.
#[derive(Clone, Copy)]
pub enum NDArrayOut<'ctx> {
    /// Tell a function should create a new ndarray with the expected element type `dtype`.
    NewNDArray { dtype: BasicTypeEnum<'ctx> },
    /// Tell a function to write the result to `ndarray`.
    WriteToNDArray { ndarray: NDArrayValue<'ctx> },
}

impl<'ctx> NDArrayOut<'ctx> {
    /// Get the dtype of this output.
    #[must_use]
    pub const fn get_dtype(&self) -> BasicTypeEnum<'ctx> {
        match self {
            NDArrayOut::NewNDArray { dtype } => *dtype,
            NDArrayOut::WriteToNDArray { ndarray } => ndarray.ty.dtype,
        }
    }

    /// Produce an `NDArrayValue` according to this output specification and the actual
    /// required output shape.
    #[must_use]
    pub fn resolve(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndims: u64,
        shape: ArraySliceValue<'ctx>,
    ) -> NDArrayValue<'ctx> {
        match self {
            NDArrayOut::NewNDArray { dtype } => {
                let result_ndarray = NDArrayType::new(ctx, *dtype, ndims).construct(ctx, None);
                result_ndarray.shape(ctx).memcpy_from(ctx, shape.value.0);
                result_ndarray.create_data(ctx);
                result_ndarray
            }

            NDArrayOut::WriteToNDArray { ndarray: result } => {
                // Use an existing ndarray.
                let out_shape = result.shape(ctx);
                assert_ndarray_can_be_written_by_out(ctx, shape, out_shape);
                *result
            }
        }
    }
}
