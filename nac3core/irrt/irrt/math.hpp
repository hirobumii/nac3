#pragma once

#include "irrt/stdlib/cstdint.h"

namespace {
// adapted from GNU Scientific Library: https://git.savannah.gnu.org/cgit/gsl.git/tree/sys/pow_int.c
// need to make sure `exp >= 0` before calling this function
template<typename T>
T __nac3_int_exp_impl(T base, T exp) {
    T res = 1;
    /* repeated squaring method */
    do {
        if (exp & 1) {
            res *= base; /* for n odd */
        }
        exp >>= 1;
        base *= base;
    } while (exp);
    return res;
}
}  // namespace

#define DEF_nac3_int_exp_(T)                   \
    T __nac3_int_exp_##T(T base, T exp) {      \
        return __nac3_int_exp_impl(base, exp); \
    }

extern "C" {

// Putting semicolons here to make clang-format not reformat this into
// a stair shape.
DEF_nac3_int_exp_(int32_t);
DEF_nac3_int_exp_(int64_t);
DEF_nac3_int_exp_(uint32_t);
DEF_nac3_int_exp_(uint64_t);

double __nac3_gammaln(double x) {
    // libm's handling of value overflows differs from scipy:
    // - scipy: gammaln(-inf) -> -inf
    // - libm : lgamma(-inf) -> inf

    if (__builtin_isinf(x)) {
        return x;
    }

    return __builtin_lgamma(x);
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
}  // namespace
