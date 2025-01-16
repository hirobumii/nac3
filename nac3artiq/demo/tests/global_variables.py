from min_artiq import *
from numpy import int32

X: Kernel[int32] = 1

@rpc
def display_X():
    print_int32(X)

@kernel
def inc_X():
    global X
    X += 1

