def f(*args: int32):
    pass


def run() -> int32:
    f()
    f(1)
    f(1, 2)
    f(1, 2, 3)

    return 0
