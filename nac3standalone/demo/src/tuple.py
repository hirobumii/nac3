@extern
def output_bool(b: bool):
    ...

@extern
def output_int32_list(x: list[int32]):
    ...

@extern
def output_int32(x: int32):
    ...

class A:
    a: int32
    b: bool
    def __init__(self, a: int32, b: bool):
        self.a = a
        self.b = b


def test_tuple_eq():
    # 0-len
    output_bool(() == ())
    # 1-len
    output_bool((1,) == ())
    output_bool(() == (1,))
    output_bool((1,) == (1,))
    output_bool((1,) == (2,))
    # # 2-len
    output_bool((1, 2) == ())
    output_bool(() == (1, 2))
    output_bool((1,) == (1, 2))
    output_bool((1, 2) == (1,))
    output_bool((2, 2) == (1, 2))
    output_bool((1, 2) == (2, 2))


def test_tuple_ne():
    # 0-len
    output_bool(() != ())
    # 1-len
    output_bool((1,) != ())
    output_bool(() != (1,))
    output_bool((1,) != (1,))
    output_bool((1,) != (2,))
    # 2-len
    output_bool((1, 2) != ())
    output_bool(() != (1, 2))
    output_bool((1,) != (1, 2))
    output_bool((1, 2) != (1,))
    output_bool((2, 2) != (1, 2))
    output_bool((1, 2) != (2, 2))


def run() -> int32:
    data = [0, 1, 2, 3]

    t = [(d, d + d) for d in data]
    for i in t:
        tt = (Some(i[1]), i[0])
        tl = ([i[0], i[1] + i[0]], i[1])
        output_int32(tt[0].unwrap())
        output_int32(tt[1])
        output_int32(tl[0][0])
        output_int32(tl[0][1])
        output_int32(tl[1])

    output_int32(len(()))
    output_int32(len((1,)))
    output_int32(len((1, 2)))
    output_int32(len((1, 2, 3)))
    output_int32(len((1, 2, 3, 4)))
    output_int32(len((1, 2, 3, 4, 5)))

    test_tuple_eq()
    test_tuple_ne()

    return 0