#pragma once

#include "irrt/exception.hpp"
#include "irrt/ndarray/def.hpp"

namespace __nac3_impl {
namespace {
namespace ndarray::reshape {
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
void resolve_and_check_new_shape(intp_t size, intp_t new_ndims, intp_t* new_shape) {
    // Is there a -1 in `new_shape`?
    bool neg1_exists = false;
    // Location of -1, only initialized if `neg1_exists` is true
    intp_t neg1_axis_i;
    // The computed ndarray size of `new_shape`
    intp_t new_size = 1;

    for (intp_t axis_i = 0; axis_i < new_ndims; axis_i++) {
        intp_t dim = new_shape[axis_i];
        if (dim < 0) {
            if (dim == -1) {
                if (neg1_exists) {
                    // Multiple `-1` found. Throw an error.
                    raise_exception(EXN_VALUE_ERROR, "can only specify one unknown dimension", NO_PARAM, NO_PARAM,
                                    NO_PARAM);
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

                raise_exception(EXN_VALUE_ERROR, "Found non -1 negative dimension {0} on axis {1}", dim, axis_i,
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
        raise_exception(EXN_VALUE_ERROR, "cannot reshape array of size {0} into given shape", size, NO_PARAM,
                        NO_PARAM);
    }
}
}  // namespace ndarray::reshape
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray;

void __nac3_ndarray_reshape_resolve_and_check_new_shape(intp_t size, intp_t new_ndims, intp_t* new_shape) {
    reshape::resolve_and_check_new_shape(size, new_ndims, new_shape);
}
}
