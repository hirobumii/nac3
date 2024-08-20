#pragma once

#include "irrt/exception.hpp"
#include "irrt/int_types.hpp"
#include "irrt/ndarray/def.hpp"

namespace {
namespace ndarray {
namespace reshape {
/**
 * @brief Perform assertions on and resolve unknown dimensions in `new_shape` in `np.reshape(<ndarray>, new_shape)`
 *
 * If `new_shape` indeed contains unknown dimensions (specified with `-1`, just like numpy), `new_shape` will be
 * modified to contain the resolved dimension.
 *
 * To perform assertions on and resolve unknown dimensions in `new_shape`, we don't need the actual
 * `<ndarray>` object itself, but only the `.size` of the `<ndarray>`.
 *
 * @param size The `.size` of `<ndarray>`
 * @param new_ndims Number of elements in `new_shape`
 * @param new_shape Target shape to reshape to
 */
template<typename SizeT>
void resolve_and_check_new_shape(SizeT size, SizeT new_ndims, SizeT* new_shape) {
    // Is there a -1 in `new_shape`?
    bool neg1_exists = false;
    // Location of -1, only initialized if `neg1_exists` is true
    SizeT neg1_axis_i;
    // The computed ndarray size of `new_shape`
    SizeT new_size = 1;

    for (SizeT axis_i = 0; axis_i < new_ndims; axis_i++) {
        SizeT dim = new_shape[axis_i];
        if (dim < 0) {
            if (dim == -1) {
                if (neg1_exists) {
                    // Multiple `-1` found. Throw an error.
                    raise_exception(SizeT, EXN_VALUE_ERROR, "can only specify one unknown dimension", NO_PARAM,
                                    NO_PARAM, NO_PARAM);
                } else {
                    neg1_exists = true;
                    neg1_axis_i = axis_i;
                }
            } else {
                // TODO: What? In `np.reshape` any negative dimensions is
                // treated like its `-1`.
                //
                // Try running `np.zeros((3, 4)).reshape((-999, 2))`
                //
                // It is not documented by numpy.
                // Throw an error for now...

                raise_exception(SizeT, EXN_VALUE_ERROR, "Found non -1 negative dimension {0} on axis {1}", dim, axis_i,
                                NO_PARAM);
            }
        } else {
            new_size *= dim;
        }
    }

    bool can_reshape;
    if (neg1_exists) {
        // Let `x` be the unknown dimension
        // Solve `x * <new_size> = <size>`
        if (new_size == 0 && size == 0) {
            // `x` has infinitely many solutions
            can_reshape = false;
        } else if (new_size == 0 && size != 0) {
            // `x` has no solutions
            can_reshape = false;
        } else if (size % new_size != 0) {
            // `x` has no integer solutions
            can_reshape = false;
        } else {
            can_reshape = true;
            new_shape[neg1_axis_i] = size / new_size;  // Resolve dimension
        }
    } else {
        can_reshape = (new_size == size);
    }

    if (!can_reshape) {
        raise_exception(SizeT, EXN_VALUE_ERROR, "cannot reshape array of size {0} into given shape", size, NO_PARAM,
                        NO_PARAM);
    }
}
}  // namespace reshape
}  // namespace ndarray
}  // namespace

extern "C" {
void __nac3_ndarray_reshape_resolve_and_check_new_shape(int32_t size, int32_t new_ndims, int32_t* new_shape) {
    ndarray::reshape::resolve_and_check_new_shape(size, new_ndims, new_shape);
}

void __nac3_ndarray_reshape_resolve_and_check_new_shape64(int64_t size, int64_t new_ndims, int64_t* new_shape) {
    ndarray::reshape::resolve_and_check_new_shape(size, new_ndims, new_shape);
}
}
