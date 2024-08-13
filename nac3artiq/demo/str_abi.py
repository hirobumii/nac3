from min_artiq import *
from numpy import ndarray, zeros as np_zeros


@nac3
class StrFail:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def hello(self, arg: str):
        pass

    @kernel
    def consume_ndarray(self, arg: ndarray[str, 1]):
        pass

    def run(self):
        self.hello("world")
        self.consume_ndarray(np_zeros([10], dtype=str))


if __name__ == "__main__":
    StrFail().run()
