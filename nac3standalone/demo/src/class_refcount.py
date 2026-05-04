T = TypeVar("T")

@extern
def output_int32(x: int32):
    ...

@extern
def output_refcount(x: T):
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


class Container:
    items: list[int32]
    point: Point

    def __init__(self, items: list[int32], point: Point):
        self.items = items
        self.point = point


def test_class_basic():
    output_strln("=== test_class_basic ===")
    p = Point(1, 2)
    output_int32(p.x)
    output_int32(p.y)
    output_refcount(p)


def test_class_aliasing():
    output_strln("=== test_class_aliasing ===")
    p1 = Point(10, 20)
    output_refcount(p1)

    p2 = p1
    output_refcount(p1)
    output_refcount(p2)

    p1 = Point(30, 40)
    output_int32(p1.x)
    output_int32(p2.x)
    output_refcount(p1)
    output_refcount(p2)


def test_class_with_refcounted_fields():
    output_strln("=== test_class_with_refcounted_fields ===")
    items = [1, 2, 3]
    p = Point(42, 99)
    c = Container(items, p)
    output_int32(c.point.x)
    output_int32(c.items[0])
    output_int32(c.items[2])
    output_refcount(c)
    output_refcount(c.point)
    output_refcount(c.items)


def test_class_reassignment():
    output_strln("=== test_class_reassignment ===")
    p = Point(1, 2)
    output_refcount(p)
    p = Point(3, 4)
    output_refcount(p)
    output_int32(p.x)


def test_class_in_function():
    output_strln("=== test_class_in_function ===")
    p = Point(7, 8)
    output_int32(use_point(p))
    output_refcount(p)


def use_point(p: Point) -> int32:
    output_refcount(p)
    return p.x + p.y


def run() -> int32:
    test_class_basic()
    test_class_aliasing()
    test_class_with_refcounted_fields()
    test_class_reassignment()
    test_class_in_function()
    return 0
