from inspect import getfullargspec, isclass, getmro
from functools import wraps
from types import SimpleNamespace
from numpy import int32, int64
from typing import Generic, TypeVar, get_origin
from math import floor, ceil

import nac3artiq


__all__ = [
    "KernelInvariant", "Kernel", "virtual",
    "round64", "floor64", "ceil64",
    "extern", "kernel", "portable", "nac3",
    "ms", "us", "ns",
    "print_int32", "print_int64",
    "Core", "TTLOut",
    "parallel", "sequential"
]


T = TypeVar('T')

class KernelInvariant(Generic[T]):
    pass

class Kernel(Generic[T]):
    pass

# The virtual class must exist before nac3artiq.NAC3 is created.
class virtual(Generic[T]):
    pass

def round64(x):
    return round(x)

def floor64(x):
    return floor(x)

def ceil64(x):
    return ceil(x)


import device_db
core_arguments = device_db.device_db["core"]["arguments"]

compiler = nac3artiq.NAC3(core_arguments["target"])
allow_registration = True
allow_kernel_read = False
# Delay NAC3 analysis until all referenced variables are supposed to exist on the CPython side.
registered_functions = set()
registered_classes = set()

def register_function(fun):
    assert allow_registration
    registered_functions.add(fun)

def register_class(cls):
    assert allow_registration
    registered_classes.add(cls)


def extern(function):
    """Decorates a function declaration defined by the core device runtime."""
    register_function(function)
    return function


def kernel(function_or_method):
    """Decorates a function or method to be executed on the core device."""
    register_function(function_or_method)
    argspec = getfullargspec(function_or_method)
    if argspec.args and argspec.args[0] == "self":
        @wraps(function_or_method)
        def run_on_core(self, *args, **kwargs):
            fake_method = SimpleNamespace(__self__=self, __name__=function_or_method.__name__)
            self.core.run(fake_method, *args, **kwargs)
    else:
        @wraps(function_or_method)
        def run_on_core(*args, **kwargs):
            raise RuntimeError("Kernel functions need explicit core.run()")
    return run_on_core


def portable(function):
    """Decorates a function or method to be executed on the same device (host/core device) as the caller."""
    register_function(function)
    return function


def nac3(cls):
    """
    Decorates a class to be analyzed by NAC3.
    All classes containing kernels or portable methods must use this decorator.
    """
    
    # python does not allow setting magic method on specific instances 
    # (https://docs.python.org/3/reference/datamodel.html#special-method-lookup).
    # use this set to keep track of those custom class instances that are
    # assigned to the `Kernel` fields of a class
    cls.__nac3_kernel_only_instances__ = set()

    def apply_kernel_only_constraints(val):
        kernel_only_set = getattr(type(val), '__nac3_kernel_only_instances__', None)
        if kernel_only_set is None:
            return
        else:
            for (_, attr_val) in val.__dict__.items():
                if not (attr_val == val):
                    apply_kernel_only_constraints(attr_val)
            kernel_only_set.add(val)

    if not isclass(cls):
        raise ValueError("nac3 annotation decorator should only be applied to classes")
    if not cls.__setattr__ in {base.__setattr__ for base in cls.__bases__}:
        raise ValueError("custom __setattr__ is not supported in kernel classes")
    
    register_class(cls)

    immutable_fields = {
        n for b in getmro(cls)
            for (n, ty) in b.__dict__.get('__annotations__', {}).items() if get_origin(ty) == Kernel
    }
    def __setattr__(obj, key, value):
        if obj in type(obj).__nac3_kernel_only_instances__:
            raise TypeError("attempting to write to kernel only variable")
        # should allow init to set value, if no attribute then allow to set attr, then
        # recursively apply constraint to all the fields of that specific object,
        # regardless of whether they are marked with `Kernel` or not
        if key in immutable_fields:
            if hasattr(obj, key):
                raise TypeError("attempting to write to kernel only variable")
            else:
                apply_kernel_only_constraints(value)
        object.__setattr__(obj, key, value)

    def __getattribute__(obj, key):        
        # need to use `object.__getattribute__` to get attr before checking
        # the key in immutable_fields for `__init__`.
        # since that in `__init__` when setting a instance variable like `self.a = 3`
        # the sequence of internal magic call is still calling cls.__getattribute__(self, 'a')
        # first, and if only "AttributeError" is raised, it will then call `__setattr__`
        # if we raise `TypeError` too early, python will just halt at this `TypeError`.
        attr = object.__getattribute__(obj, key)
        if not allow_kernel_read:
            if obj in type(obj).__nac3_kernel_only_instances__:
                raise TypeError("attempting to read kernel only variable")
            if key in immutable_fields:
                raise TypeError("attempting to read kernel only variable")
        return attr
    
    cls.__setattr__ = __setattr__
    cls.__getattribute__ = __getattribute__

    return cls


ms = 1e-3
us = 1e-6
ns = 1e-9

@extern
def rtio_init():
    raise NotImplementedError("syscall not simulated")


@extern
def rtio_get_counter() -> int64:
    raise NotImplementedError("syscall not simulated")


@extern
def rtio_output(target: int32, data: int32):
    raise NotImplementedError("syscall not simulated")


@extern
def rtio_input_timestamp(timeout_mu: int64, channel: int32) -> int64:
    raise NotImplementedError("syscall not simulated")


@extern
def rtio_input_data(channel: int32) -> int32:
    raise NotImplementedError("syscall not simulated")


# These is not part of ARTIQ and only available in runkernel. Defined here for convenience.
@extern
def print_int32(x: int32):
    raise NotImplementedError("syscall not simulated")


@extern
def print_int64(x: int64):
    raise NotImplementedError("syscall not simulated")


@nac3
class Core:
    ref_period: KernelInvariant[float]

    def __init__(self):
        self.ref_period = core_arguments["ref_period"]

    def run(self, method, *args, **kwargs):
        global allow_kernel_read
        global allow_registration
        if allow_registration:
            compiler.analyze(registered_functions, registered_classes)
            allow_registration = False

        if hasattr(method, "__self__"):
            obj = method.__self__
            name = method.__name__
        else:
            obj = method
            name = ""
        allow_kernel_read = True
        compiler.compile_method_to_file(obj, name, args, "module.elf")
        allow_kernel_read = False

    @kernel
    def reset(self):
        rtio_init()
        at_mu(rtio_get_counter() + int64(125000))

    @kernel
    def break_realtime(self):
        min_now = rtio_get_counter() + int64(125000)
        if now_mu() < min_now:
            at_mu(min_now)

    @portable
    def seconds_to_mu(self, seconds: float) -> int64:
        return int64(round(seconds/self.ref_period))

    @portable
    def mu_to_seconds(self, mu: int64) -> float:
        return float(mu)*self.ref_period

    @kernel
    def delay(self, dt: float):
        delay_mu(self.seconds_to_mu(dt))


@nac3
class TTLOut:
    core: KernelInvariant[Core]
    channel: KernelInvariant[int32]
    target_o: KernelInvariant[int32]

    def __init__(self, core: Core, channel: int32):
        self.core = core
        self.channel = channel
        self.target_o = channel << 8

    @kernel
    def output(self):
        pass

    @kernel
    def set_o(self, o: bool):
        rtio_output(self.target_o, 1 if o else 0)

    @kernel
    def on(self):
        self.set_o(True)

    @kernel
    def off(self):
        self.set_o(False)

    @kernel
    def pulse_mu(self, duration: int64):
        self.on()
        delay_mu(duration)
        self.off()

    @kernel
    def pulse(self, duration: float):
        self.on()
        self.core.delay(duration)
        self.off()

@nac3
class KernelContextManager:
    @kernel
    def __enter__(self):
        pass

    @kernel
    def __exit__(self):
        pass

parallel = KernelContextManager()
sequential = KernelContextManager()
