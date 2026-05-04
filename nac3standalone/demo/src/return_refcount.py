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


def early_return_list(flag: bool) -> list[int32]:
    a = [1, 2, 3]
    if flag:
        return a
    b = [4, 5, 6]
    return b


def return_in_try(flag: bool) -> list[int32]:
    a = [10, 20, 30]
    try:
        if flag:
            return a
    except:
        pass
    return [0]


def return_in_try_finally(flag: bool) -> list[int32]:
    a = [100, 200, 300]
    try:
        if flag:
            return a
    except:
        pass
    return [99]


def run() -> int32:
    output_strln("=== early_return_list(true) ===")
    r1 = early_return_list(True)
    output_int32(r1[0])
    output_refcount(r1)

    output_strln("=== early_return_list(false) ===")
    r2 = early_return_list(False)
    output_int32(r2[0])
    output_refcount(r2)

    output_strln("=== return_in_try(true) ===")
    r3 = return_in_try(True)
    output_int32(r3[0])
    output_refcount(r3)

    output_strln("=== return_in_try_finally(true) ===")
    r4 = return_in_try_finally(True)
    output_int32(r4[0])
    output_refcount(r4)

    output_strln("=== return_in_try_finally(false) ===")
    r5 = return_in_try_finally(False)
    output_int32(r5[0])
    output_refcount(r5)

    return 0
