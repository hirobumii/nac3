from min_artiq import *
from numpy import int32

@nac3
class Base:
    c: Kernel[int32]
    def __init__(self):
        self.c = 4
        self.a = 3
        self.a = 5

@nac3
class D:
    da: KernelInvariant[int32]
    def __init__(self):
        self.da = 321

@nac3
class C:
    ca: KernelInvariant[int32]
    cb: KernelInvariant[D]
    def __init__(self, d: D):
        self.ca = 123
        self.cb = d

@nac3
class A(Base):
    core: KernelInvariant[Core]
    led0: KernelInvariant[TTLOut]
    led1: KernelInvariant[TTLOut]
    d: Kernel[bool]
    cc: Kernel[C]

    def __init__(self, c):
        super().__init__()
        self.core = Core()
        self.led0 = TTLOut(self.core, 18)
        self.led1 = TTLOut(self.core, 19)
        self.b = 3
        self.d = False
        self.cc = c
    
    @kernel
    def run(self):
        print_int32(self.cc.cb.da)


if __name__ == '__main__':
    d = D()
    print(d.da)
    c = C(d)
    print(d.da)
    print(c.cb.da)
    
    a = A(c)
    print(a.a)
    print(a.b)
    # print(c.ca) # fail
    # print(c.cb.da) # fail
    
    a.run()

    # d = D() # redefine, ok
    # c = C(d) # redefine, ok

    # print(a.c) # fail
    # a.c = 2 # fail
    # a.d = 1 # fail
    # a.cc = 1 # fail
    # c.ca = 1 # fail
    # c.cb = 1 # fail
    # c.cb.da = 1 # fail
    # d.da = 1 # fail