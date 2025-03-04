#pragma once

extern "C" {
#define DEF_builtin_unary(RET, NAME, TY) \
    RET __nac3_##NAME(TY v) { return __builtin_##NAME(v); }
#define DEF_builtin_binary(RET, NAME, TY1, TY2) \
    RET __nac3_##NAME(TY1 v1, TY2 v2) { return __builtin_##NAME(v1, v2); }

DEF_builtin_unary(bool, isinf, double);
DEF_builtin_unary(bool, isnan, double);
DEF_builtin_unary(double, tan, double);
DEF_builtin_unary(double, asin, double);
DEF_builtin_unary(double, acos, double);
DEF_builtin_unary(double, atan, double);
DEF_builtin_unary(double, sinh, double);
DEF_builtin_unary(double, cosh, double);
DEF_builtin_unary(double, tanh, double);
DEF_builtin_unary(double, asinh, double);
DEF_builtin_unary(double, acosh, double);
DEF_builtin_unary(double, atanh, double);
DEF_builtin_unary(double, expm1, double);
DEF_builtin_unary(double, cbrt, double);
DEF_builtin_unary(double, erf, double);
DEF_builtin_unary(double, erfc, double);

#define __builtin_gamma __builtin_tgamma
DEF_builtin_unary(double, gamma, double);
#undef __builtin_gamma

DEF_builtin_binary(double, atan2, double, double);
DEF_builtin_binary(double, hypot, double, double);
DEF_builtin_binary(double, nextafter, double, double);
DEF_builtin_binary(double, ldexp, double, int);
}