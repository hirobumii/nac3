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

def u32_min() -> uint32:
    return uint32(0)

def u32_max() -> uint32:
    return ~uint32(0)

def i32_min() -> int32:
    return int32(1 << 31)

def i32_max() -> int32:
    return int32(~(1 << 31))

def u64_min() -> uint64:
    return uint64(0)

def u64_max() -> uint64:
    return ~uint64(0)

def i64_min() -> int64:
    return int64(1) << 63

def i64_max() -> int64:
    return ~(int64(1) << 63)

def test_u32_bnot():
    output_uint32(~uint32(0))

def test_u64_bnot():
    output_uint64(~uint64(0))

def test_conv_from_i32():
    for x in [
        i32_min(),
        i32_min() + 1,
        -1,
        0,
        1,
        i32_max() - 1,
        i32_max()
    ]:
        output_int64(int64(x))
        output_uint32(uint32(x))
        output_uint64(uint64(x))
        output_float64(float(x))

def test_conv_from_u32():
    for x in [
        u32_min(),
        u32_min() + uint32(1),
        u32_max() - uint32(1),
        u32_max()
    ]:
        output_uint64(uint64(x))
        output_int32(int32(x))
        output_int64(int64(x))
        output_float64(float(x))

def test_conv_from_i64():
    for x in [
        i64_min(),
        i64_min() + int64(1),
        int64(-1),
        int64(0),
        int64(1),
        i64_max() - int64(1),
        i64_max()
    ]:
        output_int32(int32(x))
        output_uint64(uint64(x))
        output_uint32(uint32(x))
        output_float64(float(x))

def test_conv_from_u64():
    for x in [
        u64_min(),
        u64_min() + uint64(1),
        u64_max() - uint64(1),
        u64_max()
    ]:
        output_uint32(uint32(x))
        output_int64(int64(x))
        output_int32(int32(x))
        output_float64(float(x))

def test_f64toi32():
    for x in [
        float(i32_min()) - 1.0,
        float(i32_min()),
        float(i32_min()) + 1.0,
        -1.5,
        -0.5,
        0.5,
        1.5,
        float(i32_max()) - 1.0,
        float(i32_max()),
        float(i32_max()) + 1.0
    ]:
        output_int32(int32(x))

def test_f64toi64():
    for x in [
        float(i64_min()),
        float(i64_min()) + 1.0,
        -1.5,
        -0.5,
        0.5,
        1.5,
        # 2^53 is the highest integral power-of-two of which uint64 and float have a one-to-one correspondence
        float(uint64(2) ** uint64(52)) - 1.0,
        float(uint64(2) ** uint64(52)),
        float(uint64(2) ** uint64(52)) + 1.0,
    ]:
        output_int64(int64(x))

def test_f64tou32():
    for x in [
        -1.5,
        float(u32_min()) - 1.0,
        -0.5,
        float(u32_min()),
        0.5,
        float(u32_min()) + 1.0,
        1.5,
        float(u32_max()) - 1.0,
        float(u32_max()),
        float(u32_max()) + 1.0
    ]:
        output_uint32(uint32(x))

def test_f64tou64():
    for x in [
        -1.5,
        float(u64_min()) - 1.0,
        -0.5,
        float(u64_min()),
        0.5,
        float(u64_min()) + 1.0,
        1.5,
        # 2^53 is the highest integral power-of-two of which uint64 and float have a one-to-one correspondence
        float(uint64(2) ** uint64(52)) - 1.0,
        float(uint64(2) ** uint64(52)),
        float(uint64(2) ** uint64(52)) + 1.0,
    ]:
        output_uint64(uint64(x))

def run() -> int32:
    test_u32_bnot()
    test_u64_bnot()

    test_conv_from_i32()
    test_conv_from_u32()
    test_conv_from_i64()
    test_conv_from_u64()

    test_f64toi32()
    test_f64toi64()
    test_f64tou32()
    test_f64tou64()

    return 0