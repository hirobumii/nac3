#pragma once

#include "irrt/stdlib/cstdint.h"
#include "irrt/stdlib/type_traits.h"

#include "irrt/ndarray/def.hpp"
#include "irrt/reference/array.hpp"

namespace __nac3_impl {
namespace {
/**
 * @brief Helper struct to enumerate through an ndarray *efficiently*.
 *
 * Example usage (in pseudo-code):
 * ```
 * // Suppose my_ndarray has been initialized, with shape [2, 3] and dtype `double`
 * RawNDIter nditer;
 * nditer.initialize(my_ndarray);
 * while (nditer.has_element()) {
 *    // This body is run 6 (= my_ndarray.size) times.
 *
 *    // [0, 0] -> [0, 1] -> [0, 2] -> [1, 0] -> [1, 1] -> [1, 2] -> end
 *    print(nditer.indices);
 *
 *    // 0 -> 1 -> 2 -> 3 -> 4 -> 5
 *    print(nditer.nth);
 *
 *    // <1st element> -> <2nd element> -> ... -> <6th element> -> end
 *    print(*((double *) nditer.element))
 *
 *    nditer.next(); // Go to next element.
 * }
 * ```
 *
 * Interesting cases:
 * - If `my_ndarray.ndims` == 0, there is one iteration.
 * - If `my_ndarray.shape` contains zeroes, there are no iterations.
 */
template<typename SizeT>
struct RawNDIter {
    using Index = __nac3_impl::stdlib::make_signed_t<SizeT>;

    /**
     * @brief The ndarray being iterated.
     *
     * Must be allocated by the caller.
     */
    NDArray<SizeT>* array;

    /**
     * @brief The current indices.
     *
     * Must be allocated by the caller.
     */
    __nac3_impl::reference::Array<SizeT, Index>* indices;

    /**
     * @brief The nth (0-based) index of the current indices.
     *
     * Initially this is 0.
     */
    SizeT nth;

    /**
     * @brief Pointer to the current element.
     *
     * Initially this points to first element of the ndarray.
     */
    SizeT offset;

    /**
     * @brief Cache for the product of shape.
     *
     * Could be 0 if `shape` has 0s in it.
     */
    SizeT size;

    void initialize(NDArray<SizeT>* array, void* element, void* indices) {
        this->array = array;

        this->indices = static_cast<__nac3_impl::reference::Array<SizeT, Index>*>(indices);
        this->offset = static_cast<uint8_t*>(element) - static_cast<uint8_t*>(array->data->data());

        // Compute size
        this->size = 1;
        for (auto i = 0; i < this->array->ndims; i++) {
            this->size *= this->array->shape->data()[i];
        }

        // `indices` starts on all 0s.
        for (auto axis = 0; axis < this->array->ndims; axis++)
            this->indices->data()[axis] = 0;
        nth = 0;
    }

    void initialize(NDArray<SizeT>* ndarray, void* indices) {
        // NOTE: `RawNDIter`'s `element` should point to the first element of the ndarray view,
        // which is at `data + offset`.
        void* first_element = static_cast<void*>(ndarray->data->template data<uint8_t>() + ndarray->offset);
        this->initialize(ndarray, first_element, indices);
    }

    // Is the current iteration valid?
    // If true, then `element`, `indices` and `nth` contain details about the current element.
    bool has_element() { return nth < size; }

    // Go to the next element.
    void next() {
        for (auto i = 0; i < array->ndims; i++) {
            auto axis = array->ndims - i - 1;
            indices->data()[axis]++;
            if (indices->data()[axis] >= array->shape->data()[axis]) {
                indices->data()[axis] = 0;

                // TODO: There is something called backstrides to speedup iteration.
                // See https://ajcr.net/stride-guide-part-1/, and
                // https://docs.scipy.org/doc/numpy-1.13.0/reference/c-api.types-and-structures.html#c.PyArrayIterObject.PyArrayIterObject.backstrides.
                offset -= array->strides->data()[axis] * (array->shape->data()[axis] - 1);
            } else {
                offset += array->strides->data()[axis];
                break;
            }
        }
        nth++;
    }
};
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;

void __nac3_nditer_initialize(RawNDIter<uint32_t>* iter, NDArray<uint32_t>* ndarray, void* indices) {
    iter->initialize(ndarray, indices);
}

void __nac3_nditer_initialize64(RawNDIter<uint64_t>* iter, NDArray<uint64_t>* ndarray, void* indices) {
    iter->initialize(ndarray, indices);
}

bool __nac3_nditer_has_element(RawNDIter<uint32_t>* iter) {
    return iter->has_element();
}

bool __nac3_nditer_has_element64(RawNDIter<uint64_t>* iter) {
    return iter->has_element();
}

void __nac3_nditer_next(RawNDIter<uint32_t>* iter) {
    iter->next();
}

void __nac3_nditer_next64(RawNDIter<uint64_t>* iter) {
    iter->next();
}
}
