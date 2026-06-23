#pragma once

#include "irrt/stdlib/algorithm.h"

#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/range.hpp"

namespace __nac3_impl {
namespace {
namespace slice {
/**
 * @brief Resolve a possibly negative index in a list of a known length.
 *
 * Returns -1 if the resolved index is out of the list's bounds.
 */
template<typename T>
T resolve_index_in_length(T length, T index) {
    T resolved = index < 0 ? length + index : index;
    if (0 <= resolved && resolved < length) {
        return resolved;
    } else {
        return -1;
    }
}

/**
 * @brief Resolve a slice as a range.
 *
 * This is equivalent to `range(*slice(start, stop, step).indices(length))` in Python.
 */
template<typename T>
void indices(bool start_defined,
             T start,
             bool stop_defined,
             T stop,
             bool step_defined,
             T step,
             T length,
             T* range_start,
             T* range_stop,
             T* range_step) {
    // Reference: https://github.com/python/cpython/blob/main/Objects/sliceobject.c#L388
    *range_step = step_defined ? step : 1;
    bool step_is_negative = *range_step < 0;

    T lower, upper;
    if (step_is_negative) {
        lower = -1;
        upper = length - 1;
    } else {
        lower = 0;
        upper = length;
    }

    if (start_defined) {
        *range_start =
            start < 0 ? __nac3_impl::stdlib::max(lower, start + length) : __nac3_impl::stdlib::min(upper, start);
    } else {
        *range_start = step_is_negative ? upper : lower;
    }

    if (stop_defined) {
        *range_stop = stop < 0 ? __nac3_impl::stdlib::max(lower, stop + length) : __nac3_impl::stdlib::min(upper, stop);
    } else {
        *range_stop = step_is_negative ? lower : upper;
    }
}
}  // namespace slice

/**
 * @brief A Python-like slice with **unresolved** indices.
 */
template<typename T>
struct Slice {
    bool start_defined;
    T start;

    bool stop_defined;
    T stop;

    bool step_defined;
    T step;

    Slice() { this->reset(); }

    void reset() {
        this->start_defined = false;
        this->stop_defined = false;
        this->step_defined = false;
    }

    void set_start(T start) {
        this->start_defined = true;
        this->start = start;
    }

    void set_stop(T stop) {
        this->stop_defined = true;
        this->stop = stop;
    }

    void set_step(T step) {
        this->step_defined = true;
        this->step = step;
    }

    /**
     * @brief Resolve this slice as a range.
     *
     * In Python, this would be `range(*slice(start, stop, step).indices(length))`.
     */
    Range<T> indices(T length) {
        // Reference:
        // https://github.com/python/cpython/blob/main/Objects/sliceobject.c#L388
        debug_assert(length >= 0);

        Range<T> result;
        slice::indices(start_defined, start, stop_defined, stop, step_defined, step, length, &result.start,
                       &result.stop, &result.step);
        return result;
    }

    /**
     * @brief Like `.indices()` but with assertions.
     */
    Range<T> indices_checked(T length) {
        if (length < 0) {
            raise_exception(EXN_VALUE_ERROR, "length should not be negative, got {0}", length, NO_PARAM, NO_PARAM);
        }

        if (this->step_defined && this->step == 0) {
            raise_exception(EXN_VALUE_ERROR, "slice step cannot be zero", NO_PARAM, NO_PARAM, NO_PARAM);
        }

        return this->indices(length);
    }
};
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;

SliceIndex __nac3_slice_index_bound(SliceIndex i, const SliceIndex len) {
    if (i < 0) {
        i = len + i;
    }
    if (i < 0) {
        return 0;
    } else if (i > len) {
        return len;
    }
    return i;
}
}
