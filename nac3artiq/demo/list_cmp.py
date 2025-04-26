from min_artiq import *
from numpy import int32


@compile
class EmptyList:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @rpc
    def get_empty(self) -> list[int32]:
        return []

    @kernel
    def run(self):
        a: list[int32] = self.get_empty()
        if a != []:
            raise ValueError


if __name__ == "__main__":
    EmptyList().run()
