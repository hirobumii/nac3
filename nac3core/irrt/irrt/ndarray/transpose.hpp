#pragma once

#include "irrt/stdlib/cstdint.h"
#include "irrt/stdlib/type_traits.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/ndarray/def.hpp"
#include "irrt/slice.hpp"

/*
 * Notes on `np.transpose(<array>, <axes>)`
 *
 * TODO: `axes`, if specified, can actually contain negative indices,
 * but it is not documented in numpy.
 *
 * Supporting it for now.
 */

namespace __nac3_impl {
namespace {
namespace ndarray::transpose {
/**
 * @brief Do assertions on `<axes>` in `np.transpose(<array>, <axes>)`.
 *
 * Note that `np.transpose`'s `<axe>` argument is optional. If the argument
 * is specified but the user, use this function to do assertions on it.
 *
 * @param ndims The number of dimensions of `<array>`
 * @param num_axes Number of elements in `<axes>` as specified by the user.
 *   This should be equal to `ndims`. If not, a "ValueError: axes don't match array" is thrown.
 * @param axes The user specified `<axes>`.
 */
template<typename SizeT>
void assert_transpose_axes(SizeT ndims, SizeT num_axes, const SizeT* axes) {
    if (ndims != num_axes) {
        raise_exception(SizeT, EXN_VALUE_ERROR, "axes don't match array", NO_PARAM, NO_PARAM, NO_PARAM);
    }

    // TODO: Optimize this
    bool* axe_specified = (bool*)__builtin_alloca(sizeof(bool) * ndims);
    for (SizeT i = 0; i < ndims; i++)
        axe_specified[i] = false;

    for (SizeT i = 0; i < ndims; i++) {
        SizeT axis = __nac3_impl::slice::resolve_index_in_length(ndims, axes[i]);
        if (axis == -1) {
            // TODO: numpy actually throws a `numpy.exceptions.AxisError`
            raise_exception(SizeT, EXN_VALUE_ERROR, "axis {0} is out of bounds for array of dimension {1}", axis, ndims,
                            NO_PARAM);
        }

        if (axe_specified[axis]) {
            raise_exception(SizeT, EXN_VALUE_ERROR, "repeated axis in transpose", NO_PARAM, NO_PARAM, NO_PARAM);
        }

        axe_specified[axis] = true;
    }
}

/**
 * @brief Create a transpose view of `src_ndarray` and perform proper assertions.
 *
 * This function is very similar to doing `dst_ndarray = np.transpose(src_ndarray, <axes>)`.
 * If `<axes>` is supposed to be `None`, caller can pass in a `nullptr` to `<axes>`.
 *
 * The transpose view created is returned by modifying `dst_ndarray`.
 *
 * The caller is responsible for setting up `dst_ndarray` before calling this function.
 * Here is what this function expects from `dst_ndarray` when called:
 *   - `dst_ndarray->data` does not have to be initialized.
 *   - `dst_ndarray->itemsize` does not have to be initialized.
 *   - `dst_ndarray->ndims` must be initialized, must be equal to `src_ndarray->ndims`.
 *   - `dst_ndarray->shape` must be allocated, through it can contain uninitialized values.
 *   - `dst_ndarray->strides` must be allocated, through it can contain uninitialized values.
 * When this function call ends:
 *   - `dst_ndarray->data` is set to `src_ndarray->data` (`dst_ndarray` is just a view to `src_ndarray`)
 *   - `dst_ndarray->itemsize` is set to `src_ndarray->itemsize`
 *   - `dst_ndarray->ndims` is unchanged
 *   - `dst_ndarray->shape` is updated according to how `np.transpose` works
 *   - `dst_ndarray->strides` is updated according to how `np.transpose` works
 *
 * @param src_ndarray The NDArray to build a transpose view on
 * @param dst_ndarray The resulting NDArray after transpose. Further details in the comments above,
 * @param num_axes Number of elements in axes. Unused if `axes` is nullptr.
 * @param axes Axes permutation. Set it to `nullptr` if `<axes>` is `None`.
 */
template<typename SizeT>
void transpose(NDArray<SizeT>* src_ndarray,
               NDArray<SizeT>* dst_ndarray,
               __nac3_impl::stdlib::make_signed_t<SizeT> num_axes,
               const __nac3_impl::stdlib::make_signed_t<SizeT>* axes) {
    debug_assert_eq(SizeT, src_ndarray->ndims, dst_ndarray->ndims);
    const auto ndims = src_ndarray->ndims;

    if (axes != nullptr)
        assert_transpose_axes(ndims, num_axes, axes);

    dst_ndarray->data = src_ndarray->data;
    dst_ndarray->itemsize = src_ndarray->itemsize;
    dst_ndarray->base = src_ndarray;
    dst_ndarray->offset = src_ndarray->offset;

    // Check out https://ajcr.net/stride-guide-part-2/ to see how `np.transpose` works behind the scenes.
    if (axes == nullptr) {
        // `np.transpose(<array>, axes=None)`

        /*
         * Minor note: `np.transpose(<array>, axes=None)` is equivalent to
         * `np.transpose(<array>, axes=[N-1, N-2, ..., 0])` - basically it
         * is reversing the order of strides and shape.
         *
         * This is a fast implementation to handle this special (but very common) case.
         */

        for (auto axis = 0; axis < ndims; axis++) {
            dst_ndarray->shape->data()[axis] = src_ndarray->shape->data()[ndims - axis - 1];
            dst_ndarray->strides->data()[axis] = src_ndarray->strides->data()[ndims - axis - 1];
        }
    } else {
        // `np.transpose(<array>, <axes>)`

        // Permute strides and shape according to `axes`, while resolving negative indices in `axes`
        for (auto axis = 0; axis < ndims; axis++) {
            // `i` cannot be OUT_OF_BOUNDS because of assertions
            SizeT i = __nac3_impl::slice::resolve_index_in_length(ndims, axes[axis]);

            dst_ndarray->shape->data()[axis] = src_ndarray->shape->data()[i];
            dst_ndarray->strides->data()[axis] = src_ndarray->strides->data()[i];
        }
    }
}
}  // namespace ndarray::transpose
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ndarray::transpose;
void __nac3_ndarray_transpose(NDArray<uint32_t>* src_ndarray,
                              NDArray<uint32_t>* dst_ndarray,
                              int32_t num_axes,
                              const int32_t* axes) {
    transpose(src_ndarray, dst_ndarray, num_axes, axes);
}

void __nac3_ndarray_transpose64(NDArray<uint64_t>* src_ndarray,
                                NDArray<uint64_t>* dst_ndarray,
                                int64_t num_axes,
                                const int64_t* axes) {
    transpose(src_ndarray, dst_ndarray, num_axes, axes);
}
}
