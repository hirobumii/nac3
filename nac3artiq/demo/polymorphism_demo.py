from typing import TypeVar, Generic
from min_artiq import *
from numpy import int32

@extern
def print_int(a: int32):
    ...

T = TypeVar('T')
V = TypeVar('V', int, float)

@kernel
def fun(a: T):
    pass

@nac3
class Demo(Generic[T]):
    core: KernelInvariant[Core]
    t: T
    a: int32

    def __init__(self, t: T, val: int32 = 3):
        self.core = Core()
        self.a = val
        self.t = t

    @kernel
    def poly(self, a: V):
        print_int(int32(a))
        if int32(a) > 5:
            print_int(self.a)
            self.poly(2.4)
        elif int32(a) < 2:
            print_int(int32(a))
            self.poly(3.4)

    @kernel
    def run(self):
        a = 5555
        a //= 2
        print_int(a)

        print_int(self.a)
        self.poly(6)
        print_int(11111111)
        self.poly(1.2)
        fun(True)
        fun((1, False, 1.2))
        


if __name__ == "__main__":
    Demo(1.23).run()
