typedef _ExtInt(8) int8_t;
typedef unsigned _ExtInt(8) uint8_t;
typedef _ExtInt(32) int32_t;
typedef unsigned _ExtInt(32) uint32_t;
typedef _ExtInt(64) int64_t;
typedef unsigned _ExtInt(64) uint64_t;

# define MAX(a, b) (a > b ? a : b)
# define MIN(a, b) (a > b ? b : a)

int32_t __nac3_irrt_range_slice_len(const int32_t start, const int32_t end, const int32_t step) {
    int32_t diff = end - start;
    if (diff > 0 && step > 0) {
        return ((diff - 1) / step) + 1;
    } else if (diff < 0 && step < 0) {
        return ((diff + 1) / step) + 1;
    } else {
        return 0;
    }
}

// adapted from GNU Scientific Library: https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
// need to make sure `exp >= 0` before calling this function
# define \
    DEF_INT_EXP(T) \
T __nac3_irrt_int_exp_##T( \
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
