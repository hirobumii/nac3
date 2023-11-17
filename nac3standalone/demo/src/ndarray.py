def consume_ndarray_1(n: ndarray[float, Literal[1]]):
    pass

def consume_ndarray_i32_1(n: ndarray[int32, Literal[1]]):
    pass

def consume_ndarray_2(n: ndarray[float, Literal[2]]):
    pass

def test_ndarray_ctor():
    n = np_ndarray([1])
    consume_ndarray_1(n)

def test_ndarray_empty():
    n = np_empty([1])
    consume_ndarray_1(n)

def run() -> int32:
    test_ndarray_ctor()
    test_ndarray_empty()

    return 0
