#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/type_traits.h"

#include "irrt/reference/array.hpp"
#include "irrt/reference/header.hpp"

namespace __nac3_impl {
namespace {
namespace ndarray {
/**
 * @brief The signed integer type used for all NDArray index, size, stride, and offset arithmetic.
 *
 * Modelled after numpy's `npy_intp`: a single pointer-width signed integer used uniformly across all
 * NDArray operations, so that negative strides and offset arithmetic work without crossing
 * signed/unsigned conversion boundaries.
 */
using intp_t = stdlib::make_signed_t<size_t>;

/**
 * @brief The NDArray object
 *
 * Official numpy implementation:
 * https://github.com/numpy/numpy/blob/735a477f0bc2b5b84d0e72d92f224bde78d4e069/doc/source/reference/c-api/types-and-structures.rst#pyarrayinterface
 *
 * Note that this implementation is based on `tinynumpy`'s `ndarray` class as opposed to the official numpy
 * implementation:
 * https://github.com/wadetb/tinynumpy/blob/0d23d22e07062ffab2afa287374c7b366eebdda1/tinynumpy/tinynumpy.py#L458
 */
struct NDArray {
    /**
     * @brief The object header for reference counting.
     */
    reference::ObjectHeader header;

    /**
     * @brief The number of bytes of a single element in `data`.
     */
    ndarray::intp_t itemsize;

    /**
     * @brief The number of dimensions of this shape.
     */
    ndarray::intp_t ndims;

    /**
     * @brief The NDArray shape, with length equal to `ndims`.
     *
     * Note that it may contain 0.
     */
    reference::Array<ndarray::intp_t>* shape;

    /**
     * @brief Array strides, with length equal to `ndims`
     *
     * The stride values are in units of bytes, not number of elements.
     *
     * Note that `strides` can have negative values or contain 0.
     */
    reference::Array<ndarray::intp_t>* strides;

    /**
     * @brief The underlying data this `ndarray` is pointing to.
     */
    reference::Array<>* data;

    /**
     * @brief The base array that owns the memory this `ndarray` is pointing to.
     */
    NDArray* base;

    /**
     * @brief The offset in bytes from the start of the base array to the first element of this `ndarray`.
     */
    intp_t offset;
};
}  // namespace ndarray
}  // namespace
}  // namespace __nac3_impl
