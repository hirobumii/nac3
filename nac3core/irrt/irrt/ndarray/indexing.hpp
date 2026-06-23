#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/range.hpp"
#include "irrt/reference/reference.hpp"
#include "irrt/slice.hpp"

namespace __nac3_impl {
namespace {
namespace ndarray {
typedef uint8_t NDIndexType;

/**
 * @brief A single element index
 *
 * `data` points to a `int32_t`.
 */
const NDIndexType ND_INDEX_TYPE_SINGLE_ELEMENT = 0;

/**
 * @brief A slice index
 *
 * `data` points to a `__nac3_impl::Slice<int32_t>`.
 */
const NDIndexType ND_INDEX_TYPE_SLICE = 1;

/**
 * @brief `np.newaxis` / `None`
 *
 * `data` is unused.
 */
const NDIndexType ND_INDEX_TYPE_NEWAXIS = 2;

/**
 * @brief `Ellipsis` / `...`
 *
 * `data` is unused.
 */
const NDIndexType ND_INDEX_TYPE_ELLIPSIS = 3;

/**
 * @brief An index used in ndarray indexing
 *
 * That is:
 * ```
 * my_ndarray[::-1, 3, ..., np.newaxis]
 *            ^^^^  ^  ^^^  ^^^^^^^^^^ each of these is represented by an NDIndex.
 * ```
 */
struct NDIndex {
    /**
     * @brief Enum tag to specify the type of index.
     *
     * Please see the comment of each enum constant.
     */
    NDIndexType type;

    /**
     * @brief The accompanying data associated with `type`.
     *
     * Please see the comment of each enum constant.
     */
    void* data;
};

namespace indexing {
/**
 * @brief Perform ndarray "basic indexing" (https://numpy.org/doc/stable/user/basics.indexing.html#basic-indexing)
 *
 * This function is very similar to performing `dst_ndarray = src_ndarray[indices]` in Python.
 *
 * This function also does proper assertions on `indices` to check for out of bounds access and more.
 *
 * # Notes on `dst_ndarray`
 *
 * The caller is responsible for allocating space for the resulting ndarray.
 * Here is what this function expects from `dst_ndarray` when called:
 *   - `dst_ndarray->data` does not have to be initialized.
 *   - `dst_ndarray->itemsize` does not have to be initialized.
 *   - `dst_ndarray->ndims` must be initialized, and it must be equal to the expected `ndims` of the `dst_ndarray` after
 *       indexing `src_ndarray` with `indices`.
 *   - `dst_ndarray->shape` must be allocated, through it can contain uninitialized values.
 *   - `dst_ndarray->strides` must be allocated, through it can contain uninitialized values.
 *   - `dst_ndarray->base` does not have to be initialized.
 *   - `dst_ndarray->offset` does not have to be initialized.
 *
 * When this function call ends:
 *   - `dst_ndarray->data` is set to `src_ndarray->data`.
 *   - `dst_ndarray->itemsize` is set to `src_ndarray->itemsize`.
 *   - `dst_ndarray->ndims` is unchanged.
 *   - `dst_ndarray->shape` is updated according to how `src_ndarray` is indexed.
 *   - `dst_ndarray->strides` is updated accordingly by how ndarray indexing works.
 *   - `dst_ndarray->base` is set to `src_ndarray`.
 *   - `dst_ndarray->offset` is set to the offset of the first element
 *
 * @param indices indices to index `src_ndarray`, ordered in the same way you would write them in Python.
 * @param src_ndarray The NDArray to be indexed.
 * @param dst_ndarray The resulting NDArray after indexing. Further details in the comments above,
 */
void index(intp_t num_indices, const NDIndex* indices, NDArray* src_ndarray, NDArray* dst_ndarray) {
    // Validate `indices`.

    // Expected value of `dst_ndarray->ndims`.
    auto expected_dst_ndims = src_ndarray->ndims;
    // To check for "too many indices for array: array is ?-dimensional, but ? were indexed"
    intp_t num_indexed = 0;
    // There may be ellipsis `...` in `indices`. There can only be 0 or 1 ellipsis.
    intp_t num_ellipsis = 0;

    for (auto i = 0; i < num_indices; i++) {
        if (indices[i].type == ND_INDEX_TYPE_SINGLE_ELEMENT) {
            expected_dst_ndims--;
            num_indexed++;
        } else if (indices[i].type == ND_INDEX_TYPE_SLICE) {
            num_indexed++;
        } else if (indices[i].type == ND_INDEX_TYPE_NEWAXIS) {
            expected_dst_ndims++;
        } else if (indices[i].type == ND_INDEX_TYPE_ELLIPSIS) {
            num_ellipsis++;
            if (num_ellipsis > 1) {
                raise_exception(EXN_INDEX_ERROR, "an index can only have a single ellipsis ('...')", NO_PARAM, NO_PARAM,
                                NO_PARAM);
            }
        } else {
            __builtin_unreachable();
        }
    }

    debug_assert_eq(expected_dst_ndims, dst_ndarray->ndims);

    if (src_ndarray->ndims - num_indexed < 0) {
        raise_exception(EXN_INDEX_ERROR,
                        "too many indices for array: array is {0}-dimensional, "
                        "but {1} were indexed",
                        src_ndarray->ndims, num_indices, NO_PARAM);
    }

    dst_ndarray->base = src_ndarray;

    dst_ndarray->data = src_ndarray->data;
    dst_ndarray->offset = 0;
    dst_ndarray->itemsize = src_ndarray->itemsize;
    dst_ndarray->base = src_ndarray;
    dst_ndarray->offset = src_ndarray->offset;

    reference::refcount_incr(dst_ndarray->data);
    reference::refcount_incr(dst_ndarray->base);

    // Reference code:
    // https://github.com/wadetb/tinynumpy/blob/0d23d22e07062ffab2afa287374c7b366eebdda1/tinynumpy/tinynumpy.py#L652
    intp_t src_axis = 0;
    intp_t dst_axis = 0;

    for (int32_t i = 0; i < num_indices; i++) {
        const NDIndex* index = &indices[i];
        if (index->type == ND_INDEX_TYPE_SINGLE_ELEMENT) {
            auto input = static_cast<intp_t>(*((int32_t*)index->data));

            auto k = slice::resolve_index_in_length(src_ndarray->shape->data()[src_axis], input);
            if (k == -1) {
                raise_exception(EXN_INDEX_ERROR,
                                "index {0} is out of bounds for axis {1} "
                                "with size {2}",
                                input, src_axis, src_ndarray->shape->data()[src_axis]);
            }

            dst_ndarray->offset += k * src_ndarray->strides->data()[src_axis];

            src_axis++;
        } else if (index->type == ND_INDEX_TYPE_SLICE) {
            Slice<int32_t>* slice = (Slice<int32_t>*)index->data;

            Range<int32_t> range = slice->indices_checked(src_ndarray->shape->data()[src_axis]);

            dst_ndarray->offset += static_cast<intp_t>(range.start) * src_ndarray->strides->data()[src_axis];
            dst_ndarray->strides->data()[dst_axis] = static_cast<intp_t>(range.step) * src_ndarray->strides->data()[src_axis];
            dst_ndarray->shape->data()[dst_axis] = static_cast<intp_t>(range.len());

            dst_axis++;
            src_axis++;
        } else if (index->type == ND_INDEX_TYPE_NEWAXIS) {
            dst_ndarray->strides->data()[dst_axis] = 0;
            dst_ndarray->shape->data()[dst_axis] = 1;

            dst_axis++;
        } else if (index->type == ND_INDEX_TYPE_ELLIPSIS) {
            // The number of ':' entries this '...' implies.
            intp_t ellipsis_size = src_ndarray->ndims - num_indexed;

            for (intp_t j = 0; j < ellipsis_size; j++) {
                dst_ndarray->strides->data()[dst_axis] = src_ndarray->strides->data()[src_axis];
                dst_ndarray->shape->data()[dst_axis] = src_ndarray->shape->data()[src_axis];

                dst_axis++;
                src_axis++;
            }
        } else {
            __builtin_unreachable();
        }
    }

    for (; dst_axis < dst_ndarray->ndims; dst_axis++, src_axis++) {
        dst_ndarray->shape->data()[dst_axis] = src_ndarray->shape->data()[src_axis];
        dst_ndarray->strides->data()[dst_axis] = src_ndarray->strides->data()[src_axis];
    }

    debug_assert_eq(src_ndarray->ndims, src_axis);
    debug_assert_eq(dst_ndarray->ndims, dst_axis);
}
}  // namespace indexing
}  // namespace ndarray
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray;

void __nac3_ndarray_index(intp_t num_indices, NDIndex* indices, NDArray* src_ndarray, NDArray* dst_ndarray) {
    indexing::index(num_indices, indices, src_ndarray, dst_ndarray);
}
}
