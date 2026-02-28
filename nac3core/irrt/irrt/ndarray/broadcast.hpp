#pragma once

#include "irrt/stdlib/cstdint.h"
#include "irrt/stdlib/algorithm.h"

#include "irrt/ndarray/def.hpp"
#include "irrt/slice.hpp"

namespace {
template<typename SizeT>
struct ShapeEntry {
    SizeT ndims;
    SizeT* shape;
};
}  // namespace

namespace {
namespace ndarray::broadcast {
/**
 * @brief Return true if `src_shape` can broadcast to `dst_shape`.
 *
 * See https://numpy.org/doc/stable/user/basics.broadcasting.html
 */
template<typename SizeT>
bool can_broadcast_shape_to(SizeT target_ndims, const SizeT* target_shape, SizeT src_ndims, const SizeT* src_shape) {
    if (src_ndims > target_ndims) {
        return false;
    }

    for (SizeT i = 0; i < src_ndims; i++) {
        SizeT target_dim = target_shape[target_ndims - i - 1];
        SizeT src_dim = src_shape[src_ndims - i - 1];
        if (!(src_dim == 1 || target_dim == src_dim)) {
            return false;
        }
    }
    return true;
}

/**
 * @brief Performs `np.broadcast_shapes(<shapes>)`
 *
 * @param num_shapes Number of entries in `shapes`
 * @param shapes The list of shape to do `np.broadcast_shapes` on.
 * @param dst_ndims The length of `dst_shape`.
 *   `dst_ndims` must be `max([shape.ndims for shape in shapes])`, but the caller has to calculate it/provide it.
 *   for this function since they should already know in order to allocate `dst_shape` in the first place.
 * @param dst_shape The resulting shape. Must be pre-allocated by the caller. This function calculate the result
 *   of `np.broadcast_shapes` and write it here.
 */
template<typename SizeT>
void broadcast_shapes(SizeT num_shapes, const ShapeEntry<SizeT>* shapes, SizeT dst_ndims, SizeT* dst_shape) {
    for (SizeT dst_axis = 0; dst_axis < dst_ndims; dst_axis++) {
        dst_shape[dst_axis] = 1;
    }

#ifdef IRRT_DEBUG_ASSERT
    SizeT max_ndims_found = 0;
#endif

    for (SizeT i = 0; i < num_shapes; i++) {
        ShapeEntry<SizeT> entry = shapes[i];

        // Check pre-condition: `dst_ndims` must be `max([shape.ndims for shape in shapes])`
        debug_assert(SizeT, entry.ndims <= dst_ndims);

#ifdef IRRT_DEBUG_ASSERT
        max_ndims_found = __nac3_impl::stdlib::max(max_ndims_found, entry.ndims);
#endif

        for (SizeT j = 0; j < entry.ndims; j++) {
            SizeT entry_axis = entry.ndims - j - 1;
            SizeT dst_axis = dst_ndims - j - 1;

            SizeT entry_dim = entry.shape[entry_axis];
            SizeT dst_dim = dst_shape[dst_axis];

            if (dst_dim == 1) {
                dst_shape[dst_axis] = entry_dim;
            } else if (entry_dim == 1 || entry_dim == dst_dim) {
                // Do nothing
            } else {
                raise_exception(SizeT, EXN_VALUE_ERROR,
                                "shape mismatch: objects cannot be broadcast "
                                "to a single shape.",
                                NO_PARAM, NO_PARAM, NO_PARAM);
            }
        }
    }

#ifdef IRRT_DEBUG_ASSERT
    // Check pre-condition: `dst_ndims` must be `max([shape.ndims for shape in shapes])`
    debug_assert_eq(SizeT, max_ndims_found, dst_ndims);
#endif
}

/**
 * @brief Perform `np.broadcast_to(<ndarray>, <target_shape>)` and appropriate assertions.
 *
 * This function attempts to broadcast `src_ndarray` to a new shape defined by `dst_ndarray.shape`,
 * and return the result by modifying `dst_ndarray`.
 *
 * # Notes on `dst_ndarray`
 * The caller is responsible for allocating space for the resulting ndarray.
 * Here is what this function expects from `dst_ndarray` when called:
 *   - `dst_ndarray->data` does not have to be initialized.
 *   - `dst_ndarray->itemsize` does not have to be initialized.
 *   - `dst_ndarray->ndims` must be initialized, determining the length of `dst_ndarray->shape`
 *   - `dst_ndarray->shape` must be allocated, and must contain the desired target broadcast shape.
 *   - `dst_ndarray->strides` must be allocated, through it can contain uninitialized values.
 * When this function call ends:
 *   - `dst_ndarray->data` is set to `src_ndarray->data` (`dst_ndarray` is just a view to `src_ndarray`)
 *   - `dst_ndarray->itemsize` is set to `src_ndarray->itemsize`
 *   - `dst_ndarray->ndims` is unchanged.
 *   - `dst_ndarray->shape` is unchanged.
 *   - `dst_ndarray->strides` is updated accordingly by how ndarray broadcast_to works.
 */
template<typename SizeT>
void broadcast_to(NDArray<SizeT>* src_ndarray, NDArray<SizeT>* dst_ndarray) {
    if (!ndarray::broadcast::can_broadcast_shape_to(dst_ndarray->ndims, dst_ndarray->shape, src_ndarray->ndims,
                                                    src_ndarray->shape)) {
        raise_exception(SizeT, EXN_VALUE_ERROR, "operands could not be broadcast together", NO_PARAM, NO_PARAM,
                        NO_PARAM);
    }

    dst_ndarray->data = src_ndarray->data;
    dst_ndarray->itemsize = src_ndarray->itemsize;
    dst_ndarray->base = src_ndarray->base;
    dst_ndarray->offset = src_ndarray->offset;

    for (SizeT i = 0; i < dst_ndarray->ndims; i++) {
        SizeT src_axis = src_ndarray->ndims - i - 1;
        SizeT dst_axis = dst_ndarray->ndims - i - 1;
        if (src_axis < 0 || (src_ndarray->shape[src_axis] == 1 && dst_ndarray->shape[dst_axis] != 1)) {
            // Freeze the steps in-place
            dst_ndarray->strides[dst_axis] = 0;
        } else {
            dst_ndarray->strides[dst_axis] = src_ndarray->strides[src_axis];
        }
    }
}
}  // namespace ndarray::broadcast
}  // namespace

extern "C" {
using namespace ndarray::broadcast;

void __nac3_ndarray_broadcast_to(NDArray<int32_t>* src_ndarray, NDArray<int32_t>* dst_ndarray) {
    broadcast_to(src_ndarray, dst_ndarray);
}

void __nac3_ndarray_broadcast_to64(NDArray<int64_t>* src_ndarray, NDArray<int64_t>* dst_ndarray) {
    broadcast_to(src_ndarray, dst_ndarray);
}

void __nac3_ndarray_broadcast_shapes(int32_t num_shapes,
                                     const ShapeEntry<int32_t>* shapes,
                                     int32_t dst_ndims,
                                     int32_t* dst_shape) {
    broadcast_shapes(num_shapes, shapes, dst_ndims, dst_shape);
}

void __nac3_ndarray_broadcast_shapes64(int64_t num_shapes,
                                       const ShapeEntry<int64_t>* shapes,
                                       int64_t dst_ndims,
                                       int64_t* dst_shape) {
    broadcast_shapes(num_shapes, shapes, dst_ndims, dst_shape);
}
}
