from __future__ import annotations

@extern
def output_int32(x: int32):
    ...

class A:
    a: int32

    def __init__(self, a: int32):
        self.a = a
    
    def output_all_fields(self):
        output_int32(self.a)
    
    def set_a(self, a: int32):
        self.a = a

class B(A):
    b: int32

    def __init__(self, b: int32):
        A.__init__(self, b + 1)
        self.set_b(b)
    
    def output_parent_fields(self):
        A.output_all_fields(self)

    def output_all_fields(self):
        A.output_all_fields(self)
        output_int32(self.b)
    
    def set_b(self, b: int32):
        self.b = b

class C(B):
    c: int32

    def __init__(self, c: int32):
        B.__init__(self, c + 1)
        self.c = c
    
    def output_parent_fields(self):
        B.output_all_fields(self)

    def output_all_fields(self):
        B.output_all_fields(self)
        output_int32(self.c)
    
    def set_c(self, c: int32):
        self.c = c

def run() -> int32:
    ccc = C(10)
    ccc.output_all_fields()
    ccc.set_a(1)
    ccc.set_b(2)
    ccc.set_c(3)
    ccc.output_all_fields()

    bbb = B(10)
    bbb.set_a(9)
    bbb.set_b(8)
    bbb.output_all_fields()
    ccc.output_all_fields()

    return 0
