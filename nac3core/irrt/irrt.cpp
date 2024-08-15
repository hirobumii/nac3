using int8_t = _BitInt(8);
using uint8_t = unsigned _BitInt(8);
using int32_t = _BitInt(32);
using uint32_t = unsigned _BitInt(32);
using int64_t = _BitInt(64);
using uint64_t = unsigned _BitInt(64);

// NDArray indices are always `uint32_t`.
using NDIndex = uint32_t;
// The type of an index or a value describing the length of a range/slice is always `int32_t`.
using SliceIndex = int32_t;

namespace
{
template <typename T> const T &max(const T &a, const T &b)
{
    return a > b ? a : b;
}

template <typename T> const T &min(const T &a, const T &b)
{
    return a > b ? b : a;
}

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

template <typename SizeT>
SizeT __nac3_ndarray_calc_size_impl(const SizeT *list_data, SizeT list_len, SizeT begin_idx, SizeT end_idx)
{
    __builtin_assume(end_idx <= list_len);

    SizeT num_elems = 1;
    for (SizeT i = begin_idx; i < end_idx; ++i)
    {
        SizeT val = list_data[i];
        __builtin_assume(val > 0);
        num_elems *= val;
    }
    return num_elems;
}

template <typename SizeT>
void __nac3_ndarray_calc_nd_indices_impl(SizeT index, const SizeT *dims, SizeT num_dims, NDIndex *idxs)
{
    SizeT stride = 1;
    for (SizeT dim = 0; dim < num_dims; dim++)
    {
        SizeT i = num_dims - dim - 1;
        __builtin_assume(dims[i] > 0);
        idxs[i] = (index / stride) % dims[i];
        stride *= dims[i];
    }
}

template <typename SizeT>
SizeT __nac3_ndarray_flatten_index_impl(const SizeT *dims, SizeT num_dims, const NDIndex *indices, SizeT num_indices)
{
    SizeT idx = 0;
    SizeT stride = 1;
    for (SizeT i = 0; i < num_dims; ++i)
    {
        SizeT ri = num_dims - i - 1;
        if (ri < num_indices)
        {
            idx += stride * indices[ri];
        }

        __builtin_assume(dims[i] > 0);
        stride *= dims[ri];
    }
    return idx;
}

template <typename SizeT>
void __nac3_ndarray_calc_broadcast_impl(const SizeT *lhs_dims, SizeT lhs_ndims, const SizeT *rhs_dims, SizeT rhs_ndims,
                                        SizeT *out_dims)
{
    SizeT max_ndims = lhs_ndims > rhs_ndims ? lhs_ndims : rhs_ndims;

    for (SizeT i = 0; i < max_ndims; ++i)
    {
        const SizeT *lhs_dim_sz = i < lhs_ndims ? &lhs_dims[lhs_ndims - i - 1] : nullptr;
        const SizeT *rhs_dim_sz = i < rhs_ndims ? &rhs_dims[rhs_ndims - i - 1] : nullptr;
        SizeT *out_dim = &out_dims[max_ndims - i - 1];

        if (lhs_dim_sz == nullptr)
        {
            *out_dim = *rhs_dim_sz;
        }
        else if (rhs_dim_sz == nullptr)
        {
            *out_dim = *lhs_dim_sz;
        }
        else if (*lhs_dim_sz == 1)
        {
            *out_dim = *rhs_dim_sz;
        }
        else if (*rhs_dim_sz == 1)
        {
            *out_dim = *lhs_dim_sz;
        }
        else if (*lhs_dim_sz == *rhs_dim_sz)
        {
            *out_dim = *lhs_dim_sz;
        }
        else
        {
            __builtin_unreachable();
        }
    }
}

template <typename SizeT>
void __nac3_ndarray_calc_broadcast_idx_impl(const SizeT *src_dims, SizeT src_ndims, const NDIndex *in_idx,
                                            NDIndex *out_idx)
{
    for (SizeT i = 0; i < src_ndims; ++i)
    {
        SizeT src_i = src_ndims - i - 1;
        out_idx[src_i] = src_dims[src_i] == 1 ? 0 : in_idx[src_i];
    }
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

    uint32_t __nac3_ndarray_calc_size(const uint32_t *list_data, uint32_t list_len, uint32_t begin_idx,
                                      uint32_t end_idx)
    {
        return __nac3_ndarray_calc_size_impl(list_data, list_len, begin_idx, end_idx);
    }

    uint64_t __nac3_ndarray_calc_size64(const uint64_t *list_data, uint64_t list_len, uint64_t begin_idx,
                                        uint64_t end_idx)
    {
        return __nac3_ndarray_calc_size_impl(list_data, list_len, begin_idx, end_idx);
    }

    void __nac3_ndarray_calc_nd_indices(uint32_t index, const uint32_t *dims, uint32_t num_dims, NDIndex *idxs)
    {
        __nac3_ndarray_calc_nd_indices_impl(index, dims, num_dims, idxs);
    }

    void __nac3_ndarray_calc_nd_indices64(uint64_t index, const uint64_t *dims, uint64_t num_dims, NDIndex *idxs)
    {
        __nac3_ndarray_calc_nd_indices_impl(index, dims, num_dims, idxs);
    }

    uint32_t __nac3_ndarray_flatten_index(const uint32_t *dims, uint32_t num_dims, const NDIndex *indices,
                                          uint32_t num_indices)
    {
        return __nac3_ndarray_flatten_index_impl(dims, num_dims, indices, num_indices);
    }

    uint64_t __nac3_ndarray_flatten_index64(const uint64_t *dims, uint64_t num_dims, const NDIndex *indices,
                                            uint64_t num_indices)
    {
        return __nac3_ndarray_flatten_index_impl(dims, num_dims, indices, num_indices);
    }

    void __nac3_ndarray_calc_broadcast(const uint32_t *lhs_dims, uint32_t lhs_ndims, const uint32_t *rhs_dims,
                                       uint32_t rhs_ndims, uint32_t *out_dims)
    {
        return __nac3_ndarray_calc_broadcast_impl(lhs_dims, lhs_ndims, rhs_dims, rhs_ndims, out_dims);
    }

    void __nac3_ndarray_calc_broadcast64(const uint64_t *lhs_dims, uint64_t lhs_ndims, const uint64_t *rhs_dims,
                                         uint64_t rhs_ndims, uint64_t *out_dims)
    {
        return __nac3_ndarray_calc_broadcast_impl(lhs_dims, lhs_ndims, rhs_dims, rhs_ndims, out_dims);
    }

    void __nac3_ndarray_calc_broadcast_idx(const uint32_t *src_dims, uint32_t src_ndims, const NDIndex *in_idx,
                                           NDIndex *out_idx)
    {
        __nac3_ndarray_calc_broadcast_idx_impl(src_dims, src_ndims, in_idx, out_idx);
    }

    void __nac3_ndarray_calc_broadcast_idx64(const uint64_t *src_dims, uint64_t src_ndims, const NDIndex *in_idx,
                                             NDIndex *out_idx)
    {
        __nac3_ndarray_calc_broadcast_idx_impl(src_dims, src_ndims, in_idx, out_idx);
    }
} // extern "C"