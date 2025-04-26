from min_artiq import *
import numpy
from numpy import int32


@compile
class NumpyBoolDecay:
    core: KernelInvariant[Core]
    np_true: KernelInvariant[bool]
    np_false: KernelInvariant[bool]
    np_int: KernelInvariant[int32]
    np_float: KernelInvariant[float]
    np_str: KernelInvariant[str]

    def __init__(self):
        self.core = Core()
        self.np_true = numpy.True_
        self.np_false = numpy.False_
        self.np_int = numpy.int32(0)
        self.np_float = numpy.float64(0.0)
        self.np_str = numpy.str_("")

    @kernel
    def run(self):
        pass


if __name__ == "__main__":
    NumpyBoolDecay().run()
