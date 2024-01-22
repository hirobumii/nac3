typedef _BitInt(8) int8_t;
typedef unsigned _BitInt(8) uint8_t;
typedef _BitInt(32) int32_t;
typedef unsigned _BitInt(32) uint32_t;
typedef _BitInt(64) int64_t;
typedef unsigned _BitInt(64) uint64_t;

# define MAX(a, b) (a > b ? a : b)
# define MIN(a, b) (a > b ? b : a)

// adapted from GNU Scientific Library: https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
// need to make sure `exp >= 0` before calling this function
#define DEF_INT_EXP(T) T __nac3_int_exp_##T( \
    T base, \
    T exp \
) { \
    T res = (T)1; \
    /* repeated squaring method */ \
    do { \
       if (exp & 1) res *= base;  /* for n odd */ \
       exp >>= 1; \
       base *= base; \
    } while (exp); \
    return res; \
} \

DEF_INT_EXP(int32_t)
DEF_INT_EXP(int64_t)
DEF_INT_EXP(uint32_t)
DEF_INT_EXP(uint64_t)


int32_t __nac3_slice_index_bound(int32_t i, const int32_t len) {
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

int32_t __nac3_range_slice_len(const int32_t start, const int32_t end, const int32_t step) {
    int32_t diff = end - start;
    if (diff > 0 && step > 0) {
        return ((diff - 1) / step) + 1;
    } else if (diff < 0 && step < 0) {
        return ((diff + 1) / step) + 1;
    } else {
        return 0;
    }
}

// Handle list assignment and dropping part of the list when
// both dest_step and src_step are +1.
// - All the index must *not* be out-of-bound or negative,
// - The end index is *inclusive*,
// - The length of src and dest slice size should already
// be checked: if dest.step == 1 then len(src) <= len(dest) else len(src) == len(dest)
int32_t __nac3_list_slice_assign_var_size(
    int32_t dest_start,
    int32_t dest_end,
    int32_t dest_step,
    uint8_t *dest_arr,
    int32_t dest_arr_len,
    int32_t src_start,
    int32_t src_end,
    int32_t src_step,
    uint8_t *src_arr,
    int32_t src_arr_len,
    const int32_t size
) {
    /* if dest_arr_len == 0, do nothing since we do not support extending list */
    if (dest_arr_len == 0) return dest_arr_len;
    /* if both step is 1, memmove directly, handle the dropping of the list, and shrink size */
    if (src_step == dest_step && dest_step == 1) {
        const int32_t src_len = (src_end >= src_start) ? (src_end - src_start + 1) : 0;
        const int32_t dest_len = (dest_end >= dest_start) ? (dest_end - dest_start + 1) : 0;
        if (src_len > 0) {
            __builtin_memmove(
                dest_arr + dest_start * size,
                src_arr + src_start * size,
                src_len * size
            );
        }
        if (dest_len > 0) {
            /* dropping */
            __builtin_memmove(
                dest_arr + (dest_start + src_len) * size,
                dest_arr + (dest_end + 1) * size,
                (dest_arr_len - dest_end - 1) * size
            );
        }
        /* shrink size */
        return dest_arr_len - (dest_len - src_len);
    }
    /* if two range overlaps, need alloca */
    uint8_t need_alloca =
        (dest_arr == src_arr)
        && !(
            MAX(dest_start, dest_end) < MIN(src_start, src_end)
            || MAX(src_start, src_end) < MIN(dest_start, dest_end)
        );
    if (need_alloca) {
        uint8_t *tmp = __builtin_alloca(src_arr_len * size);
        __builtin_memcpy(tmp, src_arr, src_arr_len * size);
        src_arr = tmp;
    }
    int32_t src_ind = src_start;
    int32_t dest_ind = dest_start;
    for (;
        (src_step > 0) ? (src_ind <= src_end) : (src_ind >= src_end);
        src_ind += src_step, dest_ind += dest_step
    ) {
        /* for constant optimization */
        if (size == 1) {
            __builtin_memcpy(dest_arr + dest_ind, src_arr + src_ind, 1);
        } else if (size == 4) {
            __builtin_memcpy(dest_arr + dest_ind * 4, src_arr + src_ind * 4, 4);
        } else if (size == 8) {
            __builtin_memcpy(dest_arr + dest_ind * 8, src_arr + src_ind * 8, 8);
        } else {
            /* memcpy for var size, cannot overlap after previous alloca */
            __builtin_memcpy(dest_arr + dest_ind * size, src_arr + src_ind * size, size);
        }
    }
    /* only dest_step == 1 can we shrink the dest list. */
    /* size should be ensured prior to calling this function */
    if (dest_step == 1 && dest_end >= dest_start) {
        __builtin_memmove(
            dest_arr + dest_ind * size,
            dest_arr + (dest_end + 1) * size,
            (dest_arr_len - dest_end - 1) * size
        );
        return dest_arr_len - (dest_end - dest_ind) - 1;
    }
    return dest_arr_len;
}

int32_t __nac3_isinf(double x) {
    return __builtin_isinf(x);
}

int32_t __nac3_isnan(double x) {
    return __builtin_isnan(x);
}

double tgamma(double arg);

double __nac3_gamma(double z) {
    // Handling for denormals
    //     | x                 | Python gamma(x) | C tgamma(x) |
    // --- | ----------------- | --------------- | ----------- |
    // (1) | nan               | nan             | nan         |
    // (2) | -inf              | -inf            | inf         |
    // (3) | inf               | inf             | inf         |
    // (4) | 0.0               | inf             | inf         |
    // (5) | {-1.0, -2.0, ...} | inf             | nan         |

    // (1)-(3)
    if (__builtin_isinf(z) || __builtin_isnan(z)) {
        return z;
    }

    double v = tgamma(z);

    // (4)-(5)
    return __builtin_isinf(v) || __builtin_isnan(v) ? __builtin_inf() : v;
}

double lgamma(double arg);

double __nac3_gammaln(double x) {
    // libm's handling of value overflows differs from scipy:
    // - scipy: gammaln(-inf) -> -inf
    // - libm : lgamma(-inf) -> inf

    if (__builtin_isinf(x)) {
        return x;
    }

    return lgamma(x);
}

double j0(double x);

double __nac3_j0(double x) {
    // libm's handling of value overflows differs from scipy:
    // - scipy: j0(inf) -> nan
    // - libm : j0(inf) -> 0.0

    if (__builtin_isinf(x)) {
        return __builtin_nan("");
    }

    return j0(x);
}

uint32_t __nac3_ndarray_calc_size(
    const uint64_t *list_data,
    uint32_t list_len
) {
    uint32_t num_elems = 1;
    for (uint32_t i = 0; i < list_len; ++i) {
        uint64_t val = list_data[i];
        __builtin_assume(val >= 0);
        num_elems *= list_data[i];
    }
    return num_elems;
}

uint64_t __nac3_ndarray_calc_size64(
    const uint64_t *list_data,
    uint64_t list_len
) {
    uint64_t num_elems = 1;
    for (uint64_t i = 0; i < list_len; ++i) {
        uint64_t val = list_data[i];
        __builtin_assume(val >= 0);
       num_elems *= list_data[i];
    }
    return num_elems;
}

void __nac3_ndarray_init_dims(
    uint32_t *ndarray_dims,
    const int32_t *shape_data,
    uint32_t shape_len
) {
    __builtin_memcpy(ndarray_dims, shape_data, shape_len * sizeof(int32_t));
}

void __nac3_ndarray_init_dims64(
    uint64_t *ndarray_dims,
    const int32_t *shape_data,
    uint64_t shape_len
) {
    for (uint64_t i = 0; i < shape_len; ++i) {
        ndarray_dims[i] = (uint64_t) shape_data[i];
    }
}

void __nac3_ndarray_calc_nd_indices(
    uint32_t index,
    const uint32_t* dims,
    uint32_t num_dims,
    uint32_t* idxs
) {
    uint32_t stride = 1;
    for (uint32_t dim = 0; dim < num_dims; dim++) {
        uint32_t i = num_dims - dim - 1;
        idxs[i] = (index / stride) % dims[i];
        stride *= dims[i];
    }
}

void __nac3_ndarray_calc_nd_indices64(
    uint64_t index,
    const uint64_t* dims,
    uint64_t num_dims,
    uint64_t* idxs
) {
    uint64_t stride = 1;
    for (uint64_t dim = 0; dim < num_dims; dim++) {
        uint64_t i = num_dims - dim - 1;
        idxs[i] = (index / stride) % dims[i];
        stride *= dims[i];
    }
}

uint32_t __nac3_ndarray_flatten_index(
    const uint32_t* dims,
    uint32_t num_dims,
    const uint32_t* indices,
    uint32_t num_indices
) {
    uint32_t idx = 0;
    uint32_t stride = 1;
    for (uint32_t i = num_dims - 1; i-- >= 0; ) {
        if (i < num_indices) {
            idx += (stride * indices[i]);
        }

        stride *= dims[i];
    }
    return idx;
}

uint64_t __nac3_ndarray_flatten_index64(
    const uint64_t* dims,
    uint64_t num_dims,
    const uint32_t* indices,
    uint64_t num_indices
) {
    uint64_t idx = 0;
    uint64_t stride = 1;
    for (uint64_t i = num_dims - 1; i-- >= 0; ) {
        if (i < num_indices) {
            idx += (stride * indices[i]);
        }

        stride *= dims[i];
    }
    return idx;
}
