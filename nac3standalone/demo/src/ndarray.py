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
    consume_ndarray_1(n)

def test_ndarray_ones():
    n: ndarray[float, 1] = np_ones([1])
    consume_ndarray_1(n)

def test_ndarray_full():
    n_float: ndarray[float, 1] = np_full([1], 2.0)
    consume_ndarray_1(n_float)
    n_i32: ndarray[int32, 1] = np_full([1], 2)
    consume_ndarray_i32_1(n_i32)

def test_ndarray_eye():
    n: ndarray[float, 2] = np_eye(2)
    consume_ndarray_2(n)

def test_ndarray_identity():
    n: ndarray[float, 2] = np_identity(2)
    consume_ndarray_2(n)

def run() -> int32:
    test_ndarray_ctor()
    test_ndarray_empty()
    test_ndarray_zeros()
    test_ndarray_ones()
    test_ndarray_full()
    test_ndarray_eye()
    test_ndarray_identity()

    return 0
