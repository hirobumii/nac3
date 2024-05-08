@extern
def dbl_nan() -> float:
    ...

@extern
def dbl_inf() -> float:
    ...

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
def output_uint32(x: uint32):
    ...

@extern
def output_uint64(x: uint64):
    ...

@extern
def output_float64(x: float):
    ...

def output_ndarray_bool_2(n: ndarray[bool, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_bool(n[r][c])

def output_ndarray_int32_1(n: ndarray[int32, Literal[1]]):
    for i in range(len(n)):
        output_int32(n[i])

def output_ndarray_int32_2(n: ndarray[int32, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_int32(n[r][c])

def output_ndarray_int64_2(n: ndarray[int64, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_int64(n[r][c])

def output_ndarray_uint32_2(n: ndarray[uint32, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_uint32(n[r][c])

def output_ndarray_uint64_2(n: ndarray[uint64, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_uint64(n[r][c])

def output_ndarray_float_1(n: ndarray[float, Literal[1]]):
    for i in range(len(n)):
        output_float64(n[i])

def output_ndarray_float_2(n: ndarray[float, Literal[2]]):
    for r in range(len(n)):
        for c in range(len(n[r])):
            output_float64(n[r][c])

def consume_ndarray_1(n: ndarray[float, Literal[1]]):
    pass

def test_ndarray_ctor():
    n: ndarray[float, Literal[1]] = np_ndarray([1])
    consume_ndarray_1(n)

def test_ndarray_empty():
    n: ndarray[float, 1] = np_empty([1])
    consume_ndarray_1(n)

def test_ndarray_zeros():
    n: ndarray[float, 1] = np_zeros([1])
    output_ndarray_float_1(n)

def test_ndarray_ones():
    n: ndarray[float, 1] = np_ones([1])
    output_ndarray_float_1(n)

def test_ndarray_full():
    n_float: ndarray[float, 1] = np_full([1], 2.0)
    output_ndarray_float_1(n_float)
    n_i32: ndarray[int32, 1] = np_full([1], 2)
    output_ndarray_int32_1(n_i32)

def test_ndarray_eye():
    n: ndarray[float, 2] = np_eye(2)
    output_ndarray_float_2(n)

def test_ndarray_identity():
    n: ndarray[float, 2] = np_identity(2)
    output_ndarray_float_2(n)

def test_ndarray_fill():
    n: ndarray[float, 2] = np_empty([2, 2])
    n.fill(1.0)
    output_ndarray_float_2(n)

def test_ndarray_copy():
    x: ndarray[float, 2] = np_identity(2)
    y = x.copy()
    x.fill(0.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_neg_idx():
    x = np_identity(2)

    for i in range(-1, -3, -1):
        for j in range(-1, -3, -1):
            output_float64(x[i][j])

def test_ndarray_add():
    x = np_identity(2)
    y = x + np_ones([2, 2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_add_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x + np_ones([2])
    y = x + np_ones([2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_add_broadcast_lhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = 1.0 + x
    y = 1.0 + x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_add_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x + 1.0
    y = x + 1.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_iadd():
    x = np_identity(2)
    x += np_ones([2, 2])

    output_ndarray_float_2(x)

def test_ndarray_iadd_broadcast():
    x = np_identity(2)
    x += np_ones([2])

    output_ndarray_float_2(x)

def test_ndarray_iadd_broadcast_scalar():
    x = np_identity(2)
    x += 1.0

    output_ndarray_float_2(x)

def test_ndarray_sub():
    x = np_ones([2, 2])
    y = x - np_identity(2)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_sub_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x - np_ones([2])
    y = x - np_ones([2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_sub_broadcast_lhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = 1.0 - x
    y = 1.0 - x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_sub_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x - 1
    y = x - 1.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_isub():
    x = np_ones([2, 2])
    x -= np_identity(2)

    output_ndarray_float_2(x)

def test_ndarray_isub_broadcast():
    x = np_identity(2)
    x -= np_ones([2])

    output_ndarray_float_2(x)

def test_ndarray_isub_broadcast_scalar():
    x = np_identity(2)
    x -= 1.0

    output_ndarray_float_2(x)

def test_ndarray_mul():
    x = np_ones([2, 2])
    y = x * np_identity(2)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mul_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x * np_ones([2])
    y = x * np_ones([2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mul_broadcast_lhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = 2.0 * x
    y = 2.0 * x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mul_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x * 2.0
    y = x * 2.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_imul():
    x = np_ones([2, 2])
    x *= np_identity(2)

    output_ndarray_float_2(x)

def test_ndarray_imul_broadcast():
    x = np_identity(2)
    x *= np_ones([2])

    output_ndarray_float_2(x)

def test_ndarray_imul_broadcast_scalar():
    x = np_identity(2)
    x *= 2.0

    output_ndarray_float_2(x)

def test_ndarray_truediv():
    x = np_identity(2)
    y = x / np_ones([2, 2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_truediv_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x / np_ones([2])
    y = x / np_ones([2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_truediv_broadcast_lhs_scalar():
    x = np_ones([2, 2])
    # y: ndarray[float, 2] = 2.0 / x
    y = 2.0 / x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_truediv_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x / 2.0
    y = x / 2.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_itruediv():
    x = np_identity(2)
    x /= np_ones([2, 2])

    output_ndarray_float_2(x)

def test_ndarray_itruediv_broadcast():
    x = np_identity(2)
    x /= np_ones([2])

    output_ndarray_float_2(x)

def test_ndarray_itruediv_broadcast_scalar():
    x = np_identity(2)
    x /= 2.0

    output_ndarray_float_2(x)

def test_ndarray_floordiv():
    x = np_identity(2)
    y = x // np_ones([2, 2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_floordiv_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x // np_ones([2])
    y = x // np_ones([2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_floordiv_broadcast_lhs_scalar():
    x = np_ones([2, 2])
    # y: ndarray[float, 2] = 2.0 // x
    y = 2.0 // x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_floordiv_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x // 2.0
    y = x // 2.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_ifloordiv():
    x = np_identity(2)
    x //= np_ones([2, 2])

    output_ndarray_float_2(x)

def test_ndarray_ifloordiv_broadcast():
    x = np_identity(2)
    x //= np_ones([2])

    output_ndarray_float_2(x)

def test_ndarray_ifloordiv_broadcast_scalar():
    x = np_identity(2)
    x //= 2.0

    output_ndarray_float_2(x)

def test_ndarray_mod():
    x = np_identity(2)
    y = x % np_full([2, 2], 2.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mod_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x % np_ones([2])
    y = x % np_full([2], 2.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mod_broadcast_lhs_scalar():
    x = np_ones([2, 2])
    # y: ndarray[float, 2] = 2.0 % x
    y = 2.0 % x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_mod_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x % 2.0
    y = x % 2.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_imod():
    x = np_identity(2)
    x %= np_full([2, 2], 2.0)

    output_ndarray_float_2(x)

def test_ndarray_imod_broadcast():
    x = np_identity(2)
    x %= np_full([2], 2.0)

    output_ndarray_float_2(x)

def test_ndarray_imod_broadcast_scalar():
    x = np_identity(2)
    x %= 2.0

    output_ndarray_float_2(x)

def test_ndarray_pow():
    x = np_identity(2)
    y = x ** np_full([2, 2], 2.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_pow_broadcast():
    x = np_identity(2)
    # y: ndarray[float, 2] = x ** np_full([2], 2.0)
    y = x ** np_full([2], 2.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_pow_broadcast_lhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = 2.0 ** x
    y = 2.0 ** x

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_pow_broadcast_rhs_scalar():
    x = np_identity(2)
    # y: ndarray[float, 2] = x % 2.0
    y = x ** 2.0

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_ipow():
    x = np_identity(2)
    x **= np_full([2, 2], 2.0)

    output_ndarray_float_2(x)

def test_ndarray_ipow_broadcast():
    x = np_identity(2)
    x **= np_full([2], 2.0)

    output_ndarray_float_2(x)

def test_ndarray_ipow_broadcast_scalar():
    x = np_identity(2)
    x **= 2.0

    output_ndarray_float_2(x)

def test_ndarray_matmul():
    x = np_identity(2)
    y = x @ np_ones([2, 2])

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_imatmul():
    x = np_identity(2)
    x @= np_ones([2, 2])

    output_ndarray_float_2(x)

def test_ndarray_pos():
    x_int32 = np_full([2, 2], -2)
    y_int32 = +x_int32

    output_ndarray_int32_2(x_int32)
    output_ndarray_int32_2(y_int32)

    x_float = np_full([2, 2], -2.0)
    y_float = +x_float

    output_ndarray_float_2(x_float)
    output_ndarray_float_2(y_float)

def test_ndarray_neg():
    x_int32 = np_full([2, 2], -2)
    y_int32 = -x_int32

    output_ndarray_int32_2(x_int32)
    output_ndarray_int32_2(y_int32)

    x_float = np_full([2, 2], 2.0)
    y_float = -x_float

    output_ndarray_float_2(x_float)
    output_ndarray_float_2(y_float)

def test_ndarray_inv():
    x_int32 = np_full([2, 2], -2)
    y_int32 = ~x_int32

    output_ndarray_int32_2(x_int32)
    output_ndarray_int32_2(y_int32)

    x_bool = np_full([2, 2], True)
    y_bool = ~x_bool

    output_ndarray_bool_2(x_bool)
    output_ndarray_bool_2(y_bool)

def test_ndarray_eq():
    x = np_identity(2)
    y = x == np_full([2, 2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_eq_broadcast():
    x = np_identity(2)
    y = x == np_full([2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_eq_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 0.0 == x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_eq_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x == 0.0

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ne():
    x = np_identity(2)
    y = x != np_full([2, 2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ne_broadcast():
    x = np_identity(2)
    y = x != np_full([2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ne_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 0.0 != x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ne_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x != 0.0

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_lt():
    x = np_identity(2)
    y = x < np_full([2, 2], 1.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_lt_broadcast():
    x = np_identity(2)
    y = x < np_full([2], 1.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_lt_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 1.0 < x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_lt_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x < 1.0

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_le():
    x = np_identity(2)
    y = x <= np_full([2, 2], 0.5)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_le_broadcast():
    x = np_identity(2)
    y = x <= np_full([2], 0.5)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_le_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 0.5 <= x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_le_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x <= 0.5

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_gt():
    x = np_identity(2)
    y = x > np_full([2, 2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_gt_broadcast():
    x = np_identity(2)
    y = x > np_full([2], 0.0)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_gt_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 0.0 > x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_gt_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x > 0.0

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ge():
    x = np_identity(2)
    y = x >= np_full([2, 2], 0.5)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ge_broadcast():
    x = np_identity(2)
    y = x >= np_full([2], 0.5)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ge_broadcast_lhs_scalar():
    x = np_identity(2)
    y = 0.5 >= x

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_ge_broadcast_rhs_scalar():
    x = np_identity(2)
    y = x >= 0.5

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_int32():
    x = np_identity(2)
    y = int32(x)

    output_ndarray_float_2(x)
    output_ndarray_int32_2(y)

def test_ndarray_int64():
    x = np_identity(2)
    y = int64(x)

    output_ndarray_float_2(x)
    output_ndarray_int64_2(y)

def test_ndarray_uint32():
    x = np_identity(2)
    y = uint32(x)

    output_ndarray_float_2(x)
    output_ndarray_uint32_2(y)

def test_ndarray_uint64():
    x = np_identity(2)
    y = uint64(x)

    output_ndarray_float_2(x)
    output_ndarray_uint64_2(y)

def test_ndarray_float():
    x = np_full([2, 2], 1)
    y = float(x)

    output_ndarray_int32_2(x)
    output_ndarray_float_2(y)

def test_ndarray_bool():
    x = np_identity(2)
    y = bool(x)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(y)

def test_ndarray_round():
    x = np_identity(2)
    xf32 = round(x)
    xf64 = round64(x)
    xff = np_round(x)

    output_ndarray_float_2(x)
    output_ndarray_int32_2(xf32)
    output_ndarray_int64_2(xf64)
    output_ndarray_float_2(xff)

def test_ndarray_floor():
    x = np_identity(2)
    xf32 = floor(x)
    xf64 = floor64(x)
    xff = np_floor(x)

    output_ndarray_float_2(x)
    output_ndarray_int32_2(xf32)
    output_ndarray_int64_2(xf64)
    output_ndarray_float_2(xff)

def test_ndarray_ceil():
    x = np_identity(2)
    xf32 = ceil(x)
    xf64 = ceil64(x)
    xff = np_ceil(x)

    output_ndarray_float_2(x)
    output_ndarray_int32_2(xf32)
    output_ndarray_int64_2(xf64)
    output_ndarray_float_2(xff)

def test_ndarray_min():
    x = np_identity(2)
    y = np_min(x)

    output_ndarray_float_2(x)
    output_float64(y)

def test_ndarray_max():
    x = np_identity(2)
    y = np_max(x)

    output_ndarray_float_2(x)
    output_float64(y)

def test_ndarray_abs():
    x = np_identity(2)
    y = abs(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_isnan():
    x = np_identity(2)
    x_isnan = np_isnan(x)
    y = np_full([2, 2], dbl_nan())
    y_isnan = np_isnan(y)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(x_isnan)
    output_ndarray_float_2(y)
    output_ndarray_bool_2(y_isnan)

def test_ndarray_isinf():
    x = np_identity(2)
    x_isinf = np_isinf(x)
    y = np_full([2, 2], dbl_inf())
    y_isinf = np_isinf(y)

    output_ndarray_float_2(x)
    output_ndarray_bool_2(x_isinf)
    output_ndarray_float_2(y)
    output_ndarray_bool_2(y_isinf)

def test_ndarray_sin():
    x = np_identity(2)
    y = np_sin(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_cos():
    x = np_identity(2)
    y = np_cos(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_exp():
    x = np_identity(2)
    y = np_exp(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_exp2():
    x = np_identity(2)
    y = np_exp2(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_log():
    x = np_identity(2)
    y = np_log(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_log10():
    x = np_identity(2)
    y = np_log10(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_log2():
    x = np_identity(2)
    y = np_log2(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_fabs():
    x = -np_identity(2)
    y = np_fabs(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_sqrt():
    x = np_identity(2)
    y = np_sqrt(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_rint():
    x = np_identity(2)
    y = np_rint(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_tan():
    x = np_identity(2)
    y = np_tan(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arcsin():
    x = np_identity(2)
    y = np_arcsin(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arccos():
    x = np_identity(2)
    y = np_arccos(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arctan():
    x = np_identity(2)
    y = np_arctan(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_sinh():
    x = np_identity(2)
    y = np_sinh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_cosh():
    x = np_identity(2)
    y = np_cosh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_tanh():
    x = np_identity(2)
    y = np_tanh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arcsinh():
    x = np_identity(2)
    y = np_arcsinh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arccosh():
    x = np_identity(2)
    y = np_arccosh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arctanh():
    x = np_identity(2)
    y = np_arctanh(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_expm1():
    x = np_identity(2)
    y = np_expm1(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_cbrt():
    x = np_identity(2)
    y = np_cbrt(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_erf():
    x = np_identity(2)
    y = sp_spec_erf(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_erfc():
    x = np_identity(2)
    y = sp_spec_erfc(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_gamma():
    x = np_identity(2)
    y = sp_spec_gamma(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_gammaln():
    x = np_identity(2)
    y = sp_spec_gammaln(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_j0():
    x = np_identity(2)
    y = sp_spec_j0(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_j1():
    x = np_identity(2)
    y = sp_spec_j1(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_arctan2():
    x = np_identity(2)
    zeros = np_zeros([2, 2])
    ones = np_ones([2, 2])
    atan2_x_zeros = np_arctan2(x, zeros)
    atan2_x_ones = np_arctan2(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(zeros)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(atan2_x_zeros)
    output_ndarray_float_2(atan2_x_ones)

def test_ndarray_arctan2_broadcast():
    x = np_identity(2)
    atan2_x_zeros = np_arctan2(x, np_zeros([2]))
    atan2_x_ones = np_arctan2(x, np_ones([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(atan2_x_zeros)
    output_ndarray_float_2(atan2_x_ones)

def test_ndarray_arctan2_broadcast_lhs_scalar():
    x = np_identity(2)
    atan2_x_zeros = np_arctan2(0.0, x)
    atan2_x_ones = np_arctan2(1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(atan2_x_zeros)
    output_ndarray_float_2(atan2_x_ones)

def test_ndarray_arctan2_broadcast_rhs_scalar():
    x = np_identity(2)
    atan2_x_zeros = np_arctan2(x, 0.0)
    atan2_x_ones = np_arctan2(x, 1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(atan2_x_zeros)
    output_ndarray_float_2(atan2_x_ones)

def test_ndarray_copysign():
    x = np_identity(2)
    ones = np_ones([2, 2])
    negones = np_full([2, 2], -1.0)
    copysign_x_ones = np_copysign(x, ones)
    copysign_x_negones = np_copysign(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(negones)
    output_ndarray_float_2(copysign_x_ones)
    output_ndarray_float_2(copysign_x_negones)

def test_ndarray_copysign_broadcast():
    x = np_identity(2)
    copysign_x_ones = np_copysign(x, np_ones([2]))
    copysign_x_negones = np_copysign(x, np_full([2], -1.0))

    output_ndarray_float_2(x)
    output_ndarray_float_2(copysign_x_ones)
    output_ndarray_float_2(copysign_x_negones)

def test_ndarray_copysign_broadcast_lhs_scalar():
    x = np_identity(2)
    copysign_x_ones = np_copysign(1.0, x)
    copysign_x_negones = np_copysign(-1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(copysign_x_ones)
    output_ndarray_float_2(copysign_x_negones)

def test_ndarray_copysign_broadcast_rhs_scalar():
    x = np_identity(2)
    copysign_x_ones = np_copysign(x, 1.0)
    copysign_x_negones = np_copysign(x, -1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(copysign_x_ones)
    output_ndarray_float_2(copysign_x_negones)

def test_ndarray_fmax():
    x = np_identity(2)
    ones = np_ones([2, 2])
    negones = np_full([2, 2], -1.0)
    fmax_x_ones = np_fmax(x, ones)
    fmax_x_negones = np_fmax(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(negones)
    output_ndarray_float_2(fmax_x_ones)
    output_ndarray_float_2(fmax_x_negones)

def test_ndarray_fmax_broadcast():
    x = np_identity(2)
    fmax_x_ones = np_fmax(x, np_ones([2]))
    fmax_x_negones = np_fmax(x, np_full([2], -1.0))

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmax_x_ones)
    output_ndarray_float_2(fmax_x_negones)

def test_ndarray_fmax_broadcast_lhs_scalar():
    x = np_identity(2)
    fmax_x_ones = np_fmax(1.0, x)
    fmax_x_negones = np_fmax(-1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmax_x_ones)
    output_ndarray_float_2(fmax_x_negones)

def test_ndarray_fmax_broadcast_rhs_scalar():
    x = np_identity(2)
    fmax_x_ones = np_fmax(x, 1.0)
    fmax_x_negones = np_fmax(x, -1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmax_x_ones)
    output_ndarray_float_2(fmax_x_negones)

def test_ndarray_fmin():
    x = np_identity(2)
    ones = np_ones([2, 2])
    negones = np_full([2, 2], -1.0)
    fmin_x_ones = np_fmin(x, ones)
    fmin_x_negones = np_fmin(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(negones)
    output_ndarray_float_2(fmin_x_ones)
    output_ndarray_float_2(fmin_x_negones)

def test_ndarray_fmin_broadcast():
    x = np_identity(2)
    fmin_x_ones = np_fmin(x, np_ones([2]))
    fmin_x_negones = np_fmin(x, np_full([2], -1.0))

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmin_x_ones)
    output_ndarray_float_2(fmin_x_negones)

def test_ndarray_fmin_broadcast_lhs_scalar():
    x = np_identity(2)
    fmin_x_ones = np_fmin(1.0, x)
    fmin_x_negones = np_fmin(-1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmin_x_ones)
    output_ndarray_float_2(fmin_x_negones)

def test_ndarray_fmin_broadcast_rhs_scalar():
    x = np_identity(2)
    fmin_x_ones = np_fmin(x, 1.0)
    fmin_x_negones = np_fmin(x, -1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(fmin_x_ones)
    output_ndarray_float_2(fmin_x_negones)

def test_ndarray_ldexp():
    x = np_identity(2)
    zeros = np_full([2, 2], 0)
    ones = np_full([2, 2], 1)
    ldexp_x_zeros = np_ldexp(x, zeros)
    ldexp_x_ones = np_ldexp(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_int32_2(zeros)
    output_ndarray_int32_2(ones)
    output_ndarray_float_2(ldexp_x_zeros)
    output_ndarray_float_2(ldexp_x_ones)

def test_ndarray_ldexp_broadcast():
    x = np_identity(2)
    ldexp_x_zeros = np_ldexp(x, np_full([2], 0))
    ldexp_x_ones = np_ldexp(x, np_full([2], 1))

    output_ndarray_float_2(x)
    output_ndarray_float_2(ldexp_x_zeros)
    output_ndarray_float_2(ldexp_x_ones)

def test_ndarray_ldexp_broadcast_lhs_scalar():
    x = int32(np_identity(2))
    ldexp_x_zeros = np_ldexp(0.0, x)
    ldexp_x_ones = np_ldexp(1.0, x)

    output_ndarray_int32_2(x)
    output_ndarray_float_2(ldexp_x_zeros)
    output_ndarray_float_2(ldexp_x_ones)

def test_ndarray_ldexp_broadcast_rhs_scalar():
    x = np_identity(2)
    ldexp_x_zeros = np_ldexp(x, 0)
    ldexp_x_ones = np_ldexp(x, 1)

    output_ndarray_float_2(x)
    output_ndarray_float_2(ldexp_x_zeros)
    output_ndarray_float_2(ldexp_x_ones)

def test_ndarray_hypot():
    x = np_identity(2)
    zeros = np_zeros([2, 2])
    ones = np_ones([2, 2])
    hypot_x_zeros = np_hypot(x, zeros)
    hypot_x_ones = np_hypot(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(zeros)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(hypot_x_zeros)
    output_ndarray_float_2(hypot_x_ones)

def test_ndarray_hypot_broadcast():
    x = np_identity(2)
    hypot_x_zeros = np_hypot(x, np_zeros([2]))
    hypot_x_ones = np_hypot(x, np_ones([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(hypot_x_zeros)
    output_ndarray_float_2(hypot_x_ones)

def test_ndarray_hypot_broadcast_lhs_scalar():
    x = np_identity(2)
    hypot_x_zeros = np_hypot(0.0, x)
    hypot_x_ones = np_hypot(1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(hypot_x_zeros)
    output_ndarray_float_2(hypot_x_ones)

def test_ndarray_hypot_broadcast_rhs_scalar():
    x = np_identity(2)
    hypot_x_zeros = np_hypot(x, 0.0)
    hypot_x_ones = np_hypot(x, 1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(hypot_x_zeros)
    output_ndarray_float_2(hypot_x_ones)

def test_ndarray_nextafter():
    x = np_identity(2)
    zeros = np_zeros([2, 2])
    ones = np_ones([2, 2])
    nextafter_x_zeros = np_nextafter(x, zeros)
    nextafter_x_ones = np_nextafter(x, ones)

    output_ndarray_float_2(x)
    output_ndarray_float_2(zeros)
    output_ndarray_float_2(ones)
    output_ndarray_float_2(nextafter_x_zeros)
    output_ndarray_float_2(nextafter_x_ones)

def test_ndarray_nextafter_broadcast():
    x = np_identity(2)
    nextafter_x_zeros = np_nextafter(x, np_zeros([2]))
    nextafter_x_ones = np_nextafter(x, np_ones([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(nextafter_x_zeros)
    output_ndarray_float_2(nextafter_x_ones)

def test_ndarray_nextafter_broadcast_lhs_scalar():
    x = np_identity(2)
    nextafter_x_zeros = np_nextafter(0.0, x)
    nextafter_x_ones = np_nextafter(1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(nextafter_x_zeros)
    output_ndarray_float_2(nextafter_x_ones)

def test_ndarray_nextafter_broadcast_rhs_scalar():
    x = np_identity(2)
    nextafter_x_zeros = np_nextafter(x, 0.0)
    nextafter_x_ones = np_nextafter(x, 1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(nextafter_x_zeros)
    output_ndarray_float_2(nextafter_x_ones)

def run() -> int32:
    test_ndarray_ctor()
    test_ndarray_empty()
    test_ndarray_zeros()
    test_ndarray_ones()
    test_ndarray_full()
    test_ndarray_eye()
    test_ndarray_identity()
    test_ndarray_fill()
    test_ndarray_copy()
    test_ndarray_neg_idx()
    test_ndarray_add()
    test_ndarray_add_broadcast()
    test_ndarray_add_broadcast_lhs_scalar()
    test_ndarray_add_broadcast_rhs_scalar()
    test_ndarray_iadd()
    test_ndarray_iadd_broadcast()
    test_ndarray_iadd_broadcast_scalar()
    test_ndarray_sub()
    test_ndarray_sub_broadcast()
    test_ndarray_sub_broadcast_lhs_scalar()
    test_ndarray_sub_broadcast_rhs_scalar()
    test_ndarray_isub()
    test_ndarray_isub_broadcast()
    test_ndarray_isub_broadcast_scalar()
    test_ndarray_mul()
    test_ndarray_mul_broadcast()
    test_ndarray_mul_broadcast_lhs_scalar()
    test_ndarray_mul_broadcast_rhs_scalar()
    test_ndarray_imul()
    test_ndarray_imul_broadcast()
    test_ndarray_imul_broadcast_scalar()
    test_ndarray_truediv()
    test_ndarray_truediv_broadcast()
    test_ndarray_truediv_broadcast_lhs_scalar()
    test_ndarray_truediv_broadcast_rhs_scalar()
    test_ndarray_itruediv()
    test_ndarray_itruediv_broadcast()
    test_ndarray_itruediv_broadcast_scalar()
    test_ndarray_floordiv()
    test_ndarray_floordiv_broadcast()
    test_ndarray_floordiv_broadcast_lhs_scalar()
    test_ndarray_floordiv_broadcast_rhs_scalar()
    test_ndarray_ifloordiv()
    test_ndarray_ifloordiv_broadcast()
    test_ndarray_ifloordiv_broadcast_scalar()
    test_ndarray_mod()
    test_ndarray_mod_broadcast()
    test_ndarray_mod_broadcast_lhs_scalar()
    test_ndarray_mod_broadcast_rhs_scalar()
    test_ndarray_imod()
    test_ndarray_imod_broadcast()
    test_ndarray_imod_broadcast_scalar()
    test_ndarray_pow()
    test_ndarray_pow_broadcast()
    test_ndarray_pow_broadcast_lhs_scalar()
    test_ndarray_pow_broadcast_rhs_scalar()
    test_ndarray_ipow()
    test_ndarray_ipow_broadcast()
    test_ndarray_ipow_broadcast_scalar()
    test_ndarray_matmul()
    test_ndarray_imatmul()
    test_ndarray_pos()
    test_ndarray_neg()
    test_ndarray_inv()
    test_ndarray_eq()
    test_ndarray_eq_broadcast()
    test_ndarray_eq_broadcast_lhs_scalar()
    test_ndarray_eq_broadcast_rhs_scalar()
    test_ndarray_ne()
    test_ndarray_ne_broadcast()
    test_ndarray_ne_broadcast_lhs_scalar()
    test_ndarray_ne_broadcast_rhs_scalar()
    test_ndarray_lt()
    test_ndarray_lt_broadcast()
    test_ndarray_lt_broadcast_lhs_scalar()
    test_ndarray_lt_broadcast_rhs_scalar()
    test_ndarray_lt()
    test_ndarray_le_broadcast()
    test_ndarray_le_broadcast_lhs_scalar()
    test_ndarray_le_broadcast_rhs_scalar()
    test_ndarray_gt()
    test_ndarray_gt_broadcast()
    test_ndarray_gt_broadcast_lhs_scalar()
    test_ndarray_gt_broadcast_rhs_scalar()
    test_ndarray_gt()
    test_ndarray_ge_broadcast()
    test_ndarray_ge_broadcast_lhs_scalar()
    test_ndarray_ge_broadcast_rhs_scalar()

    test_ndarray_int32()
    test_ndarray_int64()
    test_ndarray_uint32()
    test_ndarray_uint64()
    test_ndarray_float()
    test_ndarray_bool()

    test_ndarray_round()
    test_ndarray_floor()
    test_ndarray_min()
    test_ndarray_max()
    test_ndarray_abs()
    test_ndarray_isnan()
    test_ndarray_isinf()

    test_ndarray_sin()
    test_ndarray_cos()
    test_ndarray_exp()
    test_ndarray_exp2()
    test_ndarray_log()
    test_ndarray_log10()
    test_ndarray_log2()
    test_ndarray_fabs()
    test_ndarray_sqrt()
    test_ndarray_rint()
    test_ndarray_tan()
    test_ndarray_arcsin()
    test_ndarray_arccos()
    test_ndarray_arctan()
    test_ndarray_sinh()
    test_ndarray_cosh()
    test_ndarray_tanh()
    test_ndarray_arcsinh()
    test_ndarray_arccosh()
    test_ndarray_arctanh()
    test_ndarray_expm1()
    test_ndarray_cbrt()

    test_ndarray_erf()
    test_ndarray_erfc()
    test_ndarray_gamma()
    test_ndarray_gammaln()
    test_ndarray_j0()
    test_ndarray_j1()

    test_ndarray_arctan2()
    test_ndarray_arctan2_broadcast()
    test_ndarray_arctan2_broadcast_lhs_scalar()
    test_ndarray_arctan2_broadcast_rhs_scalar()
    test_ndarray_copysign()
    test_ndarray_copysign_broadcast()
    test_ndarray_copysign_broadcast_lhs_scalar()
    test_ndarray_copysign_broadcast_rhs_scalar()
    test_ndarray_fmax()
    test_ndarray_fmax_broadcast()
    test_ndarray_fmax_broadcast_lhs_scalar()
    test_ndarray_fmax_broadcast_rhs_scalar()
    test_ndarray_fmin()
    test_ndarray_fmin_broadcast()
    test_ndarray_fmin_broadcast_lhs_scalar()
    test_ndarray_fmin_broadcast_rhs_scalar()
    test_ndarray_ldexp()
    test_ndarray_ldexp_broadcast()
    test_ndarray_ldexp_broadcast_lhs_scalar()
    test_ndarray_ldexp_broadcast_rhs_scalar()
    test_ndarray_hypot()
    test_ndarray_hypot_broadcast()
    test_ndarray_hypot_broadcast_lhs_scalar()
    test_ndarray_hypot_broadcast_rhs_scalar()
    test_ndarray_nextafter()
    test_ndarray_nextafter_broadcast()
    test_ndarray_nextafter_broadcast_lhs_scalar()
    test_ndarray_nextafter_broadcast_rhs_scalar()

    return 0
