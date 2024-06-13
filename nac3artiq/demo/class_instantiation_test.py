from min_artiq import *

@nac3
class Foo:
    attr: Kernel[str]
    @kernel
    def __init__(self):
        self.attr = "attr"

@nac3
class Bar:
    core: KernelInvariant[Core]
    def __init__(self):
        self.core = Core()
    @kernel
    def run(self):
        a = Foo()

if __name__ == "__main__":
    Bar().run()