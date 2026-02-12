from __future__ import annotations

from min_artiq import *
from min_artiq import Auto
from typing import Generic, TypeVar
from numpy import int32, int64


@compile
class CPLDVersion:
    """Base class for version-specific CPLD implementations"""
    def __init__(self, cpld):
        self.cpld = cpld

    @kernel
    def cfg_write(self, cfg: int32):
        pass

    @kernel
    def cfg_att_en(self, channel: int32, on: bool):
        pass


@compile
class ProtoRev8(CPLDVersion):
    """Simulates ARTIQ's ProtoRev8 - with self-referential CPLD type."""

    # Self-referential type ProtoRev8 references CPLD[ProtoRev8]
    cpld: KernelInvariant[CPLD[ProtoRev8]]

    @kernel
    def cfg_write(self, cfg: int32):
        self.cpld.cfg_reg = cfg

    @kernel
    def cfg_att_en(self, channel: int32, on: bool):
        raise ValueError("cfg_att_en not supported on ProtoRev8")


@compile
class ProtoRev9(CPLDVersion):
    """Simulates ARTIQ's ProtoRev9 - with self-referential CPLD type."""
    # Self-referential type ProtoRev9 references CPLD[ProtoRev9]
    cpld: KernelInvariant[CPLD[ProtoRev9]]

    @kernel
    def cfg_write(self, cfg: int32):
        self.cpld.cfg_reg = cfg

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

    def __init__(self, core: Core, version_cls, proto_rev: int32 = int32(0x09)):
        self.core = core
        self.cfg_reg = int32(0)
        # version is created with self-reference
        if proto_rev == int32(0x08):
            self.version = ProtoRev8(self)
        else:
            self.version = ProtoRev9(self)

    @kernel
    def cfg_write(self, cfg: int32):
        self.version.cfg_write(cfg)

    @kernel
    def set_att_en(self, channel: int32, on: bool):
        self.version.cfg_att_en(channel, on)


@compile
class RegIOUpdate:
    """Simulates ARTIQ's RegIOUpdate - KEY: uses CPLD[Auto] not CPLD[ProtoRev9]."""
    # Auto in a non-generic class field
    cpld: KernelInvariant[CPLD[Auto]]
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
    """Simulates ARTIQ's AD9910 - generic over IO update type."""
    core: KernelInvariant[Core]
    # Auto in generic class field
    cpld: KernelInvariant[CPLD[Auto]]
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
class SUServo:
    """Simulates ARTIQ's SUServo - KEY: uses list[AD9910[Auto]] and list[CPLD[Auto]]."""
    core: KernelInvariant[Core]
    # list of generic types with Auto
    ddses: KernelInvariant[list[AD9910[Auto]]]
    cplds: KernelInvariant[list[CPLD[Auto]]]

    def __init__(self, core: Core, ddses, cplds):
        self.core = core
        self.ddses = ddses
        self.cplds = cplds

    @kernel
    def init(self):
        for i in range(len(self.cplds)):
            cpld = self.cplds[i]
            dds = self.ddses[i]
            cpld.cfg_write(int32(0))
            dds.init()


@compile
class Channel:
    """Simulates ARTIQ's SUServo Channel - KEY: uses AD9910[Auto]."""
    core: KernelInvariant[Core]
    servo: KernelInvariant[SUServo]
    # AD9910[Auto] in non-generic class
    dds: KernelInvariant[AD9910[Auto]]

    def __init__(self, servo: SUServo, channel: int32):
        self.core = servo.core
        self.servo = servo
        self.dds = servo.ddses[channel]

    @kernel
    def set_dds(self) -> int32:
        return self.dds.get_chip_select()


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

    servo: KernelInvariant[SUServo]
    channel: KernelInvariant[Channel]

    def __init__(self):
        self.core = Core()
        self.x_auto = int32(42)
        self.y_auto = int32(0)
        self.f_auto = 3.14
        self.s_auto = "hello"
        self.b_auto = True
        self.obj_auto = Inner(self.core, int32(99))
        self.list_auto = [int32(1), int32(2), int32(3)]

        cpld1 = CPLD(self.core, ProtoRev9, int32(0x09))
        cpld2 = CPLD(self.core, ProtoRev9, int32(0x09))
        dds1 = AD9910(self.core, cpld1, int32(4))
        dds2 = AD9910(self.core, cpld2, int32(5))
        self.dds = dds1

        self.servo = SUServo(self.core, [dds1, dds2], [cpld1, cpld2])
        self.channel = Channel(self.servo, int32(0))

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
    def test_suservo_list_auto(self):
        """Test list[AD9910[Auto]] and list[CPLD[Auto]] - mirrors ARTIQ SUServo."""
        self.servo.init()

    @kernel
    def test_channel_auto(self) -> int32:
        """Test AD9910[Auto] via Channel - mirrors ARTIQ SUServo Channel."""
        return self.channel.set_dds()

    @kernel
    def run(self):
        x = self.test_auto_int()
        y = self.test_auto_mutable()
        f = self.test_auto_float()
        b = self.test_auto_bool()
        o = self.test_auto_object()
        l = self.test_auto_list()
        n = self.test_auto_nested_generic()
        self.test_suservo_list_auto()
        c = self.test_channel_auto()


if __name__ == "__main__":
    AutoDemo().run()
