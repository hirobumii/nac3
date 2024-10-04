@extern
def output_int32(x: int32):
    ...

@extern
def output_int64(x: int64):
    ...

X: int32 = 0
Y: int64 = int64(1)

def f():
    global X, Y
    X = 1
    Y = int64(2)

def run() -> int32:
    global X, Y

    output_int32(X)
    output_int64(Y)
    f()
    output_int32(X)
    output_int64(Y)

    X = 0
    Y = int64(0)
    output_int32(X)
    output_int64(Y)

    return 0