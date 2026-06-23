#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/list.hpp"
#include "irrt/ndarray/basic.hpp"
#include "irrt/ndarray/def.hpp"

namespace __nac3_impl {
namespace {
namespace ndarray::array {
/**
 * @brief In the context of `np.array(<list>)`, deduce the ndarray's shape produced by `<list>` and raise
 * an exception if there is anything wrong with `<shape>` (e.g., inconsistent dimensions `np.array([[1.0, 2.0],
 * [3.0]])`)
 *
 * If this function finds no issues with `<list>`, the deduced shape is written to `shape`. The caller has the
 * responsibility to allocate `[intp_t; ndims]` for `shape`. The caller must also initialize `shape` with `-1`s because
 * of implementation details.
 */
void set_and_validate_list_shape_helper(intp_t axis, List* list, intp_t ndims, intp_t* shape) {
    if (shape[axis] == -1) {
        // Dimension is unspecified. Set it.
        shape[axis] = list->len;
    } else {
        // Dimension is specified. Check.
        if (shape[axis] != static_cast<intp_t>(list->len)) {
            // Mismatch, throw an error.
            // NOTE: NumPy's error message is more complex and needs more PARAMS to display.
            raise_exception(EXN_VALUE_ERROR,
                            "The requested array has an inhomogenous shape "
                            "after {0} dimension(s).",
                            axis, shape[axis], list->len);
        }
    }

    if (axis + 1 == ndims) {
        // `list` has type `list[ItemType]`
        // Do nothing
    } else {
        // `list` has type `list[list[...]]`
        auto** lists = list->items->template data<List*>();
        for (size_t i = 0; i < list->len; i++) {
            set_and_validate_list_shape_helper(axis + 1, lists[i], ndims, shape);
        }
    }
}

/**
 * @brief See `set_and_validate_list_shape_helper`.
 */
void set_and_validate_list_shape(List* list, intp_t ndims, intp_t* shape) {
    for (intp_t axis = 0; axis < ndims; axis++) {
        shape[axis] = -1;  // Sentinel to say this dimension is unspecified.
    }
    set_and_validate_list_shape_helper(0, list, ndims, shape);
}

/**
 * @brief In the context of `np.array(<list>)`, copied the contents stored in `list` to `ndarray`.
 *
 * `list` is assumed to be "legal". (i.e., no inconsistent dimensions)
 *
 * # Notes on `ndarray`
 * The caller is responsible for allocating space for `ndarray`.
 * Here is what this function expects from `ndarray` when called:
 *   - `ndarray->data` has to be allocated, contiguous, and may contain uninitialized values.
 *   - `ndarray->itemsize` has to be initialized.
 *   - `ndarray->ndims` has to be initialized.
 *   - `ndarray->shape` has to be initialized.
 *   - `ndarray->strides` is ignored, but note that `ndarray->data` is contiguous.
 * When this function call ends:
 *   - `ndarray->data` is written with contents from `<list>`.
 */
void write_list_to_array_helper(intp_t axis, intp_t* index, List* list, NDArray* ndarray) {
    debug_assert_eq(static_cast<intp_t>(list->len), ndarray->shape->data()[axis]);
    if (IRRT_DEBUG_ASSERT_BOOL) {
        if (!basic::is_c_contiguous(ndarray)) {
            raise_debug_assert("ndarray is not C-contiguous", ndarray->strides->data()[0], ndarray->strides->data()[1],
                               NO_PARAM);
        }
    }

    if (axis + 1 == ndarray->ndims) {
        // `list` has type `list[scalar]`
        // `ndarray` is contiguous, so we can do this, and this is fast.
        auto* dst = ndarray->data->template data<uint8_t>() + ndarray->offset + (ndarray->itemsize * (*index));
        __builtin_memcpy(dst, list->items->data(), ndarray->itemsize * list->len);
        *index += list->len;
    } else {
        // `list` has type `list[list[...]]`
        auto** lists = list->items->template data<List*>();

        for (size_t i = 0; i < list->len; i++) {
            write_list_to_array_helper(axis + 1, index, lists[i], ndarray);
        }
    }
}

/**
 * @brief See `write_list_to_array_helper`.
 */
void write_list_to_array(List* list, NDArray* ndarray) {
    intp_t index = 0;
    write_list_to_array_helper(0, &index, list, ndarray);
}
}  // namespace ndarray::array
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray;

void __nac3_ndarray_array_set_and_validate_list_shape(List* list, intp_t ndims, intp_t* shape) {
    array::set_and_validate_list_shape(list, ndims, shape);
}

void __nac3_ndarray_array_write_list_to_array(List* list, NDArray* ndarray) {
    array::write_list_to_array(list, ndarray);
}
}
