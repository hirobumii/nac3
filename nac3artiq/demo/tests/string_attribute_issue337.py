from min_artiq import *
from numpy import int32

@nac3
class Demo:
    attr1: Kernel[str]
    attr2: Kernel[int32]

    @kernel
    def __init__(self):
        self.attr2 = 32
        self.attr1 = "SAMPLE"

    @kernel
    def run(self):
        print_int32(self.attr2)
        self.attr1

