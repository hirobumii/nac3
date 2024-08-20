#pragma once

#include "irrt/int_types.hpp"

namespace {
/**
 * @brief The NDArray object
 *
 * Official numpy implementation:
 * https://github.com/numpy/numpy/blob/735a477f0bc2b5b84d0e72d92f224bde78d4e069/doc/source/reference/c-api/types-and-structures.rst
 */
template<typename SizeT>
struct NDArray {
    /**
     * @brief The underlying data this `ndarray` is pointing to.
     */
    uint8_t* data;

    /**
     * @brief The number of bytes of a single element in `data`.
     */
    SizeT itemsize;

    /**
     * @brief The number of dimensions of this shape.
     */
    SizeT ndims;

    /**
     * @brief The NDArray shape, with length equal to `ndims`.
     *
     * Note that it may contain 0.
     */
    SizeT* shape;

    /**
     * @brief Array strides, with length equal to `ndims`
     *
     * The stride values are in units of bytes, not number of elements.
     *
     * Note that `strides` can have negative values or contain 0.
     */
    SizeT* strides;
};
}  // namespace