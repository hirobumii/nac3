from __future__ import annotations

from min_artiq import *
from min_artiq import Auto
from typing import Generic, TypeVar
from numpy import int32, int64


@compile
class ProtoRev8:
    """Simulates a hardware revision with limited features."""
    core: KernelInvariant[Core]

    def __init__(self, core: Core):
        self.core = core

    @kernel
    def cfg_write(self, data: int32):
        pass

    @kernel
    def cfg_att_en(self, channel: int32, on: bool):
        raise ValueError("cfg_att_en not supported on ProtoRev8")


@compile
class ProtoRev9:
    """Simulates a hardware revision with full features."""
    core: KernelInvariant[Core]

    def __init__(self, core: Core):
        self.core = core

    @kernel
    def cfg_write(self, data: int32):
        pass

    @kernel
    def cfg_att_en(self, channel: int32, on: bool):
        pass


V = TypeVar("V", ProtoRev8, ProtoRev9)


@compile
class CPLD(Generic[V]):
    """Simulates ARTIQ's CPLD class - generic over hardware revision."""
    core: KernelInvariant[Core]
    version: KernelInvariant[V]
    cfg_reg: Kernel[int32]

    def __init__(self, core: Core, version: V):
        self.core = core
        self.version = version
        self.cfg_reg = int32(0)

    @kernel
    def cfg_write(self, cfg: int32):
        self.cfg_reg = cfg

    @kernel
    def set_att_en(self, channel: int32, on: bool):
        self.version.cfg_att_en(channel, on)


@compile
class RegIOUpdate:
    """Simulates ARTIQ's RegIOUpdate - constructed with required args."""
    cpld: KernelInvariant[CPLD[ProtoRev9]]
    chip_select: KernelInvariant[int32]

    def __init__(self, cpld, chip_select):
        self.cpld = cpld
        self.chip_select = chip_select

    @kernel
    def pulse_mu(self, duration: int64):
        cfg = self.cpld.cfg_reg
        self.cpld.cfg_write(cfg)


IoUpdateT = TypeVar("IoUpdateT", RegIOUpdate, TTLOut)


@compile
class AD9910(Generic[IoUpdateT]):
    """Simulates ARTIQ's AD9910 - generic over IO update type.

    This is the key test case: AD9910 has required constructor args
    (core, cpld, chip_select) and a field `cpld: CPLD[Auto]` that
    nac3 should be able to infer from the runtime value.
    """
    core: KernelInvariant[Core]
    cpld: KernelInvariant[CPLD[Auto]]       # <-- This should work: infer CPLD variant from runtime
    chip_select: KernelInvariant[int32]
    io_update: KernelInvariant[IoUpdateT]

    def __init__(self, core: Core, cpld, chip_select: int32):
        self.core = core
        self.cpld = cpld
        self.chip_select = chip_select
        self.io_update = RegIOUpdate(self.cpld, self.chip_select)

    @kernel
    def init(self):
        self.cpld.cfg_write(self.chip_select)

    @kernel
    def get_chip_select(self) -> int32:
        return self.chip_select


@compile
class Inner:
    core: KernelInvariant[Core]
    value: KernelInvariant[int32]

    def __init__(self, core: Core, v: int32):
        self.core = core
        self.value = v

    @kernel
    def get_value(self) -> int32:
        return self.value


@compile
class AutoDemo:
    core: KernelInvariant[Core]

    # Bare Auto
    x_auto: KernelInvariant[Auto]        # inferred as int32
    y_auto: Kernel[Auto]                 # inferred as int32 (mutable)
    f_auto: KernelInvariant[Auto]        # inferred as float
    s_auto: KernelInvariant[Auto]        # inferred as str
    b_auto: KernelInvariant[Auto]        # inferred as bool
    obj_auto: KernelInvariant[Auto]      # inferred as Inner

    # Nested Auto
    list_auto: KernelInvariant[list[Auto]]  # inferred as list[int32]

    # Auto in nested generic class with required constructor args
    # This mirrors ARTIQ's AD9910.cpld: CPLD[Auto] pattern
    dds: KernelInvariant[Auto]           # inferred as AD9910[RegIOUpdate]

    def __init__(self):
        self.core = Core()
        self.x_auto = int32(42)
        self.y_auto = int32(0)
        self.f_auto = 3.14
        self.s_auto = "hello"
        self.b_auto = True
        self.obj_auto = Inner(self.core, int32(99))
        self.list_auto = [int32(1), int32(2), int32(3)]

        cpld = CPLD(self.core, ProtoRev9(self.core))
        self.dds = AD9910(self.core, cpld, int32(4))

    @kernel
    def test_auto_int(self) -> int32:
        return self.x_auto

    @kernel
    def test_auto_mutable(self) -> int32:
        self.y_auto = int32(10)
        return self.y_auto

    @kernel
    def test_auto_float(self) -> float:
        return self.f_auto

    @kernel
    def test_auto_bool(self) -> bool:
        return self.b_auto

    @kernel
    def test_auto_object(self) -> int32:
        return self.obj_auto.get_value()

    @kernel
    def test_auto_list(self) -> int32:
        total: int32 = int32(0)
        for v in self.list_auto:
            total = total + v
        return total

    @kernel
    def test_auto_nested_generic(self) -> int32:
        """Test Auto inference on a class with required constructor args
        that itself contains CPLD[Auto]."""
        self.dds.init()
        return self.dds.get_chip_select()

    @kernel
    def run(self):
        x = self.test_auto_int()
        y = self.test_auto_mutable()
        f = self.test_auto_float()
        b = self.test_auto_bool()
        o = self.test_auto_object()
        l = self.test_auto_list()
        n = self.test_auto_nested_generic()


if __name__ == "__main__":
    AutoDemo().run()
