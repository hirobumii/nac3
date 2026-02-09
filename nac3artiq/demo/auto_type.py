from min_artiq import *
from min_artiq import Auto
from numpy import int32

@compile
class Inner:
    core: KernelInvariant[Core]
    value: KernelInvariant[int32]

    def __init__(self, core: Core, v: int32):
        self.core = core
        self.value = v

    @kernel
    def get_value(self) -> int32:
        return self.value


@compile
class AutoDemo:
    core: KernelInvariant[Core]

    # Bare Auto
    x_auto: KernelInvariant[Auto]        # inferred as int32
    y_auto: Kernel[Auto]                 # inferred as int32 (mutable)
    f_auto: KernelInvariant[Auto]        # inferred as float
    s_auto: KernelInvariant[Auto]        # inferred as str
    b_auto: KernelInvariant[Auto]        # inferred as bool
    obj_auto: KernelInvariant[Auto]      # inferred as Inner

    # Nested Auto
    list_auto: KernelInvariant[list[Auto]]  # inferred as list[int32]

    def __init__(self):
        self.core = Core()
        self.x_auto = int32(42)
        self.y_auto = int32(0)
        self.f_auto = 3.14
        self.s_auto = "hello"
        self.b_auto = True
        self.obj_auto = Inner(self.core, int32(99))
        self.list_auto = [int32(1), int32(2), int32(3)]

    @kernel
    def test_auto_int(self) -> int32:
        return self.x_auto

    @kernel
    def test_auto_mutable(self) -> int32:
        self.y_auto = int32(10)
        return self.y_auto

    @kernel
    def test_auto_float(self) -> float:
        return self.f_auto

    @kernel
    def test_auto_bool(self) -> bool:
        return self.b_auto

    @kernel
    def test_auto_object(self) -> int32:
        return self.obj_auto.get_value()

    @kernel
    def test_auto_list(self) -> int32:
        total: int32 = int32(0)
        for v in self.list_auto:
            total = total + v
        return total

    @kernel
    def run(self):
        x = self.test_auto_int()
        y = self.test_auto_mutable()
        f = self.test_auto_float()
        b = self.test_auto_bool()
        o = self.test_auto_object()
        l = self.test_auto_list()


if __name__ == "__main__":
    AutoDemo().run()
