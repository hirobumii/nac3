#pragma once

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/int_types.hpp"
#include "irrt/ndarray/basic.hpp"
#include "irrt/ndarray/broadcast.hpp"
#include "irrt/ndarray/iter.hpp"

// NOTE: Everything would be much easier and elegant if einsum is implemented.

namespace {
namespace ndarray {
namespace matmul {

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
template<typename SizeT>
void calculate_shapes(SizeT a_ndims,
                      SizeT* a_shape,
                      SizeT b_ndims,
                      SizeT* b_shape,
                      SizeT final_ndims,
                      SizeT* new_a_shape,
                      SizeT* new_b_shape,
                      SizeT* dst_shape) {
    debug_assert(SizeT, a_ndims >= 2);
    debug_assert(SizeT, b_ndims >= 2);
    debug_assert_eq(SizeT, max(a_ndims, b_ndims), final_ndims);

    // Check that a and b are compatible for matmul
    if (a_shape[a_ndims - 1] != b_shape[b_ndims - 2]) {
        // This is a custom error message. Different from NumPy.
        raise_exception(SizeT, EXN_VALUE_ERROR, "Cannot multiply LHS (shape ?x{0}) with RHS (shape {1}x?})",
                        a_shape[a_ndims - 1], b_shape[b_ndims - 2], NO_PARAM);
    }

    const SizeT num_entries = 2;
    ShapeEntry<SizeT> entries[num_entries] = {{.ndims = a_ndims - 2, .shape = a_shape},
                                              {.ndims = b_ndims - 2, .shape = b_shape}};

    // TODO: Optimize this
    ndarray::broadcast::broadcast_shapes<SizeT>(num_entries, entries, final_ndims - 2, new_a_shape);
    ndarray::broadcast::broadcast_shapes<SizeT>(num_entries, entries, final_ndims - 2, new_b_shape);
    ndarray::broadcast::broadcast_shapes<SizeT>(num_entries, entries, final_ndims - 2, dst_shape);

    new_a_shape[final_ndims - 2] = a_shape[a_ndims - 2];
    new_a_shape[final_ndims - 1] = a_shape[a_ndims - 1];
    new_b_shape[final_ndims - 2] = b_shape[b_ndims - 2];
    new_b_shape[final_ndims - 1] = b_shape[b_ndims - 1];
    dst_shape[final_ndims - 2] = a_shape[a_ndims - 2];
    dst_shape[final_ndims - 1] = b_shape[b_ndims - 1];
}
}  // namespace matmul
}  // namespace ndarray
}  // namespace

extern "C" {
using namespace ndarray::matmul;

void __nac3_ndarray_matmul_calculate_shapes(int32_t a_ndims,
                                            int32_t* a_shape,
                                            int32_t b_ndims,
                                            int32_t* b_shape,
                                            int32_t final_ndims,
                                            int32_t* new_a_shape,
                                            int32_t* new_b_shape,
                                            int32_t* dst_shape) {
    calculate_shapes(a_ndims, a_shape, b_ndims, b_shape, final_ndims, new_a_shape, new_b_shape, dst_shape);
}

void __nac3_ndarray_matmul_calculate_shapes64(int64_t a_ndims,
                                              int64_t* a_shape,
                                              int64_t b_ndims,
                                              int64_t* b_shape,
                                              int64_t final_ndims,
                                              int64_t* new_a_shape,
                                              int64_t* new_b_shape,
                                              int64_t* dst_shape) {
    calculate_shapes(a_ndims, a_shape, b_ndims, b_shape, final_ndims, new_a_shape, new_b_shape, dst_shape);
}
}