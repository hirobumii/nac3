#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/list.hpp"
#include "irrt/ndarray/basic.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/reference/array.hpp"

namespace {
namespace ndarray::array {
/**
 * @brief In the context of `np.array(<list>)`, deduce the ndarray's shape produced by `<list>` and raise
 * an exception if there is anything wrong with `<shape>` (e.g., inconsistent dimensions `np.array([[1.0, 2.0],
 * [3.0]])`)
 *
 * If this function finds no issues with `<list>`, the deduced shape is written to `shape`. The caller has the
 * responsibility to allocate `[SizeT; ndims]` for `shape`. The caller must also initialize `shape` with `-1`s because
 * of implementation details.
 */
template<typename SizeT>
void set_and_validate_list_shape_helper(__nac3_impl::stdlib::make_signed_t<SizeT> axis,
                                        List<SizeT>* list,
                                        __nac3_impl::stdlib::make_signed_t<SizeT> ndims,
                                        __nac3_impl::stdlib::make_signed_t<SizeT>* shape) {
    if (shape[axis] == -1) {
        // Dimension is unspecified. Set it.
        shape[axis] = list->len;
    } else {
        // Dimension is specified. Check.
        if (shape[axis] != static_cast<__nac3_impl::stdlib::make_signed_t<SizeT>>(list->len)) {
            // Mismatch, throw an error.
            // NOTE: NumPy's error message is more complex and needs more PARAMS to display.
            raise_exception(SizeT, EXN_VALUE_ERROR,
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
        auto** lists = list->items->template data<List<SizeT>*>();
        for (SizeT i = 0; i < list->len; i++) {
            set_and_validate_list_shape_helper<SizeT>(axis + 1, lists[i], ndims, shape);
        }
    }
}

/**
 * @brief See `set_and_validate_list_shape_helper`.
 */
template<typename SizeT>
void set_and_validate_list_shape(List<SizeT>* list,
                                 __nac3_impl::stdlib::make_signed_t<SizeT> ndims,
                                 __nac3_impl::stdlib::make_signed_t<SizeT>* shape) {
    for (auto axis = 0; axis < ndims; axis++) {
        shape[axis] = -1;  // Sentinel to say this dimension is unspecified.
    }
    set_and_validate_list_shape_helper<SizeT>(0, list, ndims, shape);
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
template<typename SizeT>
void write_list_to_array_helper(__nac3_impl::stdlib::make_signed_t<SizeT> axis,
                                SizeT* index,
                                List<SizeT>* list,
                                NDArray<SizeT>* ndarray) {
    debug_assert_eq(SizeT, static_cast<__nac3_impl::stdlib::make_signed_t<SizeT>>(list->len),
                    ndarray->shape->data()[axis]);
    if (IRRT_DEBUG_ASSERT_BOOL) {
        if (!ndarray::basic::is_c_contiguous(ndarray)) {
            raise_debug_assert(SizeT, "ndarray is not C-contiguous", ndarray->strides->data()[0],
                               ndarray->strides->data()[1], NO_PARAM);
        }
    }

    if (axis + 1 == ndarray->ndims) {
        // `list` has type `list[scalar]`
        // `ndarray` is contiguous, so we can do this, and this is fast.
        auto* dst = ndarray->data->template data<uint8_t>() + ndarray->offset + (ndarray->itemsize * (*index));
        __builtin_memcpy(dst, reinterpret_cast<__nac3_impl::reference::Array<SizeT>*>(list->items)->data(),
                         ndarray->itemsize * list->len);
        *index += list->len;
    } else {
        // `list` has type `list[list[...]]`
        auto** lists = list->items->template data<List<SizeT>*>();

        for (SizeT i = 0; i < list->len; i++) {
            write_list_to_array_helper<SizeT>(axis + 1, index, lists[i], ndarray);
        }
    }
}

/**
 * @brief See `write_list_to_array_helper`.
 */
template<typename SizeT>
void write_list_to_array(List<SizeT>* list, NDArray<SizeT>* ndarray) {
    SizeT index = 0;
    write_list_to_array_helper<SizeT>((SizeT)0, &index, list, ndarray);
}
}  // namespace ndarray::array
}  // namespace

extern "C" {
using namespace ndarray::array;

void __nac3_ndarray_array_set_and_validate_list_shape(List<uint32_t>* list, int32_t ndims, int32_t* shape) {
    set_and_validate_list_shape(list, ndims, shape);
}

void __nac3_ndarray_array_set_and_validate_list_shape64(List<uint64_t>* list, int64_t ndims, int64_t* shape) {
    set_and_validate_list_shape(list, ndims, shape);
}

void __nac3_ndarray_array_write_list_to_array(List<uint32_t>* list, NDArray<uint32_t>* ndarray) {
    write_list_to_array(list, ndarray);
}

void __nac3_ndarray_array_write_list_to_array64(List<uint64_t>* list, NDArray<uint64_t>* ndarray) {
    write_list_to_array(list, ndarray);
}
}
