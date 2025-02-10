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

def output_ndarray_float_3(n: ndarray[float, Literal[3]]):
    for d in range(len(n)):
        for r in range(len(n[d])):
            for c in range(len(n[d][r])):
                output_float64(n[d][r][c])

def output_ndarray_float_4(n: ndarray[float, Literal[4]]):
    for x in range(len(n)):
        for y in range(len(n[x])):
            for z in range(len(n[x][y])):
                for w in range(len(n[x][y][z])):
                    output_float64(n[x][y][z][w])

def consume_ndarray_1(n: ndarray[float, Literal[1]]):
    pass

def consume_ndarray_2(n: ndarray[float, Literal[2]]):
    pass

def test_ndarray_ctor():
    n: ndarray[float, Literal[1]] = np_ndarray([1])
    consume_ndarray_1(n)

def test_ndarray_empty():
    n1: ndarray[float, 1] = np_empty([1])
    consume_ndarray_1(n1)

    n2: ndarray[float, 1] = np_empty(10)
    consume_ndarray_1(n2)

    n3: ndarray[float, 1] = np_empty((2,))
    consume_ndarray_1(n3)

    n4: ndarray[float, 2] = np_empty((4, 4))
    consume_ndarray_2(n4)

    dim4 = (5, 2)
    n5: ndarray[float, 2] = np_empty(dim4)
    consume_ndarray_2(n5)

def test_ndarray_zeros():
    n1: ndarray[float, 1] = np_zeros([1])
    output_ndarray_float_1(n1)

    k = 3 + int32(n1[0]) # to test variable shape inputs
    n2: ndarray[float, 1] = np_zeros(k * k)
    output_ndarray_float_1(n2)

    n3: ndarray[float, 1] = np_zeros((k * 2,))
    output_ndarray_float_1(n3)

    dim4 = (3, 2 * k)
    n4: ndarray[float, 2] = np_zeros(dim4)
    output_ndarray_float_2(n4)

def test_ndarray_ones():
    n: ndarray[float, 1] = np_ones([1])
    output_ndarray_float_1(n)

    dim = (1,)
    n_tup: ndarray[float, 1] = np_ones(dim)
    output_ndarray_float_1(n_tup)

def test_ndarray_full():
    n_float: ndarray[float, 1] = np_full([1], 2.0)
    output_ndarray_float_1(n_float)
    n_i32: ndarray[int32, 1] = np_full([1], 2)
    output_ndarray_int32_1(n_i32)

    dim = (1,)
    n_float_tup: ndarray[float, 1] = np_full(dim, 2.0)
    output_ndarray_float_1(n_float_tup)
    n_i32_tup: ndarray[int32, 1] = np_full(dim, 2)
    output_ndarray_int32_1(n_i32_tup)

def test_ndarray_eye():
    n: ndarray[float, 2] = np_eye(2)
    output_ndarray_float_2(n)

def test_ndarray_array():
    n1: ndarray[float, 1] = np_array([1.0, 2.0, 3.0])
    output_ndarray_float_1(n1)
    n1to2: ndarray[float, 2] = np_array([1.0, 2.0, 3.0], ndmin=2)
    output_ndarray_float_2(n1to2)
    n2: ndarray[float, 2] = np_array([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])
    output_ndarray_float_2(n2)

    # Copy
    n2_cpy: ndarray[float, 2] = np_array(n2, copy=False)
    output_ndarray_float_2(n2_cpy)
    n2_cpy.fill(0.0)
    output_ndarray_float_2(n2_cpy)

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

def test_ndarray_slices():
    x = np_identity(3)
    output_ndarray_float_2(x)

    x_identity = x[::]
    output_ndarray_float_2(x_identity)

    x02 = x[0::2]
    output_ndarray_float_2(x02)

    x_mirror = x[::-1]
    output_ndarray_float_2(x_mirror)

    x2 = x[0::2, 0::2]
    output_ndarray_float_2(x2)

def test_ndarray_nd_idx():
    x = np_identity(2)

    x0: float = x[0, 0]
    output_float64(x0)
    output_float64(x[0, 1])
    output_float64(x[1, 0])
    output_float64(x[1, 1])

def test_ndarray_transpose():
    x: ndarray[float, 2] = np_array([[1., 2., 3.], [4., 5., 6.]])
    y = np_transpose(x)
    z = np_transpose(y)

    output_int32(np_shape(x)[0])
    output_int32(np_shape(x)[1])
    output_ndarray_float_2(x)

    output_int32(np_shape(y)[0])
    output_int32(np_shape(y)[1])
    output_ndarray_float_2(y)

    output_int32(np_shape(z)[0])
    output_int32(np_shape(z)[1])
    output_ndarray_float_2(z)

def test_ndarray_reshape():
    w: ndarray[float, 1] = np_array([1., 2., 3., 4., 5., 6., 7., 8., 9., 10.])
    x = np_reshape(w, (1, 2, 1, -1))
    y = np_reshape(x, [2, -1])
    z = np_reshape(y, 10)

    output_int32(np_shape(w)[0])
    output_ndarray_float_1(w)

    output_int32(np_shape(x)[0])
    output_int32(np_shape(x)[1])
    output_int32(np_shape(x)[2])
    output_int32(np_shape(x)[3])
    output_ndarray_float_4(x)

    output_int32(np_shape(y)[0])
    output_int32(np_shape(y)[1])
    output_ndarray_float_2(y)

    output_int32(np_shape(z)[0])
    output_ndarray_float_1(z)

    x1: ndarray[int32, 1] = np_array([1, 2, 3, 4])
    x2: ndarray[int32, 2] = np_reshape(x1, (2, 2))

    output_int32(np_shape(x1)[0])
    output_ndarray_int32_1(x1)

    output_int32(np_shape(x2)[0])
    output_int32(np_shape(x2)[1])
    output_ndarray_int32_2(x2)

def test_ndarray_broadcast_to():
    xs = np_array([1.0, 2.0, 3.0])
    ys = np_broadcast_to(xs, (1, 3))
    zs = np_broadcast_to(ys, (2, 4, 3))

    output_int32(np_shape(xs)[0])
    output_ndarray_float_1(xs)

    output_int32(np_shape(ys)[0])
    output_int32(np_shape(ys)[1])
    output_ndarray_float_2(ys)

    output_int32(np_shape(zs)[0])
    output_int32(np_shape(zs)[1])
    output_int32(np_shape(zs)[2])
    output_ndarray_float_3(zs)

def test_ndarray_subscript_assignment():
    xs = np_array([[11.0, 22.0, 33.0, 44.0], [55.0, 66.0, 77.0, 88.0]])

    xs[0, 0] = 99.0
    output_ndarray_float_2(xs)

    xs[0] = 100.0
    output_ndarray_float_2(xs)

    xs[:, ::2] = 101.0
    output_ndarray_float_2(xs)

    xs[1:, 0] = 102.0
    output_ndarray_float_2(xs)

    xs[0] = np_array([-1.0, -2.0, -3.0, -4.0])
    output_ndarray_float_2(xs)

    xs[:] = np_array([-5.0, -6.0, -7.0, -8.0])
    output_ndarray_float_2(xs)

    # Test assignment with memory sharing
    ys1 = np_reshape(xs, (2, 4))
    ys2 = np_transpose(ys1)
    ys3 = ys2[::-1, 0]
    ys3[0] = -999.0

    output_ndarray_float_2(xs)
    output_ndarray_float_2(ys1)
    output_ndarray_float_2(ys2)
    output_ndarray_float_1(ys3)

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

def test_ndarray_minimum():
    x = np_identity(2)
    min_x_zeros = np_minimum(x, np_zeros([2]))
    min_x_ones = np_minimum(x, np_zeros([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(min_x_zeros)
    output_ndarray_float_2(min_x_ones)

def test_ndarray_minimum_broadcast():
    x = np_identity(2)
    min_x_zeros = np_minimum(x, np_zeros([2]))
    min_x_ones = np_minimum(x, np_zeros([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(min_x_zeros)
    output_ndarray_float_2(min_x_ones)

def test_ndarray_minimum_broadcast_lhs_scalar():
    x = np_identity(2)
    min_x_zeros = np_minimum(0.0, x)
    min_x_ones = np_minimum(1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(min_x_zeros)
    output_ndarray_float_2(min_x_ones)

def test_ndarray_minimum_broadcast_rhs_scalar():
    x = np_identity(2)
    min_x_zeros = np_minimum(x, 0.0)
    min_x_ones = np_minimum(x, 1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(min_x_zeros)
    output_ndarray_float_2(min_x_ones)

def test_ndarray_argmin():
    x = np_array([[1., 2.], [3., 4.]])
    y = np_argmin(x)

    output_ndarray_float_2(x)
    output_int64(y)

def test_ndarray_max():
    x = np_identity(2)
    y = np_max(x)

    output_ndarray_float_2(x)
    output_float64(y)

def test_ndarray_maximum():
    x = np_identity(2)
    max_x_zeros = np_maximum(x, np_zeros([2]))
    max_x_ones = np_maximum(x, np_zeros([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(max_x_zeros)
    output_ndarray_float_2(max_x_ones)

def test_ndarray_maximum_broadcast():
    x = np_identity(2)
    max_x_zeros = np_maximum(x, np_zeros([2]))
    max_x_ones = np_maximum(x, np_zeros([2]))

    output_ndarray_float_2(x)
    output_ndarray_float_2(max_x_zeros)
    output_ndarray_float_2(max_x_ones)

def test_ndarray_maximum_broadcast_lhs_scalar():
    x = np_identity(2)
    max_x_zeros = np_maximum(0.0, x)
    max_x_ones = np_maximum(1.0, x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(max_x_zeros)
    output_ndarray_float_2(max_x_ones)

def test_ndarray_maximum_broadcast_rhs_scalar():
    x = np_identity(2)
    max_x_zeros = np_maximum(x, 0.0)
    max_x_ones = np_maximum(x, 1.0)

    output_ndarray_float_2(x)
    output_ndarray_float_2(max_x_zeros)
    output_ndarray_float_2(max_x_ones)

def test_ndarray_argmax():
    x = np_array([[1., 2.], [3., 4.]])
    y = np_argmax(x)

    output_ndarray_float_2(x)
    output_int64(y)

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


def test_ndarray_any():
    s0 = 0
    output_bool(np_any(s0))
    s1 = 1
    output_bool(np_any(s1))

    x1 = np_identity(5)
    y1 = np_any(x1)
    output_ndarray_float_2(x1)
    output_bool(y1)

    x2 = np_identity(1)
    y2 = np_any(x2)
    output_ndarray_float_2(x2)
    output_bool(y2)

    x3 = np_array([[1.0, 2.0], [3.0, 4.0]])
    y3 = np_any(x3)
    output_ndarray_float_2(x3)
    output_bool(y3)

    x4 = np_zeros([3, 5])
    y4 = np_any(x4)
    output_ndarray_float_2(x4)
    output_bool(y4)

def test_ndarray_all():
    s0 = 0
    output_bool(np_all(s0))
    s1 = 1
    output_bool(np_all(s1))

    x1 = np_identity(5)
    y1 = np_all(x1)
    output_ndarray_float_2(x1)
    output_bool(y1)

    x2 = np_identity(1)
    y2 = np_all(x2)
    output_ndarray_float_2(x2)
    output_bool(y2)

    x3 = np_array([[1.0, 2.0], [3.0, 4.0]])
    y3 = np_all(x3)
    output_ndarray_float_2(x3)
    output_bool(y3)

    x4 = np_zeros([3, 5])
    y4 = np_all(x4)
    output_ndarray_float_2(x4)
    output_bool(y4)

def test_ndarray_dot():
    x1: ndarray[float, 1] = np_array([5.0, 1.0, 4.0, 2.0])
    y1: ndarray[float, 1] = np_array([5.0, 1.0, 6.0, 6.0])
    z1 = np_dot(x1, y1)

    x2: ndarray[int32, 1] = np_array([5, 1, 4, 2])
    y2: ndarray[int32, 1] = np_array([5, 1, 6, 6])
    z2 = np_dot(x2, y2)

    x3: ndarray[bool, 1] = np_array([True, True, True, True])
    y3: ndarray[bool, 1] = np_array([True, True, True, True])
    z3 = np_dot(x3, y3)

    z4 = np_dot(2, 3)
    z5 = np_dot(2., 3.)
    z6 = np_dot(True, False)

    output_float64(z1)
    output_int32(z2)
    output_bool(z3)
    output_int32(z4)
    output_float64(z5)
    output_bool(z6)

def test_ndarray_cholesky():
    x: ndarray[float, 2] = np_array([[5.0, 1.0], [1.0, 4.0]])
    y = np_linalg_cholesky(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_qr():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    y, z  = np_linalg_qr(x)

    output_ndarray_float_2(x)

    # QR Factorization is not unique and gives different results in numpy and nalgebra
    # Reverting the decomposition to compare the initial arrays
    a = y @ z
    output_ndarray_float_2(a)

def test_ndarray_linalg_inv():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    y = np_linalg_inv(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_pinv():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5]])
    y = np_linalg_pinv(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_matrix_power():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    y = np_linalg_matrix_power(x, -9)

    output_ndarray_float_2(x)
    output_ndarray_float_2(y)

def test_ndarray_det():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    y = np_linalg_det(x)

    output_ndarray_float_2(x)
    output_float64(y)

def test_ndarray_schur():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    t, z = sp_linalg_schur(x)

    output_ndarray_float_2(x)

    # Schur Factorization is not unique and gives different results in scipy and nalgebra
    # Reverting the decomposition to compare the initial arrays
    a = (z @ t) @ np_linalg_inv(z)
    output_ndarray_float_2(a)

def test_ndarray_hessenberg():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 5.0, 8.5]])
    h, q = sp_linalg_hessenberg(x)

    output_ndarray_float_2(x)

    # Hessenberg Factorization is not unique and gives different results in scipy and nalgebra
    # Reverting the decomposition to compare the initial arrays
    a = (q @ h) @ np_linalg_inv(q)
    output_ndarray_float_2(a)


def test_ndarray_lu():
    x: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5]])
    l, u = sp_linalg_lu(x)

    output_ndarray_float_2(x)
    output_ndarray_float_2(l)
    output_ndarray_float_2(u)


def test_ndarray_svd():
    w: ndarray[float, 2] = np_array([[-5.0, -1.0, 2.0], [-1.0, 4.0, 7.5], [-1.0, 8.0, -8.5]])
    x, y, z = np_linalg_svd(w)

    output_ndarray_float_2(w)

    # SVD Factorization is not unique and gives different results in numpy and nalgebra
    # Reverting the decomposition to compare the initial arrays
    a = x @ z
    output_ndarray_float_2(a)
    output_ndarray_float_1(y)


def run() -> int32:
    test_ndarray_ctor()
    test_ndarray_empty()
    test_ndarray_zeros()
    test_ndarray_ones()
    test_ndarray_full()
    test_ndarray_eye()
    test_ndarray_array()
    test_ndarray_identity()
    test_ndarray_fill()
    test_ndarray_copy()

    test_ndarray_neg_idx()
    test_ndarray_slices()
    test_ndarray_nd_idx()

    test_ndarray_transpose()
    test_ndarray_reshape()
    test_ndarray_broadcast_to()
    test_ndarray_subscript_assignment()

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
    test_ndarray_ceil()
    test_ndarray_min()
    test_ndarray_minimum()
    test_ndarray_minimum_broadcast()
    test_ndarray_minimum_broadcast_lhs_scalar()
    test_ndarray_minimum_broadcast_rhs_scalar()
    test_ndarray_argmin()
    test_ndarray_max()
    test_ndarray_maximum()
    test_ndarray_maximum_broadcast()
    test_ndarray_maximum_broadcast_lhs_scalar()
    test_ndarray_maximum_broadcast_rhs_scalar()
    test_ndarray_argmax()
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

    test_ndarray_any()
    test_ndarray_all()

    test_ndarray_dot()
    test_ndarray_cholesky()
    test_ndarray_qr()
    test_ndarray_svd()
    test_ndarray_linalg_inv()
    test_ndarray_pinv()
    test_ndarray_matrix_power()
    test_ndarray_det()
    test_ndarray_lu()
    test_ndarray_schur()
    test_ndarray_hessenberg()
    return 0
