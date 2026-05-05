from min_artiq import *
from numpy import int32, int64


@extern
def get_tuple() -> tuple[int64, int32, bool]:
    raise NotImplementedError("syscall not simulated")


@compile
class SretInvokeRepro:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def call_sret(self):
        (a, b, c) = get_tuple()

    @kernel
    def run(self):
        self.call_sret()


if __name__ == "__main__":
    SretInvokeRepro().run()
