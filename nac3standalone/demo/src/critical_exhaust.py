# EXPECT: __nac3_raise called. Exception details:
# EXPECT:           ID: 4
# EXPECT:     Location: irrt/irrt/ctrc/ctrc.hpp:LINE:COL
# EXPECT:     Function: __nac3_alloc
# EXPECT:      Message: "Failed to allocate {0} bytes"
# EXPECT:       Params: {0}=16, {1}=0, {2}=0
#
# Exhausting the reserved working set of a `critical` region must raise `MemoryError`
# rather than returning null or over-allocating.
#
# `critical(1)` reserves one page worth of free cells (`CTRC_CELLS_PER_PAGE` = 31), so
# the 32nd allocation has nowhere to come from: the slab deliberately does *not* grow
# inside a region, since unbounded allocation there would defeat the latency guarantee.
# `critical_capacity.py` is the companion demo that stops at exactly 31 and succeeds.


@extern
def output_int32(x: int32):
    ...


class Point:
    x: int32
    y: int32

    def __init__(self, x: int32, y: int32):
        self.x = x
        self.y = y


def run() -> int32:
    acc = 0
    i = 0
    with critical(1):
        while i < 32:
            p = Point(i, i)
            acc += p.x
            i += 1

    output_int32(acc)
    return 0
