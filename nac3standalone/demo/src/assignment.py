@extern
def output_int32(x: int32):
    ...

@extern
def output_bool(x: bool):
    ...

@extern
def output_int32_list(xs: list[int32]):
    ...

def rhs_tuple_1():
    x, *ys, z = (1, 2, 3, 4, 5)
    output_int32(x)
    output_int32(len(ys))
    output_int32(ys[0])
    output_int32(ys[1])
    output_int32(ys[2])
    output_int32(z)

def rhs_tuple_2():
    x, y, *zs = (1, 2, 3, 4, 5)
    output_int32(x)
    output_int32(y)
    output_int32(len(zs))
    output_int32(zs[0])
    output_int32(zs[1])
    output_int32(zs[2])

def rhs_tuple_3():
    *xs, y, z = (1, 2, 3, 4, 5)
    output_int32(len(xs))
    output_int32(xs[0])
    output_int32(xs[1])
    output_int32(xs[2])
    output_int32(y)
    output_int32(z)

def zero_length_starred_tuple():
    *xs, y, z = (4, 5)
    output_int32(len(xs))
    output_int32(y)
    output_int32(z)

def assignment_order():
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

def class_field_assignment():
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
 
def lhs_mixed_assignment():
    x, [y, z] = 1, [2, 3]
    (a, b) = (4, 5)
    output_int32(x)
    output_int32(y)
    output_int32(z)
    output_int32(a)
    output_int32(b)

def rhs_list_assignment():
    (a, *b, c) = [1, 2, 3, 4, 5]
    output_int32(a)
    output_int32_list(b)
    output_int32(c)

    (*xs, y, z) = [x for x in range(100)]
    output_bool(len(xs) == 98)

    (u, v, *w) = [1, 2, 3, 4, 5]
    output_int32_list(w)

    f, *g = [1]
    output_int32(len(g))  # Should be 0, since g is empty

    [m, *n, o] = [1, 3]
    output_int32(m)
    output_int32(len(n))  # Should be 0, since n is empty
    output_int32(o)

def run() -> int32:
    rhs_tuple_1()
    rhs_tuple_2()
    rhs_tuple_3()
    zero_length_starred_tuple()
    assignment_order()
    class_field_assignment()
    lhs_mixed_assignment()
    rhs_list_assignment()
    return 0
