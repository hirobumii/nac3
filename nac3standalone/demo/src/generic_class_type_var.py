from __future__ import annotations

@extern
def output_int32(x: int32):
    ...


class Item1:
    value: int32

    def __init__(self, value: int32):
        self.value = value

    def get_value(self) -> int32:
        return self.value


class Item2:
    value: int32

    def __init__(self, value: int32):
        self.value = value

    def get_value(self) -> int32:
        return self.value


T = TypeVar("T", Item1, Item2)


class Container(Generic[T]):
    item: T

    def __init__(self, item: T):
        self.item = item

    def add(self, other: Container[T]) -> int32:
        """Using Container[T] as parameter type"""
        return self.item.get_value() + other.item.get_value()


def run() -> int32:
    c1 = Container(Item1(int32(10)))
    c2 = Container(Item1(int32(20)))
    result = c1.add(c2)
    output_int32(result)
    return int32(0)
