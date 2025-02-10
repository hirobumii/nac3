from min_artiq import *
from numpy import int32

# Global Variable Definition
X: Kernel[int32] = 1

# TopLevelFunction Defintion
@kernel
def display_X():
    print_int32(X)

# TopLevel Class Definition
@nac3
class A:    
    @kernel
    def __init__(self):
        self.set_x(1)
    
    @kernel
    def set_x(self, new_val: int32):
        global X
        X = new_val
    
    @kernel
    def get_X(self) -> int32:
        return X
