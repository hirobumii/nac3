use std::borrow::Cow;

use inkwell::{
    IntPredicate,
    types::{BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
};
use nac3core_derive::{ProxyType, StructFields};

use crate::{
    codegen::{
        CodeGenContext, ModuleContext,
        allocator::AllocationScope,
        expr::call_extern,
        irrt::get_usize_dependent_function_name,
        llvm_intrinsics::call_int_umin,
        stmt::gen_for_callback_incrementing,
        types::{
            ProxyTypeBase, RefCountedArrayType, RefCountedArrayValue, TypedRefCountedType,
            TypedRefCountedValue, Value, WithTypeinfo,
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
pub use contiguous::{
    ContiguousNDArrayType, ContiguousNDArrayValue, RawContiguousNDArrayType,
    RawContiguousNDArrayValue,
};
pub use indexing::{NDIndexType, NDIndexValue, RustNDIndex};
pub use iter::{NDIterType, NDIterValue, RawNDIterType, RawNDIterValue};
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
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        let ptr = self.load(ctx, field)?;
        Ok(RefCountedArrayType::new(ctx, ctx.size_t, Some(self.ty.ndims as u32))
            .map_value(ptr, self.name))
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
    shape: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array indicating the number of bytes between each element at a dimension
    #[value_type(ptr)]
    strides: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to an array containing the array data
    #[value_type(ptr)]
    data: StructField<'ctx, PointerValue<'ctx>>,
    /// Pointer to the base of the array if this array is a view.
    #[value_type(ptr)]
    base: StructField<'ctx, PointerValue<'ctx>>,
    /// The offset in bytes from the base pointer to the first element of this array.
    #[value_type(size_t)]
    pub offset: StructField<'ctx, IntValue<'ctx>>,
}

pub type RawNDArrayType<'ctx> = NDArrayLikeType<'ctx, NDArrayStructFields<'ctx>>;

impl<'ctx> RawNDArrayType<'ctx> {
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
}

impl<'ctx> NDArrayType<'ctx> {
    /// Creates an instance of [`NDArrayType`].
    #[must_use]
    pub fn create(ctx: &ModuleContext<'ctx>, dtype: BasicTypeEnum<'ctx>, ndims: u64) -> Self {
        Self::new(ctx, RawNDArrayType::new(ctx, dtype, ndims))
    }

    /// Decodes a [`Type`] into an [`NDArrayType`].
    ///
    /// Panics if `ty` is not an `NDArray` type.
    #[must_use]
    pub fn from_unifier_type(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> Self {
        let ty = RawNDArrayType::from_unifier_type(ctx, ty);
        Self::new(ctx, ty)
    }

    /// Creates a new `NDArrayValue`.
    ///
    /// The `shape` and `strides` arrays are allocated but uninitialized, the `data` array is not
    /// allocated, `base` is set to `null`, and `offset` is uninitialized.
    ///
    /// Once you properly set up the `shape` array, you can construct a fully usable ndarray with
    /// [`create_data`][NDArrayValue::create_data]. To construct a fully usable ndarray directly
    /// when the shape is known, use [`NDArrayType::with_shape`].
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<NDArrayValue<'ctx>> {
        let ndarray = self.allocate(ctx, AllocationScope::Default, name)?;

        let size = self.object.itemsize_val(ctx);
        ndarray.inner_value(ctx)?.store(ctx, field!(itemsize), size)?;
        let ndims = self.object.ndims_val(ctx);
        ndarray.inner_value(ctx)?.store(ctx, field!(ndims), ndims)?;

        let shape = RefCountedArrayType::new(ctx, ctx.size_t, None).allocate(ctx, ndims, None)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(shape), shape.value)?;
        let strides = RefCountedArrayType::new(ctx, ctx.size_t, None).allocate(ctx, ndims, None)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(strides), strides.value)?;

        // Set `base` to `null` to prevent accidental refcounting of uninitialized memory
        ndarray.inner_value(ctx)?.store(ctx, field!(base), ctx.ptr.const_null())?;

        Ok(ndarray)
    }

    /// Creates a new, contiguous `NDArrayValue` with a given shape.
    ///
    /// The shape array is initialized to `shape`. The strides array is prepared accordingly.
    /// The data array is allocated but uninitialized.
    pub fn with_shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: &[IntValue<'ctx>],
        name: Option<&'static str>,
    ) -> anyhow::Result<NDArrayValue<'ctx>> {
        let ndarray = self.construct(ctx, name)?;
        let dst = ndarray.shape(ctx)?;
        for (i, &dim) in shape.iter().enumerate() {
            let i = ctx.size_t.const_int(i as _, false);
            dst.inner_value(ctx, None)?.set_unchecked(ctx, &i, dim, name)?;
        }
        ndarray.create_data(ctx)?;
        Ok(ndarray)
    }
}

impl<'ctx> WithTypeinfo<'ctx> for RawNDArrayType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_ndarray")
    }

    fn refcounted_field_offset(&self, ctx: &ModuleContext<'ctx>) -> Vec<IntValue<'ctx>> {
        vec![
            ctx.i32.const_int(2 * ctx.sizeof(ctx.size_t), false),
            ctx.i32.const_int(2 * ctx.sizeof(ctx.size_t) + ctx.sizeof(ctx.ptr), false),
            ctx.i32.const_int(2 * ctx.sizeof(ctx.size_t) + 2 * ctx.sizeof(ctx.ptr), false),
            ctx.i32.const_int(2 * ctx.sizeof(ctx.size_t) + 3 * ctx.sizeof(ctx.ptr), false),
        ]
    }
}

pub type NDArrayType<'ctx> = TypedRefCountedType<'ctx, RawNDArrayType<'ctx>>;
pub type RawNDArrayValue<'ctx> = Value<'ctx, RawNDArrayType<'ctx>>;
pub type NDArrayValue<'ctx> = TypedRefCountedValue<'ctx, RawNDArrayType<'ctx>>;

impl<'ctx> RawNDArrayValue<'ctx> {
    /// Returns the shape of this array.
    pub fn shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        self.load_ndims_slice(ctx, field!(shape))
    }

    /// Returns the strides of this array.
    pub fn strides(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        self.load_ndims_slice(ctx, field!(strides))
    }

    /// Returns the underlying data [`RefCountedArrayValue`] of this ndarray.
    ///
    /// This points to the base of the data allocation. To get a slice starting at the ndarray's
    /// current offset, use [`data`][Self::data] instead.
    pub fn base_data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, BasicTypeEnum<'ctx>>> {
        let data = self.load(ctx, field!(data))?;
        Ok(RefCountedArrayType::new(ctx, self.ty.dtype, None).map_value(data, self.name))
    }

    /// Returns an [`ArraySliceValue`] for the data of this ndarray, starting at the ndarray's
    /// current byte offset.
    ///
    /// Element 0 of the returned slice corresponds to the first element of this ndarray view.
    pub fn data(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ArraySliceValue<'ctx, BasicTypeEnum<'ctx>>> {
        let size = self.size(ctx)?;
        let offset = self.load(ctx, field!(offset))?;
        let itemsize = self.load(ctx, field!(itemsize))?;
        let elem_idx = ctx.builder.build_int_unsigned_div(offset, itemsize, "")?;
        let base_inner = self.base_data(ctx)?.inner_value(ctx, Some(size))?;
        let ptr = base_inner.ptr_offset_unchecked(ctx, &elem_idx, None)?;
        Ok(ArraySliceValue::new(base_inner.ty.item_ty, ptr, size, self.name))
    }

    /// Returns the base of this array.
    ///
    /// Note that the returned value may be `null`.
    pub fn base(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<NDArrayValue<'ctx>> {
        let base = self.load(ctx, field!(base))?;
        Ok(TypedRefCountedType::new(ctx, self.ty).map_value(base, self.name))
    }

    /// Returns a new scalar `NDArrayValue` containing `value`.
    ///
    /// The returned value has 0 dimensions.
    pub fn new_scalar(
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
        name: Option<&'static str>,
    ) -> anyhow::Result<TypedRefCountedValue<'ctx, RawNDArrayType<'ctx>>> {
        let dtype = value.get_type();
        let ndarray = NDArrayType::create(ctx, dtype, 0).construct(ctx, name)?;
        // Allocate a 1-element RefCountedArray so the IRRT can find the value at the
        // correct offset (past ObjectHeader + count) via `data->data<uint8_t>()`.
        let alloc = RefCountedArrayType::new(ctx, dtype, Some(1)).allocate(
            ctx,
            ctx.size_t.const_int(1, false),
            None,
        )?;
        alloc.inner_value(ctx, None)?.set_unchecked(ctx, &ctx.size_t.const_zero(), value, None)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(data), alloc.value)?;
        ndarray.inner_value(ctx)?.store(ctx, field!(base), ctx.ptr.const_null())?;
        ndarray.inner_value(ctx)?.store(ctx, field!(offset), ctx.size_t.const_zero())?;
        Ok(ndarray)
    }

    /// Computes the total number of (scalar) elements in this array.
    pub fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        let shape = self.shape(ctx)?;
        let mut product = ctx.size_t.const_int(1, false);
        for i in 0..self.ty.ndims {
            let idx = ctx.size_t.const_int(i, false);
            let dim = shape.inner_value(ctx, None)?.get_unchecked(ctx, &idx, None)?;
            product = ctx.builder.build_int_mul(product, dim, "")?;
        }
        Ok(product)
    }

    /// Allocates contiguous memory for the data array and assigns strides correspondingly.
    ///
    /// Assumes `shape` has been correctly prepared.
    pub fn create_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let size = self.size(ctx)?;
        let alloc = RefCountedArrayType::new(ctx, self.ty.dtype, None).allocate(ctx, size, None)?;
        self.store(ctx, field!(data), alloc.value)?;
        self.store(ctx, field!(offset), ctx.size_t.const_zero())?;
        self.set_strides_contiguous(ctx)?;
        self.store(ctx, field!(base), ctx.ptr.const_null())?;
        self.store(ctx, field!(offset), ctx.size_t.const_zero())?;
        Ok(())
    }

    /// Assigns strides for a contiguous array.
    ///
    /// Assumes `shape` has been correctly prepared.
    pub fn set_strides_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        let shape = self.shape(ctx)?;
        let strides = self.strides(ctx)?;

        let mut stride = self.ty.itemsize_val(ctx);
        for i in (0..self.ty.ndims).rev() {
            let idx = ctx.size_t.const_int(i, false);
            strides.inner_value(ctx, None)?.set_unchecked(ctx, &idx, stride, self.name)?;
            let dim = shape.inner_value(ctx, None)?.get_unchecked(ctx, &idx, None)?;
            stride = ctx.builder.build_int_mul(stride, dim, "")?;
        }

        Ok(())
    }

    /// Returns the length of the first dimension of the array.
    pub fn len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        assert!(self.ty.ndims >= 1);
        self.shape(ctx)?.inner_value(ctx, None)?.get_unchecked(
            ctx,
            &ctx.size_t.const_zero(),
            self.name,
        )
    }

    /// Returns the number of bytes consumed by the array data.
    pub fn nbytes(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        let size = self.size(ctx)?;
        let itemsize = self.ty.itemsize_val(ctx);
        Ok(ctx.builder.build_int_mul(size, itemsize, "")?)
    }

    /// Copies the shape of `src` into this array.
    pub fn copy_shape_from(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: &Self,
    ) -> anyhow::Result<()> {
        let shape = src.shape(ctx)?;
        self.shape(ctx)?
            .inner_value(ctx, None)?
            .memcpy_from(ctx, shape.inner_value(ctx, None)?.value.0)?;
        Ok(())
    }

    fn read_shape_or_stride_as_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        arr: ArraySliceValue<'ctx, IntType<'ctx>>,
        name: &'static str,
    ) -> anyhow::Result<TupleValue<'ctx>> {
        let values = (0..self.ty.ndims)
            .map(|i| {
                let idx = ctx.size_t.const_int(i as _, false);
                let val = arr.get_unchecked::<IntValue<'ctx>>(ctx, &idx, None)?;
                Ok(ctx.builder.build_int_truncate_or_bit_cast(val, ctx.i32, "")?)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        TupleValue::new(ctx, &values, Some(name))
    }

    /// Returns a `tuple` representing the shape of this array.
    pub fn make_shape_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<TupleValue<'ctx>> {
        let shape = self.shape(ctx)?;
        self.read_shape_or_stride_as_tuple(ctx, shape.inner_value(ctx, None)?, "shape")
    }

    /// Returns a `tuple` representing the strides of this array.
    pub fn make_strides_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<TupleValue<'ctx>> {
        let strides = self.strides(ctx)?;
        self.read_shape_or_stride_as_tuple(ctx, strides.inner_value(ctx, None)?, "strides")
    }

    /// Returns the first element of this ndarray.
    pub fn first_element(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        self.data(ctx)?.get_unchecked(ctx, &ctx.size_t.const_zero(), Some("first_element"))
    }
}

impl<'ctx> TypedRefCountedValue<'ctx, RawNDArrayType<'ctx>> {
    /// Returns the shape of this array.
    pub fn shape(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        self.inner_value(ctx)?.shape(ctx)
    }

    /// Returns the strides of this array.
    pub fn strides(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<RefCountedArrayValue<'ctx, IntType<'ctx>>> {
        self.inner_value(ctx)?.strides(ctx)
    }

    /// Computes the total number of (scalar) elements in this array.
    pub fn size(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        self.inner_value(ctx)?.size(ctx)
    }

    /// Allocates contiguous memory for the data array and assigns strides correspondingly.
    pub fn create_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        self.inner_value(ctx)?.create_data(ctx)
    }

    /// Assigns strides for a contiguous array.
    pub fn set_strides_contiguous(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<()> {
        self.inner_value(ctx)?.set_strides_contiguous(ctx)
    }

    /// Copies the shape of `src` into this array.
    pub fn copy_shape_from(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: &Self,
    ) -> anyhow::Result<()> {
        self.inner_value(ctx)?.copy_shape_from(ctx, &src.inner_value(ctx)?)
    }

    /// Fills the array with the given value.
    pub fn fill(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) -> anyhow::Result<()> {
        // TODO: It is possible to optimize this by exploiting contiguous strides with memset.
        //       Probably best to implement in IRRT.
        self.foreach(ctx, |ctx, _, nditer| {
            let p = nditer.inner_value(ctx)?.curr_ptr(ctx)?;
            ctx.builder.build_store(p, value)?;
            Ok(())
        })?;
        Ok(())
    }

    /// Returns the first element of this ndarray.
    pub fn first_element(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<BasicValueEnum<'ctx>> {
        self.inner_value(ctx)?.first_element(ctx)
    }

    /// Returns the length of the first dimension of the array.
    pub fn len(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        self.inner_value(ctx)?.len(ctx)
    }

    /// Returns the number of bytes consumed by the array data.
    pub fn nbytes(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<IntValue<'ctx>> {
        self.inner_value(ctx)?.nbytes(ctx)
    }

    /// Returns a `tuple` representing the shape of this array.
    pub fn make_shape_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<TupleValue<'ctx>> {
        self.inner_value(ctx)?.make_shape_tuple(ctx)
    }

    /// Returns a `tuple` representing the strides of this array.
    pub fn make_strides_tuple(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<TupleValue<'ctx>> {
        self.inner_value(ctx)?.make_strides_tuple(ctx)
    }

    /// If this ndarray is unsized, return its sole value as a [`BasicValueEnum`].
    /// Otherwise, do nothing and return the ndarray itself.
    pub fn split_unsized(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<ScalarOrNDArray<'ctx>> {
        let inner = self.inner_value(ctx)?;
        Ok(if inner.ty.ndims == 0 {
            ScalarOrNDArray::Scalar(inner.first_element(ctx)?)
        } else {
            ScalarOrNDArray::NDArray(*self)
        })
    }

    /// Checks if the array is C-contiguous.
    pub fn is_c_contiguous(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<IntValue<'ctx>> {
        let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_is_c_contiguous");
        call_extern!(ctx: (ctx.i1) "is_c_contiguous" = name(self.value))
    }

    /// Copies data from `src` into this array.
    pub fn copy_data_from(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: &Self,
    ) -> anyhow::Result<()> {
        let name = get_usize_dependent_function_name(ctx, "__nac3_ndarray_copy_data");
        call_extern!(ctx: void _ = name(src.value, self.value))?;
        Ok(())
    }

    /// Creates a copy of this array.
    pub fn make_copy(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> anyhow::Result<Self> {
        let shape = self.inner_value(ctx)?.shape(ctx)?;
        let clone = NDArrayOut::NewNDArray { dtype: self.inner_value(ctx)?.ty.dtype }.resolve(
            ctx,
            self.inner_value(ctx)?.ty.ndims,
            shape.inner_value(ctx, None)?,
        )?;
        clone.copy_data_from(ctx, self)?;
        Ok(clone)
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

impl<'ctx> ArrayLikeIndexer<'ctx, ArraySliceValue<'ctx, IntType<'ctx>>> for NDArrayValue<'ctx> {
    fn item_type(&self, _ctx: &ModuleContext<'ctx>) -> BasicTypeEnum<'ctx> {
        self.ty.object.dtype
    }

    fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &ArraySliceValue<'ctx, IntType<'ctx>>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        let name = name.unwrap_or("pelement");
        let fn_name =
            get_usize_dependent_function_name(ctx, "__nac3_ndarray_get_pelement_by_indices");
        call_extern!(ctx: (ctx.ptr) name = fn_name(self.value, idx.value.0))
    }

    fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: &ArraySliceValue<'ctx, IntType<'ctx>>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        let llvm_usize = ctx.size_t;

        let indices_len = idx.value.1;
        let ndims = self.inner_value(ctx)?.ty.ndims_val(ctx);
        let nidx_leq_ndims =
            ctx.builder.build_int_compare(IntPredicate::SLE, indices_len, ndims, "")?;
        ctx.make_assert(
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        )?;

        let len = call_int_umin(ctx, indices_len, ndims, None)?;
        gen_for_callback_incrementing(
            &mut (),
            ctx,
            None,
            llvm_usize.const_zero(),
            (len, false),
            |(), ctx, _, i| {
                let (dim_idx, dim_sz) = (
                    idx.get_unchecked::<IntValue<'ctx>>(ctx, &i, None)?,
                    self.inner_value(ctx)?
                        .shape(ctx)?
                        .inner_value(ctx, None)?
                        .get_unchecked::<IntValue<'ctx>>(ctx, &i, None)?,
                );
                let dim_idx =
                    ctx.builder.build_int_z_extend_or_bit_cast(dim_idx, dim_sz.get_type(), "")?;

                let dim_lt =
                    ctx.builder.build_int_compare(IntPredicate::SLT, dim_idx, dim_sz, "")?;

                ctx.make_assert(
                    dim_lt,
                    "0:IndexError",
                    "index {0} is out of bounds for axis 0 with size {1}",
                    [Some(dim_idx), Some(dim_sz), None],
                    ctx.current_loc,
                )?;

                Ok(())
            },
            llvm_usize.const_int(1, false),
            |(), _| Ok(()),
        )?;

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
    src_shape: ArraySliceValue<'ctx, IntType<'ctx>>,
    dst_shape: ArraySliceValue<'ctx, IntType<'ctx>>,
) -> anyhow::Result<()> {
    let name =
        get_usize_dependent_function_name(ctx, "__nac3_ndarray_util_assert_output_shape_same");
    let ((src_ptr, src_len), (dst_ptr, dst_len)) = (src_shape.value, dst_shape.value);
    call_extern!(ctx: (ctx.size_t) _ = name(src_len, src_ptr, dst_len, dst_ptr))?;
    Ok(())
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
                let llvm_ndarray_ty = RawNDArrayType::from_unifier_type(ctx, object_ty);
                let ndarray = TypedRefCountedType::new(ctx, llvm_ndarray_ty)
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
    pub fn to_ndarray(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> anyhow::Result<NDArrayValue<'ctx>> {
        match self {
            ScalarOrNDArray::Scalar(v) => RawNDArrayValue::new_scalar(ctx, *v, None),
            ScalarOrNDArray::NDArray(val) => Ok(*val),
        }
    }

    /// Get the dtype of the ndarray created if this were called with
    /// [`ScalarOrNDArray::to_ndarray`].
    #[must_use]
    pub fn get_dtype(&self) -> BasicTypeEnum<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(v) => v.get_type(),
            ScalarOrNDArray::NDArray(val) => val.ty.object.dtype,
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
            NDArrayOut::WriteToNDArray { ndarray } => ndarray.ty.object.dtype,
        }
    }

    /// Produce an `NDArrayValue` according to this output specification and the actual
    /// required output shape.
    pub fn resolve(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ndims: u64,
        shape: ArraySliceValue<'ctx, IntType<'ctx>>,
    ) -> anyhow::Result<NDArrayValue<'ctx>> {
        match self {
            NDArrayOut::NewNDArray { dtype } => {
                let result_ndarray =
                    NDArrayType::create(ctx, *dtype, ndims).construct(ctx, None)?;
                result_ndarray
                    .shape(ctx)?
                    .inner_value(ctx, None)?
                    .memcpy_from(ctx, shape.value.0)?;
                result_ndarray.create_data(ctx)?;
                Ok(result_ndarray)
            }

            NDArrayOut::WriteToNDArray { ndarray: result } => {
                // Use an existing ndarray.
                let out_shape = result.shape(ctx)?;
                assert_ndarray_can_be_written_by_out(
                    ctx,
                    shape,
                    out_shape.inner_value(ctx, None)?,
                )?;
                Ok(*result)
            }
        }
    }
}
