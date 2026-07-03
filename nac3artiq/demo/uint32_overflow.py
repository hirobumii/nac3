from min_artiq import *
import numpy as np


@compile
class Test:
    core: KernelInvariant[Core]
    scalar: KernelInvariant[np.uint32]
    values: KernelInvariant[list[np.uint32]]
    empty: KernelInvariant[list[np.uint32]]

    def __init__(self):
        self.core = Core()
        self.scalar = np.iinfo(np.uint32).max
        self.values = [np.iinfo(np.uint32).max, 0, 42]
        self.empty = []

    @kernel
    def run(self):
        pass


if __name__ == "__main__":
    Test().run()
