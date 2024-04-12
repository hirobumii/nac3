@extern
def output_bool(x: bool):
    ...

@extern
def output_int32(x: int32):
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

    return 0
