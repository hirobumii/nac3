T = TypeVar("T")

@extern
def output_refcount(str: str, x: T):
    ...


@extern
def output_int32_list(x: list[int32]):
    ...


@extern
def output_strln(x: str):
    ...


def create_list() -> list[int32]:
    return [5, 6, 7, 8]


def foo(lst: list[int32]):
    output_refcount("lst", lst)


def run() -> int32:
    data = [0, 1, 2, 3]

    output_refcount("data", data)

    data = data
    output_refcount("data", data)

    data2 = data
    output_refcount("data", data)
    output_refcount("data2", data2)

    foo(data2)
    output_refcount("data", data)
    output_refcount("data2", data2)

    if False:
        data3 = data2
        output_refcount("data", data)
        output_refcount("data2", data2)
        output_refcount("data3", data3)

    output_refcount("data", data)
    output_refcount("data2", data2)

    lst = create_list()
    output_int32_list(lst)
    output_refcount("lst", lst)

    lst_2d = [[1, 2], [3, 4]]
    output_refcount("lst_2d", lst_2d)

    return 0
