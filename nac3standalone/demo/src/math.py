@extern
def output_bool(x: bool):
    ...

@extern
def output_int32(x: int32):
    ...

@extern
def output_int64(x: int64):
    ...

@extern
def output_float64(x: float):
    ...

@extern
def dbl_nan() -> float:
    ...

@extern
def dbl_inf() -> float:
    ...

def dbl_pi() -> float:
    return 3.1415926535897932384626433

def dbl_e() -> float:
    return 2.71828182845904523536028747135266249775724709369995

def test_round():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int32(round(x))

def test_round64():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int64(round64(x))

def test_isnan():
    for x in [dbl_nan(), 0.0, dbl_inf()]:
        output_bool(isnan(x))

def test_isinf():
    for x in [dbl_inf(), -dbl_inf(), 0.0, dbl_nan()]:
        output_bool(isinf(x))

def test_sin():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sin(x))

def test_cos():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(cos(x))

def test_exp():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(exp(x))

def test_exp2():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(exp2(x))

def test_log():
    e = dbl_e()
    for x in [1.0, e, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(log(x))

def test_log10():
    for x in [1.0, 10.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(log10(x))

def test_log2():
    for x in [1.0, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(log2(x))

def test_fabs():
    for x in [-1.0, 0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(fabs(x))

def test_floor():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int32(floor(x))

def test_floor64():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int64(floor64(x))

def test_ceil():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int32(ceil(x))

def test_ceil64():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int64(ceil64(x))

def test_sqrt():
    for x in [1.0, 2.0, 4.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sqrt(x))

def test_rint():
    for x in [-1.5, -0.5, 0.5, 1.5, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(rint(x))

def test_tan():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(tan(x))

def test_arcsin():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arcsin(x))

def test_arccos():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arccos(x))

def test_arctan():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arctan(x))

def test_sinh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sinh(x))

def test_cosh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(cosh(x))

def test_tanh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(tanh(x))

def test_arcsinh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arcsinh(x))

def test_arccosh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arccosh(x))

def test_arctanh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(arctanh(x))

def test_expm1():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(expm1(x))

def test_cbrt():
    for x in [1.0, 8.0, 27.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(expm1(x))

def test_erf():
    for x in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(erf(x))

def test_erfc():
    for x in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(erfc(x))

def test_gamma():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(gamma(x))

def test_gammaln():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(gammaln(x))

def test_j0():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(j0(x))

def test_j1():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0]:
        output_float64(j1(x))

def test_arctan2():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(arctan2(x1, x2))

def test_copysign():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(copysign(x1, x2))

def test_fmax():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(fmax(x1, x2))

def test_fmin():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(fmin(x1, x2))

def test_ldexp():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2, -1, 0, 1, 2]:
            output_float64(ldexp(x1, x2))

def test_hypot():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(hypot(x1, x2))

def test_nextafter():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(nextafter(x1, x2))

def run() -> int32:
    test_round()
    test_round64()
    test_isnan()
    test_isinf()
    test_sin()
    test_cos()
    test_exp()
    test_exp2()
    test_log()
    test_log10()
    test_log2()
    test_fabs()
    test_floor()
    test_floor64()
    test_ceil()
    test_ceil64()
    test_sqrt()
    test_rint()
    test_tan()
    test_arcsin()
    test_arccos()
    test_arctan()
    test_sinh()
    test_cosh()
    test_tanh()
    test_arcsinh()
    test_arccosh()
    test_arctanh()
    test_expm1()
    test_cbrt()
    test_erf()
    test_erfc()
    test_gamma()
    test_gammaln()
    test_j0()
    test_j1()
    test_arctan2()
    test_copysign()
    test_fmax()
    test_fmin()
    test_ldexp()
    test_hypot()
    test_nextafter()

    return 0