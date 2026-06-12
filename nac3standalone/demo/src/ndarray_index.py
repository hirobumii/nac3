@extern
def output_int32(x: int32):
    ...

@extern
def output_float64(x: float):
    ...

@extern
def output_bool(x: bool):
    ...


def test_1d_read():
    a = np_array([10, 20, 30, 40, 50])
    output_int32(a[0])
    output_int32(a[4])
    # negative indices
    output_int32(a[-1])
    output_int32(a[-5])

def test_1d_write():
    a = np_array([0, 0, 0, 0, 0])
    a[0] = 11
    a[2] = 33
    a[-1] = 99
    for i in range(5):
        output_int32(a[i])

def test_2d_comma_read():
    a = np_array([[1, 2, 3], [4, 5, 6]])
    output_int32(a[0, 0])
    output_int32(a[1, 2])
    output_int32(a[-1, -1])

def test_2d_comma_write():
    a = np_array([[0, 0, 0], [0, 0, 0]])
    a[0, 0] = 7
    a[1, 2] = 8
    a[-1, -2] = 9
    for i in range(2):
        for j in range(3):
            output_int32(a[i, j])

def test_2d_nested_read():
    a = np_array([[1, 2, 3], [4, 5, 6]])
    output_int32(a[0][0])
    output_int32(a[1][2])
    output_int32(a[-1][-1])

def test_2d_nested_write():
    a = np_array([[0, 0, 0], [0, 0, 0]])
    line = np_array([[10, 20, 30]])
    for i in range(2):
        for j in range(3):
            a[i][j] = line[0][j]
    for i in range(2):
        for j in range(3):
            output_int32(a[i][j])

def test_3d_read():
    a = np_array([[[1, 2], [3, 4]], [[5, 6], [7, 8]]])
    output_int32(a[0, 0, 0])
    output_int32(a[1, 1, 1])
    output_int32(a[0][1][0])

def test_float_dtype():
    a = np_array([1.5, 2.5, 3.5])
    a[1] = 9.25
    output_float64(a[0])
    output_float64(a[1])
    output_float64(a[-1])


def test_bool_dtype():
    a = np_array([True, False, True])
    a[0] = 1 > 2     # comparison result (i1) stored into an i8 bool element
    a[1] = 2 > 1
    a[-1] = False
    for i in range(3):
        output_bool(a[i])


def run() -> int32:
    test_1d_read()
    test_1d_write()
    test_2d_comma_read()
    test_2d_comma_write()
    test_2d_nested_read()
    test_2d_nested_write()
    test_3d_read()
    test_float_dtype()
    test_bool_dtype()
    return 0
