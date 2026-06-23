#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/slice.hpp"

namespace __nac3_impl {
namespace {
namespace ndarray::basic {
/**
 * @brief Assert that `shape` does not contain negative dimensions.
 *
 * @param ndims Number of dimensions in `shape`
 * @param shape The shape to check on
 */
void assert_shape_no_negative(intp_t ndims, const intp_t* shape) {
    for (intp_t axis = 0; axis < ndims; axis++) {
        if (shape[axis] < 0) {
            raise_exception(EXN_VALUE_ERROR,
                            "negative dimensions are not allowed; axis {0} "
                            "has dimension {1}",
                            axis, shape[axis], NO_PARAM);
        }
    }
}

/**
 * @brief Assert that two shapes are the same in the context of writing output to an ndarray.
 */
void assert_output_shape_same(intp_t ndarray_ndims,
                              const intp_t* ndarray_shape,
                              intp_t output_ndims,
                              const intp_t* output_shape) {
    if (ndarray_ndims != output_ndims) {
        // There is no corresponding NumPy error message like this.
        raise_exception(EXN_VALUE_ERROR, "Cannot write output of ndims {0} to an ndarray with ndims {1}", output_ndims,
                        ndarray_ndims, NO_PARAM);
    }

    for (intp_t axis = 0; axis < ndarray_ndims; axis++) {
        if (ndarray_shape[axis] != output_shape[axis]) {
            // There is no corresponding NumPy error message like this.
            raise_exception(EXN_VALUE_ERROR,
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
intp_t calc_size_from_shape(intp_t ndims, const intp_t* shape) {
    intp_t size = 1;
    for (intp_t axis = 0; axis < ndims; axis++)
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
[[maybe_unused]] void set_indices_by_nth(intp_t ndims, const intp_t* shape, intp_t* indices, intp_t nth) {
    for (intp_t i = 0; i < ndims; i++) {
        intp_t axis = ndims - i - 1;
        intp_t dim = shape[axis];

        indices[axis] = nth % dim;
        nth /= dim;
    }
}

/**
 * @brief Return the number of elements of an `ndarray`
 *
 * This function corresponds to `<an_ndarray>.size`
 */
intp_t size(const NDArray* ndarray) {
    return calc_size_from_shape(ndarray->ndims, ndarray->shape->data());
}

/**
 * @brief Return of the number of its content of an `ndarray`.
 *
 * This function corresponds to `<an_ndarray>.nbytes`.
 */
intp_t nbytes(const NDArray* ndarray) {
    return size(ndarray) * ndarray->itemsize;
}

/**
 * @brief Get the `len()` of an ndarray, and asserts that `ndarray` is a sized object.
 *
 * This function corresponds to `<an_ndarray>.__len__`.
 *
 * @param dst_length The length.
 */
intp_t len(const NDArray* ndarray) {
    if (ndarray->ndims != 0) {
        return ndarray->shape->data()[0];
    }

    // numpy prohibits `__len__` on unsized objects
    raise_exception(EXN_TYPE_ERROR, "len() of unsized object", NO_PARAM, NO_PARAM, NO_PARAM);
    __builtin_unreachable();
}

/**
 * @brief Return a boolean indicating if `ndarray` is (C-)contiguous.
 *
 * You may want to see ndarray's rules for C-contiguity:
 * https://github.com/numpy/numpy/blob/df256d0d2f3bc6833699529824781c58f9c6e697/numpy/core/src/multiarray/flagsobject.c#L95C1-L99C45
 */
bool is_c_contiguous(const NDArray* ndarray) {
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

    for (intp_t i = 1; i < ndarray->ndims; i++) {
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
void* get_pelement_by_indices(const NDArray* ndarray, const intp_t* indices) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (intp_t dim_i = 0; dim_i < ndarray->ndims; dim_i++)
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
void* get_pelement_by_indices_single(const NDArray* ndarray, const intp_t* indices) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (intp_t axis = 0; axis < ndarray->ndims; axis++) {
        auto input = indices[axis];
        auto k = __nac3_impl::slice::resolve_index_in_length(ndarray->shape->data()[axis], input);
        if (k == -1) {
            raise_exception(EXN_INDEX_ERROR, "index {0} is out of bounds for axis {1} with size {2}", input, axis,
                            ndarray->shape->data()[axis]);
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
void* get_nth_pelement(const NDArray* ndarray, intp_t nth) {
    auto* element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
    for (intp_t i = 0; i < ndarray->ndims; i++) {
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
void set_strides_by_shape(NDArray* ndarray) {
    intp_t stride_product = 1;
    for (intp_t i = 0; i < ndarray->ndims; i++) {
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
void set_pelement_value(NDArray* ndarray, void* pelement, const void* pvalue) {
    __builtin_memcpy(pelement, pvalue, ndarray->itemsize);
}

/**
 * @brief Copy data from one ndarray to another of the exact same size and itemsize.
 *
 * Both ndarrays will be viewed in their flatten views when copying the elements.
 */
void copy_data(const NDArray* src_ndarray, NDArray* dst_ndarray) {
    // TODO: Make this faster with memcpy when we see a contiguous segment.
    // TODO: Handle overlapping.

    debug_assert_eq(src_ndarray->itemsize, dst_ndarray->itemsize);

    for (intp_t i = 0; i < size(src_ndarray); i++) {
        auto src_element = basic::get_nth_pelement(src_ndarray, i);
        auto dst_element = basic::get_nth_pelement(dst_ndarray, i);
        basic::set_pelement_value(dst_ndarray, dst_element, src_element);
    }
}
}  // namespace ndarray::basic
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray;

void __nac3_ndarray_util_assert_shape_no_negative(intp_t ndims, intp_t* shape) {
    basic::assert_shape_no_negative(ndims, shape);
}

void __nac3_ndarray_util_assert_output_shape_same(intp_t ndarray_ndims,
                                                  const intp_t* ndarray_shape,
                                                  intp_t output_ndims,
                                                  const intp_t* output_shape) {
    basic::assert_output_shape_same(ndarray_ndims, ndarray_shape, output_ndims, output_shape);
}

intp_t __nac3_ndarray_size(NDArray* ndarray) {
    return basic::size(ndarray);
}

intp_t __nac3_ndarray_nbytes(NDArray* ndarray) {
    return basic::nbytes(ndarray);
}

intp_t __nac3_ndarray_len(NDArray* ndarray) {
    return basic::len(ndarray);
}

bool __nac3_ndarray_is_c_contiguous(NDArray* ndarray) {
    return basic::is_c_contiguous(ndarray);
}

void* __nac3_ndarray_get_nth_pelement(const NDArray* ndarray, intp_t nth) {
    return basic::get_nth_pelement(ndarray, nth);
}

void* __nac3_ndarray_get_pelement_by_indices(const NDArray* ndarray, intp_t* indices) {
    return basic::get_pelement_by_indices(ndarray, indices);
}

void* __nac3_ndarray_get_pelement_by_indices_single(const NDArray* ndarray, intp_t* indices) {
    return basic::get_pelement_by_indices_single(ndarray, indices);
}

void __nac3_ndarray_set_strides_by_shape(NDArray* ndarray) {
    basic::set_strides_by_shape(ndarray);
}

void __nac3_ndarray_copy_data(NDArray* src_ndarray, NDArray* dst_ndarray) {
    basic::copy_data(src_ndarray, dst_ndarray);
}
}
