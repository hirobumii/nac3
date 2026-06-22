#pragma once

#include "irrt/stdlib/cstdint.h"
#include "irrt/stdlib/type_traits.h"

#include "irrt/reference/array.hpp"
#include "irrt/reference/header.hpp"

namespace __nac3_impl {
namespace {
/**
 * @brief The NDArray object
 *
 * Official numpy implementation:
 * https://github.com/numpy/numpy/blob/735a477f0bc2b5b84d0e72d92f224bde78d4e069/doc/source/reference/c-api/types-and-structures.rst#pyarrayinterface
 *
 * Note that this implementation is based on `tinynumpy`'s `ndarray` class as opposed to the official numpy
 * implementation:
 * https://github.com/wadetb/tinynumpy/blob/0d23d22e07062ffab2afa287374c7b366eebdda1/tinynumpy/tinynumpy.py#L458
 *
 * TODO
 */
template<typename SizeT>
struct NDArray {
    /**
     * @brief The object header for reference counting.
     */
    __nac3_impl::reference::ObjectHeader header;

    /**
     * @brief The number of bytes of a single element in `data`.
     */
    __nac3_impl::stdlib::make_signed_t<SizeT> itemsize;

    /**
     * @brief The number of dimensions of this shape.
     */
    __nac3_impl::stdlib::make_signed_t<SizeT> ndims;

    /**
     * @brief The NDArray shape, with length equal to `ndims`.
     *
     * Note that it may contain 0.
     */
    __nac3_impl::reference::Array<SizeT, __nac3_impl::stdlib::make_signed_t<SizeT>>* shape;

    /**
     * @brief Array strides, with length equal to `ndims`
     *
     * The stride values are in units of bytes, not number of elements.
     *
     * Note that `strides` can have negative values or contain 0.
     */
    __nac3_impl::reference::Array<SizeT, __nac3_impl::stdlib::make_signed_t<SizeT>>* strides;

    /**
     * @brief The underlying data this `ndarray` is pointing to.
     */
    __nac3_impl::reference::Array<SizeT>* data;

    /**
     * @brief The base array that owns the memory this `ndarray` is pointing to.
     */
    NDArray<SizeT>* base;

    /**
     * @brief The offset in bytes from the start of the base array to the first element of this `ndarray`.
     */
    __nac3_impl::stdlib::make_signed_t<SizeT> offset;
};
}  // namespace
}  // namespace __nac3_impl
