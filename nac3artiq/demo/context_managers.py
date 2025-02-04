from numpy import int32
from min_artiq import *

@nac3
class PassContextManager:
    @kernel
    def __init__(self):
        pass

    @kernel
    def __enter__(self):
        pass

    @kernel
    def __exit__(self):
        pass

@nac3
class CustomDataContextManager:
    a: Kernel[int32]
    b: Kernel[int32]

    @kernel
    def __init__(self, a: int32, b: int32):
        self.a = a
        self.b = b

    @kernel
    def __enter__(self):
        self.a += 1
        print_int32(self.a)

    @kernel
    def __exit__(self):
        self.b += 1
        print_int32(self.b)

@nac3
class WithAsContextManager:
    a: Kernel[int32]

    @kernel
    def __init__(self, val: int32):
        self.a = val

    @kernel
    def __enter__(self) -> int32:
        print_int32(self.a)
        return self.a

    @kernel
    def __exit__(self):
        print_int32(self.a)

@nac3
class CtxMgrTest:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        x = 0
        a = PassContextManager()
        with a:
            x += 1
        
        h = CustomDataContextManager(1, 2)
        with h:
            x += h.a + h.b

        with WithAsContextManager(15) as num:
            x += num

        with WithAsContextManager(10) as c, WithAsContextManager(5) as d:
            e: int32 = c + d
            x += e

        print_int32(x)
        return

if __name__ == "__main__":
    CtxMgrTest().run()