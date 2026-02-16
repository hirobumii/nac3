from __future__ import annotations

from min_artiq import *
from min_artiq import Auto
from typing import Generic, TypeVar
from numpy import int32

# Self-referential ProtoRev types

@compile
class ProtoRev8:
    cpld: KernelInvariant[CPLD[ProtoRev8]]

    def __init__(self, cpld):
        self.cpld = cpld

    @kernel
    def cfg_write(self, cfg: int32):
        self.cpld.cfg_reg = cfg

    @kernel
    def sta_read(self) -> int32:
        return self.cpld.cfg_reg

@compile
class ProtoRev9:
    cpld: KernelInvariant[CPLD[ProtoRev9]]

    def __init__(self, cpld):
        self.cpld = cpld

    @kernel
    def cfg_write(self, cfg: int32):
        self.cpld.cfg_reg = cfg

    @kernel
    def sta_read(self) -> int32:
        return self.cpld.cfg_reg


V = TypeVar("V", ProtoRev8, ProtoRev9)


@compile
class CPLD(Generic[V]):
    core: KernelInvariant[Core]
    version: KernelInvariant[V]
    cfg_reg: Kernel[int32]

    def __init__(self, core: Core, version_cls, proto_rev: int32 = int32(0x09)):
        self.core = core
        self.cfg_reg = int32(0)
        if proto_rev == int32(0x08):
            self.version = ProtoRev8(self)
        else:
            self.version = ProtoRev9(self)

    @kernel
    def cfg_write(self, cfg: int32):
        self.version.cfg_write(cfg)

    @kernel
    def sta_read(self) -> int32:
        return self.version.sta_read()

    @kernel
    def init(self):
        pass

@compile
class UninstantiatedDevice:
    core: KernelInvariant[Core]
    cpld: KernelInvariant[CPLD[Auto]]
    chip_select: KernelInvariant[int32]

    def __init__(self, core: Core, cpld, chip_select: int32):
        self.core = core
        self.cpld = cpld
        self.chip_select = chip_select

    @kernel
    def init(self):
        self.cpld.cfg_write(self.chip_select)

    @kernel
    def get_status(self) -> int32:
        return self.cpld.sta_read()

@compile
class StressDemo:
    core: KernelInvariant[Core]
    cpld: KernelInvariant[CPLD[Auto]]

    def __init__(self):
        self.core = Core()
        self.cpld = CPLD(self.core, ProtoRev9, int32(0x09))

    @kernel
    def run(self):
        self.cpld.init()


if __name__ == "__main__":
    StressDemo().run()
