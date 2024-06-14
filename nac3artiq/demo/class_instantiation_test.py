from min_artiq import *
from numpy import int32, ceil


@nac3
class Foo:
    attr: KernelInvariant[int32]
        
@nac3
class Bar:
    core: KernelInvariant[Core]
    attr2: KernelInvariant[int32]

    def __init__(self):
        self.core = Core()
        self.attr2 = 4

    @kernel
    def run(self):
        self.core.reset()
        a: Foo = Foo()
        


if __name__ == "__main__":
    Bar().run()