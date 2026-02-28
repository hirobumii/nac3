#pragma once

#include "irrt/stdlib/cstdint.h"

namespace {
/**
 * @brief The NDArray object
 *
 * Official numpy implementation:
 * https://github.com/numpy/numpy/blob/735a477f0bc2b5b84d0e72d92f224bde78d4e069/doc/source/reference/c-api/types-and-structures.rst#pyarrayinterface
 *
 * Note that this implementation is based on `PyArrayInterface` rather of `PyArrayObject`. The
 * difference between `PyArrayInterface` and `PyArrayObject` (relevant to our implementation) is
 * that `PyArrayInterface` *has* `itemsize` and uses `void*` for its `data`, whereas `PyArrayObject`
 * does not require `itemsize` (probably using `strides[-1]` instead) and uses `char*` for its
 * `data`. There are also minor differences in the struct layout.
 */
template<typename SizeT>
struct NDArray {
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

    /**
     * @brief The underlying data this `ndarray` is pointing to.
     */
    void* data;

    /**
     * @brief The base array that owns the memory this `ndarray` is pointing to.
     */
    NDArray<SizeT>* base;

    /**
     * @brief The offset in bytes from the start of the base array to the first element of this `ndarray`.
     */
    SizeT offset;
};
}  // namespace
