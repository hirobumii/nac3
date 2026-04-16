class A:
    def __init__(self):
        pass


class B:
    def __init__(self):
        pass


T = TypeVar("T", A, B)


class C(Generic[T]):
    x: T

    # a typevar id for T
    def __init__(self, x: T):
        self.x = x


# a different typevar id for T
def simple(a: T):
    pass


class D:
    f: C[A]

    def __init__(self, f_x: A):
        # acquires typevar id from return type
        # then resolves for concrete type
        #
        # different typevar ids for T must be reconciled
        self.f = C(f_x)


def run() -> int32:
    insta = A()
    d = D(insta)
    return 0
