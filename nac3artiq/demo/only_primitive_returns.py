from min_artiq import *
from numpy import int32

@compile
class NestedTupleList:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @rpc
    def get_nested_list(self, length: int32) -> list[int32]:
        return [int32(i) for i in range(length)]

    @kernel
    def run(self):
        a = self.get_nested_list(3)
        # b = [x for x in a]
        # print_rpc(b)

if __name__ == "__main__":
    NestedTupleList().run()
