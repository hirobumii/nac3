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

@extern
def output_range(x: range):
    ...

@extern
def output_int32_list(x: list[int32]):
    ...

@extern
def output_asciiart(x: int32):
    ...

@extern
def output_str(x: str):
    ...

@extern
def output_strln(x: str):
    ...

def test_output_bool():
    output_bool(True)
    output_bool(False)

def test_output_int32():
    output_int32(-128)

def test_output_int64():
    output_int64(int64(-256))

def test_output_uint32():
    output_uint32(uint32(128))

def test_output_uint64():
    output_uint64(uint64(256))

def test_output_float64():
    output_float64(0.0)
    output_float64(1.0)
    output_float64(-1.0)
    output_float64(128.0)
    output_float64(-128.0)
    output_float64(16.25)
    output_float64(-16.25)

def test_output_range():
    r = range(1, 100, 5)
    output_int32(r.start)
    output_int32(r.stop)
    output_int32(r.step)
    output_range(range(10))
    output_range(range(1, 10))
    output_range(range(1, 10, 2))

def test_output_asciiart():
    for i in range(17):
        output_asciiart(i)
    output_asciiart(0)

def test_output_int32_list():
    output_int32_list([0, 1, 3, 5, 10])

def test_output_str_family():
    output_str("hello")
    output_strln(" world")

def run() -> int32:
    test_output_bool()
    test_output_int32()
    test_output_int64()
    test_output_uint32()
    test_output_uint64()
    test_output_float64()
    test_output_range()
    test_output_asciiart()
    test_output_int32_list()
    test_output_str_family()
    return 0