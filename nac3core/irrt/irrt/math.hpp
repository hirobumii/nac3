#pragma once

#include "irrt/int_types.hpp"

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
}  // namespace