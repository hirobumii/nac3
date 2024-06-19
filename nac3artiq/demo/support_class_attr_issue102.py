from min_artiq import *
from numpy import int32


@nac3
class Demo:
    attr1: KernelInvariant[int32] = 2
    attr2: int32 = 4
    attr3: Kernel[int32]

    @kernel
    def __init__(self):
        self.attr3 = 8


@nac3
class NAC3Devices:
    core: KernelInvariant[Core]
    attr4: KernelInvariant[int32] = 16
    
    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        Demo.attr1  # Supported
        # Demo.attr2  # Field not accessible on Kernel
        # Demo.attr3  # Only attributes can be accessed in this way
        # Demo.attr1 = 2  # Attributes are immutable

        self.attr4  # Attributes can be accessed within class

        obj = Demo()
        obj.attr1  # Attributes can be accessed by class objects

        NAC3Devices.attr4  # Attributes accessible for classes without __init__


if __name__ == "__main__":
    NAC3Devices().run()
