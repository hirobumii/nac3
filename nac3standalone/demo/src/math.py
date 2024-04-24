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

def test_np_round():
    for x in [-1.5, -0.5, 0.5, 1.5, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_round(x))

def test_np_isnan():
    for x in [dbl_nan(), 0.0, dbl_inf()]:
        output_bool(np_isnan(x))

def test_np_isinf():
    for x in [dbl_inf(), -dbl_inf(), 0.0, dbl_nan()]:
        output_bool(np_isinf(x))

def test_np_sin():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_sin(x))

def test_np_cos():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_cos(x))

def test_np_exp():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_exp(x))

def test_np_exp2():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_exp2(x))

def test_np_log():
    e = dbl_e()
    for x in [1.0, e, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_log(x))

def test_np_log10():
    for x in [1.0, 10.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_log10(x))

def test_np_log2():
    for x in [1.0, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_log2(x))

def test_np_fabs():
    for x in [-1.0, 0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_fabs(x))

def test_floor():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int32(floor(x))

def test_floor64():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int64(floor64(x))

def test_np_floor():
    for x in [-1.5, -0.5, 0.5, 1.5, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_floor(x))

def test_ceil():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int32(ceil(x))

def test_ceil64():
    for x in [-1.5, -0.5, 0.5, 1.5]:
        output_int64(ceil64(x))

def test_np_ceil():
    for x in [-1.5, -0.5, 0.5, 1.5, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_ceil(x))

def test_np_sqrt():
    for x in [1.0, 2.0, 4.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_sqrt(x))

def test_np_rint():
    for x in [-1.5, -0.5, 0.5, 1.5, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_rint(x))

def test_np_tan():
    pi = dbl_pi()
    for x in [-pi, -pi / 2.0, -pi / 4.0, 0.0, pi / 4.0, pi / 2.0, pi, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_tan(x))

def test_np_arcsin():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arcsin(x))

def test_np_arccos():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arccos(x))

def test_np_arctan():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arctan(x))

def test_np_sinh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_sinh(x))

def test_np_cosh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_cosh(x))

def test_np_tanh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_tanh(x))

def test_np_arcsinh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arcsinh(x))

def test_np_arccosh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arccosh(x))

def test_np_arctanh():
    for x in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_arctanh(x))

def test_np_expm1():
    for x in [0.0, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_expm1(x))

def test_np_cbrt():
    for x in [1.0, 8.0, 27.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(np_cbrt(x))

def test_sp_spec_erf():
    for x in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sp_spec_erf(x))

def test_sp_spec_erfc():
    for x in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sp_spec_erfc(x))

def test_sp_spec_gamma():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sp_spec_gamma(x))

def test_sp_spec_gammaln():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sp_spec_gammaln(x))

def test_sp_spec_j0():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        output_float64(sp_spec_j0(x))

def test_sp_spec_j1():
    for x in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0]:
        output_float64(sp_spec_j1(x))

def test_np_arctan2():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_arctan2(x1, x2))

def test_np_copysign():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_copysign(x1, x2))

def test_np_fmax():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_fmax(x1, x2))

def test_np_fmin():
    for x1 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-1.0, -0.5, 0.0, 0.5, 1.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_fmin(x1, x2))

def test_np_ldexp():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2, -1, 0, 1, 2]:
            output_float64(np_ldexp(x1, x2))

def test_np_hypot():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_hypot(x1, x2))

def test_np_nextafter():
    for x1 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
        for x2 in [-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, dbl_inf(), -dbl_inf(), dbl_nan()]:
            output_float64(np_nextafter(x1, x2))

def run() -> int32:
    test_round()
    test_round64()
    test_np_round()
    test_np_isnan()
    test_np_isinf()
    test_np_sin()
    test_np_cos()
    test_np_exp()
    test_np_exp2()
    test_np_log()
    test_np_log10()
    test_np_log2()
    test_np_fabs()
    test_floor()
    test_floor64()
    test_np_floor()
    test_ceil()
    test_ceil64()
    test_np_ceil()
    test_np_sqrt()
    test_np_rint()
    test_np_tan()
    test_np_arcsin()
    test_np_arccos()
    test_np_arctan()
    test_np_sinh()
    test_np_cosh()
    test_np_tanh()
    test_np_arcsinh()
    test_np_arccosh()
    test_np_arctanh()
    test_np_expm1()
    test_np_cbrt()
    test_sp_spec_erf()
    test_sp_spec_erfc()
    test_sp_spec_gamma()
    test_sp_spec_gammaln()
    test_sp_spec_j0()
    test_sp_spec_j1()
    test_np_arctan2()
    test_np_copysign()
    test_np_fmax()
    test_np_fmin()
    test_np_ldexp()
    test_np_hypot()
    test_np_nextafter()

    return 0