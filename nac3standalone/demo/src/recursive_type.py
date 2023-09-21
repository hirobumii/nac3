from __future__ import annotations

@extern
def output_int32(a: int32, newline: bool=True):
    ...

class A:
    d: int32
    a: list[B]
    def __init__(self, b: list[B]):
        self.d = 123
        self.a = b
    
    def f(self):
        output_int32(self.d)

class B:
    a: A
    def __init__(self, a: A):
        self.a = a

    def ff(self):
        self.a.f()

class Demo:
    a: A
    def __init__(self, a: A):
        self.a = a

def run() -> int32:
    aa = A([])
    bb = B(aa)
    aa.a = [bb]
    d = Demo(aa)
    d.a.a[0].ff()
    return 0