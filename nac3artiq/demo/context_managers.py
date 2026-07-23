from numpy import int32
from min_artiq import *

@compile
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

@compile
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

@compile
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

@compile
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

@compile
class CriticalCtxMgrTest:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        x = 0

        # Default reservation size.
        with critical():
            x += 1

        # Reservation size given positionally and by keyword.
        with critical(2):
            a = [1, 2, 3]
            x += a[0]

        with critical(num_free_pages=2):
            b = [4, 5, 6]
            x += b[2]

        # `critical(0)` reserves nothing - Still valid.
        with critical(0):
            x += 1

        # Nested critical regions.
        with critical(2):
            with critical(1):
                c = [7, 8]
                x += c[1]

        # Nesting in both directions with the ARTIQ timeline context managers, which
        # nac3artiq intercepts in its own `gen_with` before delegating to the core one.
        with sequential:
            with critical(1):
                x += 1

        with critical(1):
            with parallel:
                x += 1

        print_int32(x)
        return

if __name__ == "__main__":
    CtxMgrTest().run()
    CriticalCtxMgrTest().run()
