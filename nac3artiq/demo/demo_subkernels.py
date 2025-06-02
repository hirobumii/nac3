from min_artiq import *


@compile
class Demo:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @subkernel(destination=1)
    def simple_sk(self):
        pass

    @kernel
    def run(self):
        self.core.reset()
        subkernel_preload(self.simple_sk)
        self.simple_sk()
        subkernel_await(self.simple_sk)


if __name__ == "__main__":
    Demo().run()
