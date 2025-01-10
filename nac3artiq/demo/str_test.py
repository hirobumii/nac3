from min_artiq import *
from numpy import int32

@nac3
class StrTest:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def test_string(self, s: str):
        # Just print the string to verify it works
        print_int32(int32(42))

    def run(self):
        self.test_string("hello")

if __name__ == "__main__":
    StrTest().run()
