#pragma once

#include <irrt/int_types.hpp>
#include <irrt/ndarray/def.hpp>

namespace
{
/**
 * @brief Helper struct to enumerate through an ndarray *efficiently*.
 *
 * Interesting cases:
 * - If ndims == 0, there is one iteration.
 * - If shape contains zeroes, there are no iterations.
 */
template <typename SizeT> struct NDIter
{
    // Information about the ndarray being iterated over.
    SizeT ndims;
    SizeT *shape;
    SizeT *strides;

    /**
     * @brief The current indices.
     *
     * Must be allocated by the caller.
     */
    SizeT *indices;

    /**
     * @brief The nth (0-based) index of the current indices.
     *
     * Initially this is all 0s.
     */
    SizeT nth;

    /**
     * @brief Pointer to the current element.
     *
     * Initially this points to first element of the ndarray.
     */
    uint8_t *element;

    /**
     * @brief Cache for the product of shape.
     *
     * Could be 0 if `shape` has 0s in it.
     */
    SizeT size;

    // TODO:: Not implemented: There is something called backstrides to speedup iteration.
    // See https://ajcr.net/stride-guide-part-1/, and https://docs.scipy.org/doc/numpy-1.13.0/reference/c-api.types-and-structures.html#c.PyArrayIterObject.PyArrayIterObject.backstrides.

    void initialize(SizeT ndims, SizeT *shape, SizeT *strides, uint8_t *element, SizeT *indices)
    {
        this->ndims = ndims;
        this->shape = shape;
        this->strides = strides;

        this->indices = indices;
        this->element = element;

        // Compute size
        this->size = 1;
        for (SizeT i = 0; i < ndims; i++)
        {
            this->size *= shape[i];
        }

        for (SizeT axis = 0; axis < ndims; axis++)
            indices[axis] = 0;
        nth = 0;
    }

    void initialize_by_ndarray(NDArray<SizeT> *ndarray, SizeT *indices)
    {
        this->initialize(ndarray->ndims, ndarray->shape, ndarray->strides, ndarray->data, indices);
    }

    bool has_next()
    {
        return nth < size;
    }

    void next()
    {
        for (SizeT i = 0; i < ndims; i++)
        {
            SizeT axis = ndims - i - 1;
            indices[axis]++;
            if (indices[axis] >= shape[axis])
            {
                indices[axis] = 0;

                // TODO: Can be optimized with backstrides.
                element -= strides[axis] * (shape[axis] - 1);
            }
            else
            {
                element += strides[axis];
                break;
            }
        }
        nth++;
    }
};
} // namespace

extern "C"
{
    void __nac3_nditer_initialize(NDIter<int32_t> *iter, NDArray<int32_t> *ndarray, int32_t *indices)
    {
        iter->initialize_by_ndarray(ndarray, indices);
    }

    void __nac3_nditer_initialize64(NDIter<int64_t> *iter, NDArray<int64_t> *ndarray, int64_t *indices)
    {
        iter->initialize_by_ndarray(ndarray, indices);
    }

    bool __nac3_nditer_has_next(NDIter<int32_t> *iter)
    {
        return iter->has_next();
    }

    bool __nac3_nditer_has_next64(NDIter<int64_t> *iter)
    {
        return iter->has_next();
    }

    void __nac3_nditer_next(NDIter<int32_t> *iter)
    {
        iter->next();
    }

    void __nac3_nditer_next64(NDIter<int64_t> *iter)
    {
        iter->next();
    }
}