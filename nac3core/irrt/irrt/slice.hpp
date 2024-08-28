#pragma once

#include "irrt/int_types.hpp"

extern "C" {
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

SliceIndex __nac3_range_slice_len(const SliceIndex start, const SliceIndex end, const SliceIndex step) {
    SliceIndex diff = end - start;
    if (diff > 0 && step > 0) {
        return ((diff - 1) / step) + 1;
    } else if (diff < 0 && step < 0) {
        return ((diff + 1) / step) + 1;
    } else {
        return 0;
    }
}
}