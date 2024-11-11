#pragma once

#include "irrt/int_types.hpp"
#include "irrt/math_util.hpp"

extern "C" {
// Handle list assignment and dropping part of the list when
// both dest_step and src_step are +1.
// - All the index must *not* be out-of-bound or negative,
// - The end index is *inclusive*,
// - The length of src and dest slice size should already
// be checked: if dest.step == 1 then len(src) <= len(dest) else len(src) == len(dest)
SliceIndex __nac3_list_slice_assign_var_size(SliceIndex dest_start,
                                             SliceIndex dest_end,
                                             SliceIndex dest_step,
                                             void* dest_arr,
                                             SliceIndex dest_arr_len,
                                             SliceIndex src_start,
                                             SliceIndex src_end,
                                             SliceIndex src_step,
                                             void* src_arr,
                                             SliceIndex src_arr_len,
                                             const SliceIndex size) {
    /* if dest_arr_len == 0, do nothing since we do not support extending list */
    if (dest_arr_len == 0)
        return dest_arr_len;
    /* if both step is 1, memmove directly, handle the dropping of the list, and shrink size */
    if (src_step == dest_step && dest_step == 1) {
        const SliceIndex src_len = (src_end >= src_start) ? (src_end - src_start + 1) : 0;
        const SliceIndex dest_len = (dest_end >= dest_start) ? (dest_end - dest_start + 1) : 0;
        if (src_len > 0) {
            __builtin_memmove(static_cast<uint8_t*>(dest_arr) + dest_start * size,
                              static_cast<uint8_t*>(src_arr) + src_start * size, src_len * size);
        }
        if (dest_len > 0) {
            /* dropping */
            __builtin_memmove(static_cast<uint8_t*>(dest_arr) + (dest_start + src_len) * size,
                              static_cast<uint8_t*>(dest_arr) + (dest_end + 1) * size,
                              (dest_arr_len - dest_end - 1) * size);
        }
        /* shrink size */
        return dest_arr_len - (dest_len - src_len);
    }
    /* if two range overlaps, need alloca */
    uint8_t need_alloca = (dest_arr == src_arr)
                          && !(max(dest_start, dest_end) < min(src_start, src_end)
                               || max(src_start, src_end) < min(dest_start, dest_end));
    if (need_alloca) {
        void* tmp = __builtin_alloca(src_arr_len * size);
        __builtin_memcpy(tmp, src_arr, src_arr_len * size);
        src_arr = tmp;
    }
    SliceIndex src_ind = src_start;
    SliceIndex dest_ind = dest_start;
    for (; (src_step > 0) ? (src_ind <= src_end) : (src_ind >= src_end); src_ind += src_step, dest_ind += dest_step) {
        /* for constant optimization */
        if (size == 1) {
            __builtin_memcpy(static_cast<uint8_t*>(dest_arr) + dest_ind, static_cast<uint8_t*>(src_arr) + src_ind, 1);
        } else if (size == 4) {
            __builtin_memcpy(static_cast<uint8_t*>(dest_arr) + dest_ind * 4,
                             static_cast<uint8_t*>(src_arr) + src_ind * 4, 4);
        } else if (size == 8) {
            __builtin_memcpy(static_cast<uint8_t*>(dest_arr) + dest_ind * 8,
                             static_cast<uint8_t*>(src_arr) + src_ind * 8, 8);
        } else {
            /* memcpy for var size, cannot overlap after previous alloca */
            __builtin_memcpy(static_cast<uint8_t*>(dest_arr) + dest_ind * size,
                             static_cast<uint8_t*>(src_arr) + src_ind * size, size);
        }
    }
    /* only dest_step == 1 can we shrink the dest list. */
    /* size should be ensured prior to calling this function */
    if (dest_step == 1 && dest_end >= dest_start) {
        __builtin_memmove(static_cast<uint8_t*>(dest_arr) + dest_ind * size,
                          static_cast<uint8_t*>(dest_arr) + (dest_end + 1) * size,
                          (dest_arr_len - dest_end - 1) * size);
        return dest_arr_len - (dest_end - dest_ind) - 1;
    }
    return dest_arr_len;
}
}  // extern "C"