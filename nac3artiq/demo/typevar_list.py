from min_artiq import *
from typing import Generic, TypeVar


@compile
class TTLInOut:
    @kernel
    def on(self):
        pass


@compile
class TTLOut:
    @kernel
    def on(self):
        pass


T = TypeVar("T", TTLOut, TTLInOut)


@compile
class TTLCtrl(Generic[T]):
    ttls: KernelInvariant[list[T]]

    def __init__(self, ttls):
        self.ttls = ttls

    @kernel
    def turn_on(self):
        for ttl in self.ttls:
            ttl.on()


@compile
class TypeVarTest:
    core: KernelInvariant[Core]
    ctrl0: KernelInvariant[TTLCtrl[TTLOut]]
    ctrl1: KernelInvariant[TTLCtrl[TTLInOut]]

    def __init__(self):
        self.core = Core()
        self.ctrl0 = TTLCtrl([TTLOut(), TTLOut()])
        self.ctrl1 = TTLCtrl([TTLInOut(), TTLInOut()])

    @kernel
    def run(self):
        self.ctrl0.turn_on()
        self.ctrl1.turn_on()


if __name__ == "__main__":
    TypeVarTest().run()
