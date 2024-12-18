from min_artiq import *
from numpy import int32

@rpc
def sum_3(a: int32, b: int32 = 10, c: int32 = 20) -> int32:
    """
    An RPC function to test NAC3's handling of positional/keyword arguments.
    """
    return int32(a + b + c)

@nac3
class RpcKwargTest:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        #1) All positional => a=1, b=2, c=3 -> total=6
        s1 = sum_3(1, 2, 3)
        assert s1 == 6
        
        #2) Use the default b=10, c=20 => a=5 => total=35
        s2 = sum_3(5)
        assert s2 == 35

        #3) a=1 (positional), b=100 (keyword), omit c => c=20 => total=121
        s3 = sum_3(1, b=100)
        assert s3 == 121

        #4) a=2, c=300 => b=10 (default) => total=312
        s4 = sum_3(a=2, c=300)
        assert s4 == 312

if __name__ == "__main__":
    RpcKwargTest().run()