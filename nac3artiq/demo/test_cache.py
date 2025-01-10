from min_artiq import *
from numpy import int32

@nac3
class StringListTest:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel 
    def test_string_and_list(self, key: str, value: list[int32]):
        print_int32(int32(42))

def test_params():
    exp = StringListTest()
    exp.test_string_and_list("x4", [])

if __name__ == "__main__":
    test_params()
