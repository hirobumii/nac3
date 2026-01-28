from __future__ import annotations

from min_artiq import *
from numpy import int32, int64
from typing import Generic, TypeVar


@compile
class ProtoRev8:
    """Has back-reference to CPLD[ProtoRev8] - this is key to triggering the bug."""
    cpld: KernelInvariant[CPLD[ProtoRev8]]
    cfg_reg: Kernel[int64]

    def __init__(self, cpld: CPLD[ProtoRev8]):
        self.cpld = cpld
        self.cfg_reg = int64(0)

    @kernel
    def cfg_write(self, cfg: int64):
        pass

    @kernel
    def sta_read(self) -> int32:
        return int32(0)


@compile
class ProtoRev9:
    """Has back-reference to CPLD[ProtoRev9] - this is key to triggering the bug."""
    cpld: KernelInvariant[CPLD[ProtoRev9]]
    cfg_reg: Kernel[int64]

    def __init__(self, cpld: CPLD[ProtoRev9]):
        self.cpld = cpld
        self.cfg_reg = int64(0)

    @kernel
    def cfg_write(self, cfg: int64):
        pass

    @kernel
    def sta_read(self) -> int32:
        return int32(0)


V = TypeVar("V", ProtoRev8, ProtoRev9)


@compile
class CPLD(Generic[V]):
    core: KernelInvariant[Core]
    version: KernelInvariant[V]
    cfg_reg: Kernel[int64]

    def __init__(self):
        self.core = Core()
        # Create version with back-reference to self
        self.version = ProtoRev9(self)
        self.cfg_reg = int64(0)

    @kernel
    def cfg_write(self, cfg: int64):
        self.version.cfg_write(cfg)

    @kernel
    def sta_read(self) -> int32:
        return self.version.sta_read()


@compile
class AD9910:
    core: KernelInvariant[Core]
    cpld: KernelInvariant[CPLD[ProtoRev9]]

    def __init__(self, cpld: CPLD[ProtoRev9]):
        self.core = Core()
        self.cpld = cpld

    @kernel
    def init(self):
        self.cpld.cfg_write(self.cpld.cfg_reg)


@compile
class Test:
    core: KernelInvariant[Core]
    cpld: KernelInvariant[CPLD[ProtoRev9]]
    dds: KernelInvariant[AD9910]

    def __init__(self):
        self.core = Core()
        self.cpld = CPLD()
        self.dds = AD9910(self.cpld)

    @kernel
    def run(self):
        self.cpld.cfg_write(int64(0))
        self.dds.init()


if __name__ == "__main__":
    Test().run()
