@extern
def output_int(x: int32):
    ...

def foo(x: int32 = 1):
    output_int(x)

def run() -> int32:
    foo()
    a = A()
    a.fun(1, 2)
    a.fun(4444)
    return 0

class A:
    def __init__(self):
        pass
    
    def fun(self, a: int32, b: int32 = 333, c: tuple[int32, int32, int64] = (222, 111, int64(123))):
        aa, bb, cc = c
        output_int(a)
        output_int(aa)
        output_int(b)
        output_int(bb)
