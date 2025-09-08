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
class TurnOn(Generic[T]):
    ttl: KernelInvariant[T]

    def __init__(self, ttl):
        self.ttl = ttl

    @kernel
    def turn_on(self):
        self.ttl.on()


@compile
class TypeVarTest:
    core: KernelInvariant[Core]
    turn_on: KernelInvariant[TurnOn[TTLOut]]

    def __init__(self):
        self.core = Core()
        self.turn_on = TurnOn(TTLOut())

    @kernel
    def run(self):
        self.turn_on.turn_on()


if __name__ == "__main__":
    TypeVarTest().run()
