def run() -> int32:
    # Numeric Primitives
    b: bool = False
    i32: int32 = 0
    i64: int64 = int64(0)
    u32: uint32 = uint32(0)
    u64: uint64 = uint64(0)
    f64: float = 0.0

    # String
    s: str = ""

    # List
    l_i32: list[int32] = []
    l_f64: list[float] = []
    l_str: list[str] = []

    # Option
    o_some: Option[int32] = Some(0)
    o_none: Option[int32] = none

    # Tuple
    t_i32_i32: tuple[int32, int32] = (0, 0)
    t_i32_f64: tuple[int32, float] = (0, 0.0)

    return 0