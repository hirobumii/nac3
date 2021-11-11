@extern
def output_int(x: int32):
    ...

@extern
def output_double(x: float):
    ...


class A:
    a: int32
    b: B
    def __init__(self, a: int32):
        self.a = a
        self.b = B(a + 1)

    def get_a(self) -> int32:
        return self.a

    def get_self(self) -> A:
        return self
    
    def get_b(self) -> B:
        return self.b

class B:
    b: int32
    def __init__(self, a: int32):
        self.b = a


def run() -> int32:
    i = 1
    i += 2
    output_int(i)
    i -= 1
    output_int(i)
    i *= 3
    output_int(i)
    i //= 4
    # i /= 4 # should error, cannot unify float with int32
    output_int(i)

    d = 2.4
    output_double(d)
    d -= 0.1
    output_double(d)
    d *= 2.0
    output_double(d)
    d /= 0.5
    output_double(d)

    a = A(10)
    output_int(a.a)

    a = A(20)
    output_int(a.a)
    output_int(a.get_a())
    output_int(a.get_b().b)
    return 0
