#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/debug.hpp"

namespace __nac3_impl {
namespace {
/**
 * @brief The type of an index or a value describing the length of a range/slice.
 *
 * @note `SliceIndex` is defined here rather than in `slice.hpp` (where it would more naturally belong) because
 * `slice.hpp` already depends on `range.hpp` - Defining it in `slice.hpp` would force `range.hpp` to include
 * `slice.hpp` and create a circular dependency.
 */
using SliceIndex = int32_t;

namespace range {
template<typename T>
T len(T start, T stop, T step) {
    // Reference:
    // https://github.com/python/cpython/blob/9dbd12375561a393eaec4b21ee4ac568a407cdb0/Objects/rangeobject.c#L933
    if (step > 0 && start < stop)
        return 1 + (stop - 1 - start) / step;
    else if (step < 0 && start > stop)
        return 1 + (start - 1 - stop) / (-step);
    else
        return 0;
}
}  // namespace range

/**
 * @brief A Python range.
 */
template<typename T>
struct Range {
    T start;
    T stop;
    T step;

    /**
     * @brief Calculate the `len()` of this range.
     */
    T len() {
        debug_assert(step != 0);
        return range::len(start, stop, step);
    }
};
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::range;

SliceIndex __nac3_range_slice_len(const SliceIndex start, const SliceIndex end, const SliceIndex step) {
    return len(start, end, step);
}
}
