# `critical(1)` reserves one page worth of free cells (`CTRC_CELLS_PER_PAGE` = 31).
# Exactly that many slab allocations must succeed - this demo pins the lower edge of
# that boundary, and `critical_exhaust.py` pins the upper edge by going one past it.
# Together they catch an off-by-one in the free-cell accounting in either direction.
#
# A `while` loop is used rather than `for` because `range()` is itself a heap
# allocation, which inside the region would consume one of the 31 cells and blur the
# boundary being tested.


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
        while i < 31:
            p = Point(i, i)
            acc += p.x
            i += 1

    output_int32(acc)
    return 0
