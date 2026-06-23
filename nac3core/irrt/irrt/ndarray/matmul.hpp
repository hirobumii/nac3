#pragma once

#include "irrt/stdlib/algorithm.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/ndarray/broadcast.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/reference/array.hpp"

// NOTE: Everything would be much easier and elegant if einsum is implemented.

namespace __nac3_impl {
namespace {
namespace ndarray::matmul {

/**
 * @brief Perform the broadcast in `np.einsum("...ij,...jk->...ik", a, b)`.
 *
 * Example:
 *   Suppose      `a_shape ==      [1, 97, 4, 2]`
 *       and      `b_shape == [99, 98,  1, 2, 5]`,
 *
 *   ...then  `new_a_shape == [99, 98, 97, 4, 2]`,
 *            `new_b_shape == [99, 98, 97, 2, 5]`,
 *      and     `dst_shape == [99, 98, 97, 4, 5]`.
 *                             ^^^^^^^^^^  ^^^^
 *                          (broadcasted)  (4x2 @ 2x5 => 4x5)
 *
 * @param a_ndims Length of `a_shape`.
 * @param a_shape Shape of `a`.
 * @param b_ndims Length of `b_shape`.
 * @param b_shape Shape of `b`.
 * @param final_ndims Should be equal to `max(a_ndims, b_ndims)`. This is the length of `new_a_shape`,
 * `new_b_shape`, and `dst_shape` - the number of dimensions after broadcasting.
 */
void calculate_shapes(intp_t a_ndims,
                      reference::Array<intp_t>* a_shape,
                      intp_t b_ndims,
                      reference::Array<intp_t>* b_shape,
                      intp_t final_ndims,
                      reference::Array<intp_t>* new_a_shape,
                      reference::Array<intp_t>* new_b_shape,
                      reference::Array<intp_t>* dst_shape) {
    debug_assert(a_ndims >= 2);
    debug_assert(b_ndims >= 2);
    debug_assert_eq(stdlib::max(a_ndims, b_ndims), final_ndims);

    // Check that a and b are compatible for matmul
    if (a_shape->data()[a_ndims - 1] != b_shape->data()[b_ndims - 2]) {
        // This is a custom error message. Different from NumPy.
        raise_exception(EXN_VALUE_ERROR, "Cannot multiply LHS (shape ?x{0}) with RHS (shape {1}x?})",
                        a_shape->data()[a_ndims - 1], b_shape->data()[b_ndims - 2], NO_PARAM);
    }

    constexpr intp_t num_entries = 2;
    ShapeEntry entries[num_entries] = {{.ndims = a_ndims - 2, .shape = a_shape},
                                       {.ndims = b_ndims - 2, .shape = b_shape}};

    // TODO: Optimize this
    broadcast::broadcast_shapes(num_entries, entries, final_ndims - 2, new_a_shape);
    broadcast::broadcast_shapes(num_entries, entries, final_ndims - 2, new_b_shape);
    broadcast::broadcast_shapes(num_entries, entries, final_ndims - 2, dst_shape);

    new_a_shape->data()[final_ndims - 2] = a_shape->data()[a_ndims - 2];
    new_a_shape->data()[final_ndims - 1] = a_shape->data()[a_ndims - 1];
    new_b_shape->data()[final_ndims - 2] = b_shape->data()[b_ndims - 2];
    new_b_shape->data()[final_ndims - 1] = b_shape->data()[b_ndims - 1];
    dst_shape->data()[final_ndims - 2] = a_shape->data()[a_ndims - 2];
    dst_shape->data()[final_ndims - 1] = b_shape->data()[b_ndims - 1];
}
}  // namespace ndarray::matmul
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray;

void __nac3_ndarray_matmul_calculate_shapes(intp_t a_ndims,
                                            reference::Array<intp_t>* a_shape,
                                            intp_t b_ndims,
                                            reference::Array<intp_t>* b_shape,
                                            intp_t final_ndims,
                                            reference::Array<intp_t>* new_a_shape,
                                            reference::Array<intp_t>* new_b_shape,
                                            reference::Array<intp_t>* dst_shape) {
    matmul::calculate_shapes(a_ndims, a_shape, b_ndims, b_shape, final_ndims, new_a_shape, new_b_shape, dst_shape);
}
}
