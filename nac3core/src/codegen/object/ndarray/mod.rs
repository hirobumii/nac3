use inkwell::{
    context::Context,
    types::BasicType,
    values::{BasicValue, BasicValueEnum, PointerValue},
    AddressSpace,
};

use super::any::AnyObject;
use crate::{
    codegen::{
        irrt::{
            call_nac3_ndarray_copy_data, call_nac3_ndarray_get_nth_pelement,
            call_nac3_ndarray_get_pelement_by_indices, call_nac3_ndarray_is_c_contiguous,
            call_nac3_ndarray_len, call_nac3_ndarray_nbytes,
            call_nac3_ndarray_set_strides_by_shape, call_nac3_ndarray_size,
        },
        model::*,
        CodeGenContext, CodeGenerator,
    },
    toplevel::{helper::extract_ndims, numpy::unpack_ndarray_var_tys},
    typecheck::typedef::Type,
};

pub mod factory;
pub mod indexing;
pub mod nditer;
pub mod shape_util;

/// Fields of [`NDArray`]
pub struct NDArrayFields<'ctx, F: FieldTraversal<'ctx>> {
    pub data: F::Output<Ptr<Int<Byte>>>,
    pub itemsize: F::Output<Int<SizeT>>,
    pub ndims: F::Output<Int<SizeT>>,
    pub shape: F::Output<Ptr<Int<SizeT>>>,
    pub strides: F::Output<Ptr<Int<SizeT>>>,
}

/// A strided ndarray in NAC3.
///
/// See IRRT implementation for details about its fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct NDArray;

impl<'ctx> StructKind<'ctx> for NDArray {
    type Fields<F: FieldTraversal<'ctx>> = NDArrayFields<'ctx, F>;

    fn iter_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields {
            data: traversal.add_auto("data"),
            itemsize: traversal.add_auto("itemsize"),
            ndims: traversal.add_auto("ndims"),
            shape: traversal.add_auto("shape"),
            strides: traversal.add_auto("strides"),
        }
    }
}

/// A NAC3 Python ndarray object.
#[derive(Debug, Clone, Copy)]
pub struct NDArrayObject<'ctx> {
    pub dtype: Type,
    pub ndims: u64,
    pub instance: Instance<'ctx, Ptr<Struct<NDArray>>>,
}

impl<'ctx> NDArrayObject<'ctx> {
    /// Attempt to convert an [`AnyObject`] into an [`NDArrayObject`].
    pub fn from_object<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        object: AnyObject<'ctx>,
    ) -> NDArrayObject<'ctx> {
        let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, object.ty);
        let ndims = extract_ndims(&ctx.unifier, ndims);

        let value = Ptr(Struct(NDArray)).check_value(generator, ctx.ctx, object.value).unwrap();
        NDArrayObject { dtype, ndims, instance: value }
    }

    /// Get this ndarray's `ndims` as an LLVM constant.
    pub fn ndims_llvm<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &'ctx Context,
    ) -> Instance<'ctx, Int<SizeT>> {
        Int(SizeT).const_int(generator, ctx, self.ndims, false)
    }

    /// Allocate an ndarray on the stack given its `ndims` and `dtype`.
    ///
    /// `shape` and `strides` will be automatically allocated onto the stack.
    ///
    /// The returned ndarray's content will be:
    /// - `data`: uninitialized.
    /// - `itemsize`: set to the `sizeof()` of `dtype`.
    /// - `ndims`: set to the value of  `ndims`.
    /// - `shape`: allocated with an array of length `ndims` with uninitialized values.
    /// - `strides`: allocated with an array of length `ndims` with uninitialized values.
    pub fn alloca<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        ndims: u64,
    ) -> Self {
        let ndarray = Struct(NDArray).alloca(generator, ctx);

        let itemsize = ctx.get_llvm_type(generator, dtype).size_of().unwrap();
        let itemsize = Int(SizeT).z_extend_or_truncate(generator, ctx, itemsize);
        ndarray.set(ctx, |f| f.itemsize, itemsize);

        let ndims_val = Int(SizeT).const_int(generator, ctx.ctx, ndims, false);
        ndarray.set(ctx, |f| f.ndims, ndims_val);

        let shape = Int(SizeT).array_alloca(generator, ctx, ndims_val.value);
        ndarray.set(ctx, |f| f.shape, shape);

        let strides = Int(SizeT).array_alloca(generator, ctx, ndims_val.value);
        ndarray.set(ctx, |f| f.strides, strides);

        NDArrayObject { dtype, ndims, instance: ndarray }
    }

    /// Convenience function. Allocate an [`NDArrayObject`] with a statically known shape.
    ///
    /// The returned [`NDArrayObject`]'s `data` and `strides` are uninitialized.
    pub fn alloca_constant_shape<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: &[u64],
    ) -> Self {
        let ndarray = NDArrayObject::alloca(generator, ctx, dtype, shape.len() as u64);

        // Write shape
        let dst_shape = ndarray.instance.get(generator, ctx, |f| f.shape);
        for (i, dim) in shape.iter().enumerate() {
            let dim = Int(SizeT).const_int(generator, ctx.ctx, *dim, false);
            dst_shape.offset_const(ctx, i64::try_from(i).unwrap()).store(ctx, dim);
        }

        ndarray
    }

    /// Convenience function. Allocate an [`NDArrayObject`] with a dynamically known shape.
    ///
    /// The returned [`NDArrayObject`]'s `data` and `strides` are uninitialized.
    pub fn alloca_dynamic_shape<G: CodeGenerator + ?Sized>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dtype: Type,
        shape: &[Instance<'ctx, Int<SizeT>>],
    ) -> Self {
        let ndarray = NDArrayObject::alloca(generator, ctx, dtype, shape.len() as u64);

        // Write shape
        let dst_shape = ndarray.instance.get(generator, ctx, |f| f.shape);
        for (i, dim) in shape.iter().enumerate() {
            dst_shape.offset_const(ctx, i64::try_from(i).unwrap()).store(ctx, *dim);
        }

        ndarray
    }

    /// Initialize an ndarray's `data` by allocating a buffer on the stack.
    /// The allocated data buffer is considered to be *owned* by the ndarray.
    ///
    /// `strides` of the ndarray will also be updated with `set_strides_by_shape`.
    ///
    /// `shape` and `itemsize` of the ndarray ***must*** be initialized first.
    pub fn create_data<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) {
        let nbytes = self.nbytes(generator, ctx);

        let data = Int(Byte).array_alloca(generator, ctx, nbytes.value);
        self.instance.set(ctx, |f| f.data, data);

        self.set_strides_contiguous(generator, ctx);
    }

    /// Copy shape dimensions from an array.
    pub fn copy_shape_from_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        shape: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) {
        let num_items = self.ndims_llvm(generator, ctx.ctx).value;
        self.instance.get(generator, ctx, |f| f.shape).copy_from(generator, ctx, shape, num_items);
    }

    /// Copy shape dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_shape_from_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayObject<'ctx>,
    ) {
        assert_eq!(self.ndims, src_ndarray.ndims);
        let src_shape = src_ndarray.instance.get(generator, ctx, |f| f.shape);
        self.copy_shape_from_array(generator, ctx, src_shape);
    }

    /// Copy strides dimensions from an array.
    pub fn copy_strides_from_array<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        strides: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) {
        let num_items = self.ndims_llvm(generator, ctx.ctx).value;
        self.instance
            .get(generator, ctx, |f| f.strides)
            .copy_from(generator, ctx, strides, num_items);
    }

    /// Copy strides dimensions from an ndarray.
    /// Panics if `ndims` mismatches.
    pub fn copy_strides_from_ndarray<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src_ndarray: NDArrayObject<'ctx>,
    ) {
        assert_eq!(self.ndims, src_ndarray.ndims);
        let src_strides = src_ndarray.instance.get(generator, ctx, |f| f.strides);
        self.copy_strides_from_array(generator, ctx, src_strides);
    }

    /// Get the `np.size()` of this ndarray.
    pub fn size<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        call_nac3_ndarray_size(generator, ctx, self.instance)
    }

    /// Get the `ndarray.nbytes` of this ndarray.
    pub fn nbytes<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        call_nac3_ndarray_nbytes(generator, ctx, self.instance)
    }

    /// Get the `len()` of this ndarray.
    pub fn len<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<SizeT>> {
        call_nac3_ndarray_len(generator, ctx, self.instance)
    }

    /// Check if this ndarray is C-contiguous.
    ///
    /// See NumPy's `flags["C_CONTIGUOUS"]`: <https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html#numpy.ndarray.flags>
    pub fn is_c_contiguous<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Instance<'ctx, Int<Bool>> {
        call_nac3_ndarray_is_c_contiguous(generator, ctx, self.instance)
    }

    /// Get the pointer to the n-th (0-based) element.
    ///
    /// The returned pointer has the element type of the LLVM type of this ndarray's `dtype`.
    pub fn get_nth_pelement<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        nth: Instance<'ctx, Int<SizeT>>,
    ) -> PointerValue<'ctx> {
        let elem_ty = ctx.get_llvm_type(generator, self.dtype);

        let p = call_nac3_ndarray_get_nth_pelement(generator, ctx, self.instance, nth);
        ctx.builder
            .build_pointer_cast(p.value, elem_ty.ptr_type(AddressSpace::default()), "")
            .unwrap()
    }

    /// Get the n-th (0-based) scalar.
    pub fn get_nth_scalar<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        nth: Instance<'ctx, Int<SizeT>>,
    ) -> AnyObject<'ctx> {
        let ptr = self.get_nth_pelement(generator, ctx, nth);
        let value = ctx.builder.build_load(ptr, "").unwrap();
        AnyObject { ty: self.dtype, value }
    }

    /// Get the pointer to the element indexed by `indices`.
    ///
    /// The returned pointer has the element type of the LLVM type of this ndarray's `dtype`.
    pub fn get_pelement_by_indices<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        indices: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> PointerValue<'ctx> {
        let elem_ty = ctx.get_llvm_type(generator, self.dtype);

        let p = call_nac3_ndarray_get_pelement_by_indices(generator, ctx, self.instance, indices);
        ctx.builder
            .build_pointer_cast(p.value, elem_ty.ptr_type(AddressSpace::default()), "")
            .unwrap()
    }

    /// Get the scalar indexed by `indices`.
    pub fn get_scalar_by_indices<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        indices: Instance<'ctx, Ptr<Int<SizeT>>>,
    ) -> AnyObject<'ctx> {
        let ptr = self.get_pelement_by_indices(generator, ctx, indices);
        let value = ctx.builder.build_load(ptr, "").unwrap();
        AnyObject { ty: self.dtype, value }
    }

    /// Call [`call_nac3_ndarray_set_strides_by_shape`] on this ndarray to update `strides`.
    ///
    /// Update the ndarray's strides to make the ndarray contiguous.
    pub fn set_strides_contiguous<G: CodeGenerator + ?Sized>(
        self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) {
        call_nac3_ndarray_set_strides_by_shape(generator, ctx, self.instance);
    }

    /// Copy data from another ndarray.
    ///
    /// This ndarray and `src` is that their `np.size()` should be the same. Their shapes
    /// do not matter. The copying order is determined by how their flattened views look.
    ///
    /// Panics if the `dtype`s of ndarrays are different.
    pub fn copy_data_from<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        src: NDArrayObject<'ctx>,
    ) {
        assert!(ctx.unifier.unioned(self.dtype, src.dtype), "self and src dtype should match");
        call_nac3_ndarray_copy_data(generator, ctx, src.instance, self.instance);
    }

    /// Returns true if this ndarray is unsized - `ndims == 0` and only contains a scalar.
    #[must_use]
    pub fn is_unsized(&self) -> bool {
        self.ndims == 0
    }

    /// If this ndarray is unsized, return its sole value as an [`AnyObject`].
    /// Otherwise, do nothing and return the ndarray itself.
    pub fn split_unsized<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> ScalarOrNDArray<'ctx> {
        if self.is_unsized() {
            // NOTE: `np.size(self) == 0` here is never possible.
            let zero = Int(SizeT).const_0(generator, ctx.ctx);
            let value = self.get_nth_scalar(generator, ctx, zero).value;

            ScalarOrNDArray::Scalar(AnyObject { ty: self.dtype, value })
        } else {
            ScalarOrNDArray::NDArray(*self)
        }
    }

    /// Fill the ndarray with a scalar.
    ///
    /// `fill_value` must have the same LLVM type as the `dtype` of this ndarray.
    pub fn fill<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        value: BasicValueEnum<'ctx>,
    ) {
        self.foreach(generator, ctx, |generator, ctx, _hooks, nditer| {
            let p = nditer.get_pointer(generator, ctx);
            ctx.builder.build_store(p, value).unwrap();
            Ok(())
        })
        .unwrap();
    }
}

/// A convenience enum for implementing functions that acts on scalars or ndarrays or both.
#[derive(Debug, Clone, Copy)]
pub enum ScalarOrNDArray<'ctx> {
    Scalar(AnyObject<'ctx>),
    NDArray(NDArrayObject<'ctx>),
}

impl<'ctx> ScalarOrNDArray<'ctx> {
    /// Get the underlying [`BasicValueEnum<'ctx>`] of this [`ScalarOrNDArray`].
    #[must_use]
    pub fn to_basic_value_enum(self) -> BasicValueEnum<'ctx> {
        match self {
            ScalarOrNDArray::Scalar(scalar) => scalar.value,
            ScalarOrNDArray::NDArray(ndarray) => ndarray.instance.value.as_basic_value_enum(),
        }
    }
}
