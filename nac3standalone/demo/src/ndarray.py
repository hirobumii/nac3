@extern
def output_int32(x: int32):
    ...

@extern
def output_float64(x: float):
    ...

def consume_ndarray_1(n: ndarray[float, Literal[1]]):
    pass

def consume_ndarray_i32_1(n: ndarray[int32, Literal[1]]):
    pass

def consume_ndarray_2(n: ndarray[float, Literal[2]]):
    pass

def test_ndarray_ctor():
    n: ndarray[float, Literal[1]] = np_ndarray([1])
    consume_ndarray_1(n)

def test_ndarray_empty():
    n: ndarray[float, 1] = np_empty([1])
    consume_ndarray_1(n)

def test_ndarray_zeros():
    n: ndarray[float, 1] = np_zeros([1])
    output_float64(n[0])
    consume_ndarray_1(n)

def test_ndarray_ones():
    n: ndarray[float, 1] = np_ones([1])
    output_float64(n[0])
    consume_ndarray_1(n)

def test_ndarray_full():
    n_float: ndarray[float, 1] = np_full([1], 2.0)
    output_float64(n_float[0])
    consume_ndarray_1(n_float)
    n_i32: ndarray[int32, 1] = np_full([1], 2)
    output_int32(n_i32[0])
    consume_ndarray_i32_1(n_i32)

def test_ndarray_eye():
    n: ndarray[float, 2] = np_eye(2)
    n0: ndarray[float, 1] = n[0]
    v: float = n0[0]
    output_float64(v)
    consume_ndarray_2(n)

def test_ndarray_identity():
    n: ndarray[float, 2] = np_identity(2)
    consume_ndarray_2(n)

def test_ndarray_fill():
    n: ndarray[float, 2] = np_empty([2, 2])
    n.fill(1.0)
    output_float64(n[0][0])
    output_float64(n[0][1])
    output_float64(n[1][0])
    output_float64(n[1][1])

def run() -> int32:
    test_ndarray_ctor()
    test_ndarray_empty()
    test_ndarray_zeros()
    test_ndarray_ones()
    test_ndarray_full()
    test_ndarray_eye()
    test_ndarray_identity()
    test_ndarray_fill()

    return 0
