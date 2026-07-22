# EXPECT: __nac3_raise called. Exception details:
# EXPECT:           ID: 4
# EXPECT:     Location: irrt/irrt/ctrc/ctrc.hpp:LINE:COL
# EXPECT:     Function: __nac3_alloc
# EXPECT:      Message: "Failed to allocate {0} bytes"
# EXPECT:       Params: {0}=136, {1}=0, {2}=0
#
# An object too large for a single slab cell must raise `MemoryError` on allocation
# inside a `critical` region.
#
# Slab cells are a fixed `CTRC_CELL_SIZE` (128 bytes), which bounds allocation to O(1);
# an object that does not fit cannot be served no matter how many pages are reserved,
# so `critical(1)` versus any larger reservation makes no difference here. `Big` is 16
# `float64` fields plus the 8-byte `ObjectHeader` = 136 bytes, which also keeps the
# reported size identical on 32-bit and 64-bit targets (a `list` would not, since its
# header embeds a pointer-sized length).


@extern
def output_float64(x: float):
    ...


class Big:
    f0: float
    f1: float
    f2: float
    f3: float
    f4: float
    f5: float
    f6: float
    f7: float
    f8: float
    f9: float
    f10: float
    f11: float
    f12: float
    f13: float
    f14: float
    f15: float

    def __init__(self, v: float):
        self.f0 = v
        self.f1 = v
        self.f2 = v
        self.f3 = v
        self.f4 = v
        self.f5 = v
        self.f6 = v
        self.f7 = v
        self.f8 = v
        self.f9 = v
        self.f10 = v
        self.f11 = v
        self.f12 = v
        self.f13 = v
        self.f14 = v
        self.f15 = v


def run() -> int32:
    with critical(1):
        big = Big(1.5)
        output_float64(big.f0)

    return 0
