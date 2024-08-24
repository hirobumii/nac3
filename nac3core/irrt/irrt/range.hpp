#pragma once

#include "irrt/debug.hpp"
#include "irrt/int_types.hpp"

namespace {
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
    template<typename SizeT>
    T len() {
        debug_assert(SizeT, step != 0);
        return range::len(start, stop, step);
    }
};
}  // namespace

extern "C" {
using namespace range;

SliceIndex __nac3_range_slice_len(const SliceIndex start, const SliceIndex end, const SliceIndex step) {
    return len(start, end, step);
}
}