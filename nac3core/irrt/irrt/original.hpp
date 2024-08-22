#pragma once

#include <irrt/int_types.hpp>
#include <irrt/math_util.hpp>

// The type of an index or a value describing the length of a range/slice is always `int32_t`.
using SliceIndex = int32_t;

namespace
{
// adapted from GNU Scientific Library: https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
// need to make sure `exp >= 0` before calling this function
template <typename T> T __nac3_int_exp_impl(T base, T exp)
{
    T res = 1;
    /* repeated squaring method */
    do
    {
        if (exp & 1)
        {
            res *= base; /* for n odd */
        }
        exp >>= 1;
        base *= base;
    } while (exp);
    return res;
}
} // namespace

extern "C"
{
#define DEF_nac3_int_exp_(T)                                                                                           \
    T __nac3_int_exp_##T(T base, T exp)                                                                                \
    {                                                                                                                  \
        return __nac3_int_exp_impl(base, exp);                                                                         \
    }

    DEF_nac3_int_exp_(int32_t) DEF_nac3_int_exp_(int64_t) DEF_nac3_int_exp_(uint32_t) DEF_nac3_int_exp_(uint64_t)

        SliceIndex __nac3_slice_index_bound(SliceIndex i, const SliceIndex len)
    {
        if (i < 0)
        {
            i = len + i;
        }
        if (i < 0)
        {
            return 0;
        }
        else if (i > len)
        {
            return len;
        }
        return i;
    }

    SliceIndex __nac3_range_slice_len(const SliceIndex start, const SliceIndex end, const SliceIndex step)
    {
        SliceIndex diff = end - start;
        if (diff > 0 && step > 0)
        {
            return ((diff - 1) / step) + 1;
        }
        else if (diff < 0 && step < 0)
        {
            return ((diff + 1) / step) + 1;
        }
        else
        {
            return 0;
        }
    }

    // Handle list assignment and dropping part of the list when
    // both dest_step and src_step are +1.
    // - All the index must *not* be out-of-bound or negative,
    // - The end index is *inclusive*,
    // - The length of src and dest slice size should already
    // be checked: if dest.step == 1 then len(src) <= len(dest) else len(src) == len(dest)
    SliceIndex __nac3_list_slice_assign_var_size(SliceIndex dest_start, SliceIndex dest_end, SliceIndex dest_step,
                                                 uint8_t *dest_arr, SliceIndex dest_arr_len, SliceIndex src_start,
                                                 SliceIndex src_end, SliceIndex src_step, uint8_t *src_arr,
                                                 SliceIndex src_arr_len, const SliceIndex size)
    {
        /* if dest_arr_len == 0, do nothing since we do not support extending list */
        if (dest_arr_len == 0)
            return dest_arr_len;
        /* if both step is 1, memmove directly, handle the dropping of the list, and shrink size */
        if (src_step == dest_step && dest_step == 1)
        {
            const SliceIndex src_len = (src_end >= src_start) ? (src_end - src_start + 1) : 0;
            const SliceIndex dest_len = (dest_end >= dest_start) ? (dest_end - dest_start + 1) : 0;
            if (src_len > 0)
            {
                __builtin_memmove(dest_arr + dest_start * size, src_arr + src_start * size, src_len * size);
            }
            if (dest_len > 0)
            {
                /* dropping */
                __builtin_memmove(dest_arr + (dest_start + src_len) * size, dest_arr + (dest_end + 1) * size,
                                  (dest_arr_len - dest_end - 1) * size);
            }
            /* shrink size */
            return dest_arr_len - (dest_len - src_len);
        }
        /* if two range overlaps, need alloca */
        uint8_t need_alloca = (dest_arr == src_arr) && !(max(dest_start, dest_end) < min(src_start, src_end) ||
                                                         max(src_start, src_end) < min(dest_start, dest_end));
        if (need_alloca)
        {
            uint8_t *tmp = reinterpret_cast<uint8_t *>(__builtin_alloca(src_arr_len * size));
            __builtin_memcpy(tmp, src_arr, src_arr_len * size);
            src_arr = tmp;
        }
        SliceIndex src_ind = src_start;
        SliceIndex dest_ind = dest_start;
        for (; (src_step > 0) ? (src_ind <= src_end) : (src_ind >= src_end); src_ind += src_step, dest_ind += dest_step)
        {
            /* for constant optimization */
            if (size == 1)
            {
                __builtin_memcpy(dest_arr + dest_ind, src_arr + src_ind, 1);
            }
            else if (size == 4)
            {
                __builtin_memcpy(dest_arr + dest_ind * 4, src_arr + src_ind * 4, 4);
            }
            else if (size == 8)
            {
                __builtin_memcpy(dest_arr + dest_ind * 8, src_arr + src_ind * 8, 8);
            }
            else
            {
                /* memcpy for var size, cannot overlap after previous alloca */
                __builtin_memcpy(dest_arr + dest_ind * size, src_arr + src_ind * size, size);
            }
        }
        /* only dest_step == 1 can we shrink the dest list. */
        /* size should be ensured prior to calling this function */
        if (dest_step == 1 && dest_end >= dest_start)
        {
            __builtin_memmove(dest_arr + dest_ind * size, dest_arr + (dest_end + 1) * size,
                              (dest_arr_len - dest_end - 1) * size);
            return dest_arr_len - (dest_end - dest_ind) - 1;
        }
        return dest_arr_len;
    }

    int32_t __nac3_isinf(double x)
    {
        return __builtin_isinf(x);
    }

    int32_t __nac3_isnan(double x)
    {
        return __builtin_isnan(x);
    }

    double tgamma(double arg);

    double __nac3_gamma(double z)
    {
        // Handling for denormals
        //     | x                 | Python gamma(x) | C tgamma(x) |
        // --- | ----------------- | --------------- | ----------- |
        // (1) | nan               | nan             | nan         |
        // (2) | -inf              | -inf            | inf         |
        // (3) | inf               | inf             | inf         |
        // (4) | 0.0               | inf             | inf         |
        // (5) | {-1.0, -2.0, ...} | inf             | nan         |

        // (1)-(3)
        if (__builtin_isinf(z) || __builtin_isnan(z))
        {
            return z;
        }

        double v = tgamma(z);

        // (4)-(5)
        return __builtin_isinf(v) || __builtin_isnan(v) ? __builtin_inf() : v;
    }

    double lgamma(double arg);

    double __nac3_gammaln(double x)
    {
        // libm's handling of value overflows differs from scipy:
        // - scipy: gammaln(-inf) -> -inf
        // - libm : lgamma(-inf) -> inf

        if (__builtin_isinf(x))
        {
            return x;
        }

        return lgamma(x);
    }

    double j0(double x);

    double __nac3_j0(double x)
    {
        // libm's handling of value overflows differs from scipy:
        // - scipy: j0(inf) -> nan
        // - libm : j0(inf) -> 0.0

        if (__builtin_isinf(x))
        {
            return __builtin_nan("");
        }

        return j0(x);
    }
} // extern "C"