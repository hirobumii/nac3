T = TypeVar("T")

@extern
def output_refcount(x: T):
    ...

@extern
def output_int32(x: int32):
    ...


def single_element_source():
    # `[x] * n` aliases the single inner list `n` times, so its refcount gains exactly `n`.
    inner = [int32(1), int32(2)]
    output_refcount(inner)      # baseline

    l = [inner] * 3
    output_refcount(inner)      # baseline + 3

    # Aliasing: every row is the same object, so a write through one row is visible everywhere.
    l[0][0] = int32(99)
    output_int32(l[0][0])
    output_int32(l[1][0])
    output_int32(l[2][0])

    output_refcount(l[1])       # l[1] is `inner`


def multi_element_source():
    # `[a, b] * n` aliases each distinct element `n` times, so each gains exactly `n`.
    a = [int32(1)]
    b = [int32(2)]
    output_refcount(a)
    output_refcount(b)

    l = [a, b] * 4
    output_refcount(a)          # baseline + 4
    output_refcount(b)          # baseline + 4
    output_int32(int32(len(l))) # 8


def zero_repeat():
    # `[x] * 0` is empty and must not touch the element's refcount.
    inner = [int32(7)]
    output_refcount(inner)
    l = [inner] * 0
    output_refcount(inner)      # unchanged
    output_int32(int32(len(l))) # 0


def run() -> int32:
    single_element_source()
    multi_element_source()
    zero_repeat()
    return 0
