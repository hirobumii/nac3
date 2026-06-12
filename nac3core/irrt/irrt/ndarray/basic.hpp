#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/slice.hpp"

namespace {
namespace ndarray::basic {
/**
 * @brief Assert that `shape` does not contain negative dimensions.
 *
 * @param ndims Number of dimensions in `shape`
 * @param shape The shape to check on
 */
template<typename SizeT>
void assert_shape_no_negative(SizeT ndims, const SizeT* shape) {
    for (SizeT axis = 0; axis < ndims; axis++) {
        if (shape[axis] < 0) {
            raise_exception(SizeT, EXN_VALUE_ERROR,
                            "negative dimensions are not allowed; axis {0} "
                            "has dimension {1}",
                            axis, shape[axis], NO_PARAM);
        }
    }
}

/**
 * @brief Assert that two shapes are the same in the context of writing output to an ndarray.
 */
template<typename SizeT>
void assert_output_shape_same(SizeT ndarray_ndims,
                              const SizeT* ndarray_shape,
                              SizeT output_ndims,
                              const SizeT* output_shape) {
    if (ndarray_ndims != output_ndims) {
        // There is no corresponding NumPy error message like this.
        raise_exception(SizeT, EXN_VALUE_ERROR, "Cannot write output of ndims {0} to an ndarray with ndims {1}",
                        output_ndims, ndarray_ndims, NO_PARAM);
    }

    for (SizeT axis = 0; axis < ndarray_ndims; axis++) {
        if (ndarray_shape[axis] != output_shape[axis]) {
            // There is no corresponding NumPy error message like this.
            raise_exception(SizeT, EXN_VALUE_ERROR,
                            "Mismatched dimensions on axis {0}, output has "
                            "dimension {1}, but destination ndarray has dimension {2}.",
                            axis, output_shape[axis], ndarray_shape[axis]);
        }
    }
}

/**
 * @brief Return the number of elements of an ndarray given its shape.
 *
 * @param ndims Number of dimensions in `shape`
 * @param shape The shape of the ndarray
 */
template<typename SizeT>
SizeT calc_size_from_shape(SizeT ndims, const SizeT* shape) {
    SizeT size = 1;
    for (SizeT axis = 0; axis < ndims; axis++)
        size *= shape[axis];
    return size;
}

/**
 * @brief Compute the array indices of the `nth` (0-based) element of an ndarray given only its shape.
 *
 * @param ndims Number of elements in `shape` and `indices`
 * @param shape The shape of the ndarray
 * @param indices The returned indices indexing the ndarray with shape `shape`.
 * @param nth The index of the element of interest.
 */
template<typename SizeT>
void set_indices_by_nth(SizeT ndims, const SizeT* shape, SizeT* indices, SizeT nth) {
    for (SizeT i = 0; i < ndims; i++) {
        SizeT axis = ndims - i - 1;
        SizeT dim = shape[axis];

        indices[axis] = nth % dim;
        nth /= dim;
    }
}

/**
 * @brief Return the number of elements of an `ndarray`
 *
 * This function corresponds to `<an_ndarray>.size`
 */
template<typename SizeT>
SizeT size(const NDArray<SizeT>* ndarray) {
    return calc_size_from_shape(ndarray->ndims, ndarray->shape->data());
}

/**
 * @brief Return of the number of its content of an `ndarray`.
 *
 * This function corresponds to `<an_ndarray>.nbytes`.
 */
template<typename SizeT>
SizeT nbytes(const NDArray<SizeT>* ndarray) {
    return size(ndarray) * ndarray->itemsize;
}

/**
 * @brief Get the `len()` of an ndarray, and asserts that `ndarray` is a sized object.
 *
 * This function corresponds to `<an_ndarray>.__len__`.
 *
 * @param dst_length The length.
 */
template<typename SizeT>
SizeT len(const NDArray<SizeT>* ndarray) {
    if (ndarray->ndims != 0) {
        return ndarray->shape->data()[0];
    }

    // numpy prohibits `__len__` on unsized objects
    raise_exception(SizeT, EXN_TYPE_ERROR, "len() of unsized object", NO_PARAM, NO_PARAM, NO_PARAM);
    __builtin_unreachable();
}

/**
 * @brief Return a boolean indicating if `ndarray` is (C-)contiguous.
 *
 * You may want to see ndarray's rules for C-contiguity:
 * https://github.com/numpy/numpy/blob/df256d0d2f3bc6833699529824781c58f9c6e697/numpy/core/src/multiarray/flagsobject.c#L95C1-L99C45
 */
template<typename SizeT>
bool is_c_contiguous(const NDArray<SizeT>* ndarray) {
    // References:
    // - tinynumpy's implementation:
    // https://github.com/wadetb/tinynumpy/blob/0d23d22e07062ffab2afa287374c7b366eebdda1/tinynumpy/tinynumpy.py#L102
    // - ndarray's flags["C_CONTIGUOUS"]:
    // https://numpy.org/doc/stable/reference/generated/numpy.ndarray.flags.html#numpy.ndarray.flags
    // - ndarray's rules for C-contiguity:
    // https://github.com/numpy/numpy/blob/df256d0d2f3bc6833699529824781c58f9c6e697/numpy/core/src/multiarray/flagsobject.c#L95C1-L99C45

    // From
    // https://github.com/numpy/numpy/blob/df256d0d2f3bc6833699529824781c58f9c6e697/numpy/core/src/multiarray/flagsobject.c#L95C1-L99C45:
    //
    // The traditional rule is that for an array to be flagged as C contiguous,
    // the following must hold:
    //
    // strides[-1] == itemsize
    // strides[i] == shape[i+1] * strides[i + 1]
    // [...]
    // According to these rules, a 0- or 1-dimensional array is either both
    // C- and F-contiguous, or neither; and an array with 2+ dimensions
    // can be C- or F- contiguous, or neither, but not both. Though there
    // there are exceptions for arrays with zero or one item, in the first
    // case the check is relaxed up to and including the first dimension
    // with shape[i] == 0. In the second case `strides == itemsize` will
    // can be true for all dimensions and both flags are set.

    if (ndarray->ndims == 0) {
        return true;
    }

    if (ndarray->strides->data()[ndarray->ndims - 1] != ndarray->itemsize) {
        return false;
    }

    for (auto i = 1; i < ndarray->ndims; i++) {
        auto axis_i = ndarray->ndims - i - 1;
        if (ndarray->strides->data()[axis_i]
            != ndarray->shape->data()[axis_i + 1] * ndarray->strides->data()[axis_i + 1]) {
            return false;
        }
    }

    return true;
}

/**
 * @brief Return the pointer to the element indexed by `indices` along the ndarray's axes.
 *
 * This function does no bound check.
 */
template<typename SizeT>
void* get_pelement_by_indices(const NDArray<SizeT>* ndarray, const __nac3_impl::stdlib::make_signed_t<SizeT>* indices) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (auto dim_i = 0; dim_i < ndarray->ndims; dim_i++)
        element = static_cast<uint8_t*>(element) + indices[dim_i] * ndarray->strides->data()[dim_i];
    return element;
}

/**
 * @brief Return the pointer to the single element selected by `indices`, performing Python-style
 * negative-index resolution and bounds checking on every axis.
 *
 * This is the fast path for indexing that selects exactly one scalar element (by having exactly one integer index per
 * axis).
 *
 * The caller must guarantee `indices` has exactly `ndarray->ndims` elements.
 */
template<typename SizeT>
void* get_pelement_by_indices_single(const NDArray<SizeT>* ndarray,
                                     const __nac3_impl::stdlib::make_signed_t<SizeT>* indices) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (__nac3_impl::stdlib::make_signed_t<SizeT> axis = 0; axis < ndarray->ndims; axis++) {
        auto input = static_cast<__nac3_impl::stdlib::make_signed_t<SizeT>>(indices[axis]);
        auto k = slice::resolve_index_in_length(ndarray->shape->data()[axis], input);
        if (k == -1) {
            raise_exception(SizeT, EXN_INDEX_ERROR, "index {0} is out of bounds for axis {1} with size {2}", input,
                            axis, ndarray->shape->data()[axis]);
        }
        element = static_cast<uint8_t*>(element) + k * ndarray->strides->data()[axis];
    }
    return element;
}

/**
 * @brief Return the pointer to the nth (0-based) element of `ndarray` in flattened view.
 *
 * This function does no bound check.
 */
template<typename SizeT>
void* get_nth_pelement(const NDArray<SizeT>* ndarray, __nac3_impl::stdlib::make_signed_t<SizeT> nth) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (auto i = 0; i < ndarray->ndims; i++) {
        auto axis = ndarray->ndims - i - 1;
        auto dim = ndarray->shape->data()[axis];
        element = static_cast<uint8_t*>(element) + ndarray->strides->data()[axis] * (nth % dim);
        nth /= dim;
    }
    return element;
}

/**
 * @brief Update the strides of an ndarray given an ndarray `shape` to be contiguous.
 *
 * You might want to read https://ajcr.net/stride-guide-part-1/.
 */
template<typename SizeT>
void set_strides_by_shape(NDArray<SizeT>* ndarray) {
    __nac3_impl::stdlib::make_signed_t<SizeT> stride_product = 1;
    for (auto i = 0; i < ndarray->ndims; i++) {
        auto axis = ndarray->ndims - i - 1;
        ndarray->strides->data()[axis] = stride_product * ndarray->itemsize;
        stride_product *= ndarray->shape->data()[axis];
    }
}

/**
 * @brief Set an element in `ndarray`.
 *
 * @param pelement Pointer to the element in `ndarray` to be set.
 * @param pvalue Pointer to the value `pelement` will be set to.
 */
template<typename SizeT>
void set_pelement_value(NDArray<SizeT>* ndarray, void* pelement, const void* pvalue) {
    __builtin_memcpy(pelement, pvalue, ndarray->itemsize);
}

/**
 * @brief Copy data from one ndarray to another of the exact same size and itemsize.
 *
 * Both ndarrays will be viewed in their flatten views when copying the elements.
 */
template<typename SizeT>
void copy_data(const NDArray<SizeT>* src_ndarray, NDArray<SizeT>* dst_ndarray) {
    // TODO: Make this faster with memcpy when we see a contiguous segment.
    // TODO: Handle overlapping.

    debug_assert_eq(SizeT, src_ndarray->itemsize, dst_ndarray->itemsize);

    for (SizeT i = 0; i < size(src_ndarray); i++) {
        auto src_element = ndarray::basic::get_nth_pelement(src_ndarray, i);
        auto dst_element = ndarray::basic::get_nth_pelement(dst_ndarray, i);
        ndarray::basic::set_pelement_value(dst_ndarray, dst_element, src_element);
    }
}
}  // namespace ndarray::basic
}  // namespace

extern "C" {
using namespace ndarray::basic;

void __nac3_ndarray_util_assert_shape_no_negative(int32_t ndims, int32_t* shape) {
    assert_shape_no_negative(ndims, shape);
}

void __nac3_ndarray_util_assert_shape_no_negative64(int64_t ndims, int64_t* shape) {
    assert_shape_no_negative(ndims, shape);
}

void __nac3_ndarray_util_assert_output_shape_same(int32_t ndarray_ndims,
                                                  const int32_t* ndarray_shape,
                                                  int32_t output_ndims,
                                                  const int32_t* output_shape) {
    assert_output_shape_same(ndarray_ndims, ndarray_shape, output_ndims, output_shape);
}

void __nac3_ndarray_util_assert_output_shape_same64(int64_t ndarray_ndims,
                                                    const int64_t* ndarray_shape,
                                                    int64_t output_ndims,
                                                    const int64_t* output_shape) {
    assert_output_shape_same(ndarray_ndims, ndarray_shape, output_ndims, output_shape);
}

uint32_t __nac3_ndarray_size(NDArray<uint32_t>* ndarray) {
    return size(ndarray);
}

uint64_t __nac3_ndarray_size64(NDArray<uint64_t>* ndarray) {
    return size(ndarray);
}

uint32_t __nac3_ndarray_nbytes(NDArray<uint32_t>* ndarray) {
    return nbytes(ndarray);
}

uint64_t __nac3_ndarray_nbytes64(NDArray<uint64_t>* ndarray) {
    return nbytes(ndarray);
}

int32_t __nac3_ndarray_len(NDArray<uint32_t>* ndarray) {
    return len(ndarray);
}

int64_t __nac3_ndarray_len64(NDArray<uint64_t>* ndarray) {
    return len(ndarray);
}

bool __nac3_ndarray_is_c_contiguous(NDArray<uint32_t>* ndarray) {
    return is_c_contiguous(ndarray);
}

bool __nac3_ndarray_is_c_contiguous64(NDArray<uint64_t>* ndarray) {
    return is_c_contiguous(ndarray);
}

void* __nac3_ndarray_get_nth_pelement(const NDArray<uint32_t>* ndarray, int32_t nth) {
    return get_nth_pelement(ndarray, nth);
}

void* __nac3_ndarray_get_nth_pelement64(const NDArray<uint64_t>* ndarray, int64_t nth) {
    return get_nth_pelement(ndarray, nth);
}

void* __nac3_ndarray_get_pelement_by_indices(const NDArray<uint32_t>* ndarray, int32_t* indices) {
    return get_pelement_by_indices(ndarray, indices);
}

void* __nac3_ndarray_get_pelement_by_indices64(const NDArray<uint64_t>* ndarray, int64_t* indices) {
    return get_pelement_by_indices(ndarray, indices);
}

void* __nac3_ndarray_get_pelement_by_indices_single(const NDArray<uint32_t>* ndarray, int32_t* indices) {
    return get_pelement_by_indices_single(ndarray, indices);
}

void* __nac3_ndarray_get_pelement_by_indices_single64(const NDArray<uint64_t>* ndarray, int64_t* indices) {
    return get_pelement_by_indices_single(ndarray, indices);
}

void __nac3_ndarray_set_strides_by_shape(NDArray<uint32_t>* ndarray) {
    set_strides_by_shape(ndarray);
}

void __nac3_ndarray_set_strides_by_shape64(NDArray<uint64_t>* ndarray) {
    set_strides_by_shape(ndarray);
}

void __nac3_ndarray_copy_data(NDArray<uint32_t>* src_ndarray, NDArray<uint32_t>* dst_ndarray) {
    copy_data(src_ndarray, dst_ndarray);
}

void __nac3_ndarray_copy_data64(NDArray<uint64_t>* src_ndarray, NDArray<uint64_t>* dst_ndarray) {
    copy_data(src_ndarray, dst_ndarray);
}
}
