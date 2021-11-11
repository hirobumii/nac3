@extern
def output_int(x: int32):
    ...

T = TypeVar('T', int32, bool)
V = TypeVar('V')

class A(Generic[V], 'Test'):
    v: V
    def __init__(self, val: V):
        self.v = val
        self.a = 1111
    
    def poly(self, v: V, t: T, a: int32):
        if a > 1:
            self.poly(v, 12, a - 1)
            self.poly(self.v, False, a - 1)
    
    def put_other_int(self, a: int32):
        output_int(a * 2)

class Test:
    a: int32

    def __init__(self, val: int32):
        self.a = val

    def put_self_int(self):
        output_int(self.a)
    
    def put_other_int(self, a: int32):
        output_int(a)

def run() -> int32:
    a = A(1.2)
    a.poly(2.4, True, 2)
    a.poly(3.6, 1, 4)

    a2 = A(Test(2))
    a2.poly(Test(2), True, 2)
    a2.poly(Test(2), 1, 4)

    a3 = A(Test(2))
    a3.put_other_int(44) # expect 88
    a3.v.put_other_int(44) # expect 44
    a3.put_self_int() # expect 1111
    a3.v.put_self_int() # expect 2
    
    return 0