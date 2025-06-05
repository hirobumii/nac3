@extern
def output_int32(x: int32):
    ...

@extern
def output_bool(x: bool):
    ...

def example1():
    x, *ys, z = (1, 2, 3, 4, 5)
    output_int32(x)
    output_int32(len(ys))
    output_int32(ys[0])
    output_int32(ys[1])
    output_int32(ys[2])
    output_int32(z)

def example2():
    x, y, *zs = (1, 2, 3, 4, 5)
    output_int32(x)
    output_int32(y)
    output_int32(len(zs))
    output_int32(zs[0])
    output_int32(zs[1])
    output_int32(zs[2])

def example3():
    *xs, y, z = (1, 2, 3, 4, 5)
    output_int32(len(xs))
    output_int32(xs[0])
    output_int32(xs[1])
    output_int32(xs[2])
    output_int32(y)
    output_int32(z)

def example4():
    *xs, y, z = (4, 5)
    output_int32(len(xs))
    output_int32(y)
    output_int32(z)

def example5():
    # Example from: https://docs.python.org/3/reference/simple_stmts.html#assignment-statements
    x = [0, 1]
    i = 0
    i, x[i] = 1, 2 # i is updated, then x[i] is updated
    output_int32(i)
    output_int32(x[0])
    output_int32(x[1])

class A:
    value: int32
    def __init__(self):
        self.value = 1000

def example6():
    ws = [88, 7, 8]
    a = A()
    x, [y, *ys, a.value], ws[0], (ws[0],) = 1, (2, False, 4, 5), 99, (6,)
    output_int32(x)
    output_int32(y)
    output_bool(ys[0])
    output_int32(ys[1])
    output_int32(a.value)
    output_int32(ws[0])
    output_int32(ws[1])
    output_int32(ws[2])
 
def example7():
    x, [y, z] = 1, [2, 3]
    (a, b) = (4, 5)
    output_int32(x)
    output_int32(y)
    output_int32(z)
    output_int32(a)
    output_int32(b)

def example8():
    (a, *b, c) = [1, 2, 3, 4, 5]
    output_int32(a)
    for val in b:
        output_int32(val)
    output_int32(c)

    (*xs, y, z) = [x for x in range(100)]
    output_bool(len(xs) == 98)

    (u, v, *w) = [1, 2, 3, 4, 5]
    output_int32(w[0])
    output_int32(w[1])
    output_int32(w[2])

    f, *g = [1]
    output_int32(len(g))  # Should be 0, since g is empty

def run() -> int32:
    example1()
    example2()
    example3()
    example4()
    example5()
    example6()
    example7()
    example8()
    return 0
