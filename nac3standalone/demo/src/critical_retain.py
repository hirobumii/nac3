@extern
def output_int32(x: int32):
    ...

@extern
def output_strln(x: str):
    ...


class Point:
    x: int32
    y: int32

    def __init__(self, x: int32, y: int32):
        self.x = x
        self.y = y


def sum_points(points: list[Point]) -> int32:
    total = 0
    for i in range(len(points)):
        total += points[i].x + points[i].y
    return total


def test_retain_across_regions():
    # `with critical(1)` guarantees one page worth of FREE cells on every entry, rather than merely
    # that the pool has ever grown to one page. `retained` holds 20 of the first region's cells
    # alive past its end, leaving 11 of the page's 31 cells free, so the second region can only get
    # its 31 cells if the pool is grown again.
    #
    # 31 is `CTRC_CELLS_PER_PAGE` (nac3core/irrt/irrt/ctrc/page.hpp); the counts here are chosen
    # against it, so this test would stop being a regression test if that constant changed.
    output_strln("=== test_retain_across_regions ===")

    retained = [Point(0, 0) for _ in range(20)]
    second = [Point(0, 0) for _ in range(31)]

    with critical(1):
        for i in range(20):
            retained[i] = Point(i, i + 1)

    output_int32(sum_points(retained))

    with critical(1):
        for i in range(31):
            second[i] = Point(i, i * 2)

    output_int32(sum_points(second))

    # the first region's cells must have survived the second region untouched
    output_int32(sum_points(retained))


def test_zero_pages():
    # `critical(0)` allocates nothing at entry and runs on whatever capacity is already free - here
    # the capacity guaranteed by the enclosing region, which the inner one must not need to grow
    output_strln("=== test_zero_pages ===")

    with critical(1):
        with critical(0):
            total = 0
            for i in range(20):
                p = Point(i, i + 1)
                total += p.x + p.y
            output_int32(total)


def run() -> int32:
    test_retain_across_regions()
    test_zero_pages()
    return 0
