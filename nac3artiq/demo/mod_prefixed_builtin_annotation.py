from typing import Literal

from min_artiq import Core, KernelInvariant, compile, kernel
import numpy as np
from numpy import ndarray

@compile
class FindTrapResonance():
    core: KernelInvariant[Core]
    frequencies: KernelInvariant[np.ndarray[float, Literal[1]]]

    def __init__(self):
        self.core = Core()
        self.frequencies = np.zeros((1,))

    @kernel
    def run(self):
        pass

if __name__ == "__main__":
    FindTrapResonance().run()
