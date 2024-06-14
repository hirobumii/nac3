from min_artiq import *
from numpy import int32


@nac3
class Demo:
    core: KernelInvariant[Core]
    attr1: KernelInvariant[str]
    attr2: KernelInvariant[int32]


    def __init__(self):
        self.core = Core()
        self.attr2 = 32
        self.attr1 = "SAMPLE"

    @kernel
    def run(self):
        print_int32(self.attr2)
        self.attr1


if __name__ == "__main__":
    Demo().run()
