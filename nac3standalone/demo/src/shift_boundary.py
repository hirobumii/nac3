@extern
def output_int32(x: int32):
    ...
@extern
def output_uint32(x: uint32):
    ...
@extern
def output_int64(x: int64):
    ...
@extern
def output_uint64(x: uint64):
    ...

# Shift helpers take the count as a parameter so the saturating logic is exercised
# at runtime (not constant-folded). Bitshift counts follow NumPy's fixed-width
# semantics: a count that is negative or `>= bit_width` shifts every bit out.

def lshift_i32(x: int32, n: int32) -> int32:
    return x << n
def rshift_i32(x: int32, n: int32) -> int32:
    return x >> n
def lshift_u32(x: uint32, n: uint32) -> uint32:
    return x << n
def rshift_u32(x: uint32, n: uint32) -> uint32:
    return x >> n
def lshift_i64(x: int64, n: int64) -> int64:
    return x << n
def rshift_i64(x: int64, n: int64) -> int64:
    return x >> n
def lshift_u64(x: uint64, n: uint64) -> uint64:
    return x << n
def rshift_u64(x: uint64, n: uint64) -> uint64:
    return x >> n

def test_i32():
    # in-range
    output_int32(lshift_i32(int32(1), int32(0)))
    output_int32(lshift_i32(int32(1), int32(31)))     # -> INT_MIN
    output_int32(rshift_i32(int32(-256), int32(2)))   # arithmetic -> -64
    # count == width and > width: flush out
    output_int32(lshift_i32(int32(1), int32(32)))     # -> 0
    output_int32(lshift_i32(int32(1), int32(40)))     # -> 0
    # arithmetic right shift out of range: sign fill
    output_int32(rshift_i32(int32(-1), int32(32)))    # -> -1
    output_int32(rshift_i32(int32(255), int32(32)))   # -> 0
    # negative count: saturates (NumPy does not raise)
    output_int32(lshift_i32(int32(1), int32(-1)))     # -> 0
    output_int32(rshift_i32(int32(-8), int32(-1)))    # -> -1

def test_u32():
    output_uint32(lshift_u32(uint32(1), uint32(0)))
    output_uint32(lshift_u32(uint32(1), uint32(31)))
    output_uint32(rshift_u32(uint32(0x80000000), uint32(4)))
    output_uint32(lshift_u32(uint32(1), uint32(32)))          # -> 0
    output_uint32(rshift_u32(uint32(0xFFFFFFFF), uint32(40))) # logical -> 0

def test_i64():
    output_int64(lshift_i64(int64(1), int64(0)))
    output_int64(lshift_i64(int64(1), int64(63)))     # -> INT64_MIN
    output_int64(rshift_i64(int64(-256), int64(2)))   # -> -64
    output_int64(lshift_i64(int64(1), int64(64)))     # -> 0
    output_int64(lshift_i64(int64(1), int64(65)))     # -> 0
    output_int64(rshift_i64(int64(-1), int64(64)))    # -> -1
    output_int64(lshift_i64(int64(1), int64(-1)))     # -> 0

def test_u64():
    output_uint64(lshift_u64(uint64(1), uint64(0)))
    output_uint64(lshift_u64(uint64(1), uint64(63)))
    output_uint64(lshift_u64(uint64(1), uint64(64)))          # -> 0
    output_uint64(rshift_u64(uint64(0xFFFFFFFFFFFFFFFF), uint64(70)))  # -> 0

def run() -> int32:
    test_i32()
    test_u32()
    test_i64()
    test_u64()
    return 0
