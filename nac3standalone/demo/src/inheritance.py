from __future__ import annotations

@extern
def output_int32(x: int32):
    ...

class A:
    a: int32

    def __init__(self, a: int32):
        self.a = a
    
    def f1(self):
        self.f2()
    
    def f2(self):
        output_int32(self.a)

class B(A):
    b: int32

    def __init__(self, b: int32):
        self.a = b + 1
        self.b = b


def run() -> int32:
    aaa = A(5)
    bbb = B(2)
    aaa.f1()
    bbb.f1()
    return 0
