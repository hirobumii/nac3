from inspect import getfullargspec, getmodule
from functools import wraps
from math import floor, ceil
from numpy import int32, int64, uint32, uint64, float64, bool_, str_, ndarray
from types import GenericAlias, ModuleType, SimpleNamespace
from typing import _GenericAlias, Generic, TypeVar

import nac3artiq


__all__ = [
    "Kernel", "KernelInvariant", "virtual", "ConstGeneric",
    "Option", "Some", "none", "UnwrapNoneError",
    "round64", "floor64", "ceil64",
    "extern", "kernel", "portable", "compile",
    "rpc", "ms", "us", "ns",
    "print_int32", "print_int64",
    "Core", "TTLOut",
    "parallel", "legacy_parallel", "sequential"
]


T = TypeVar('T')

class Kernel(Generic[T]):
    pass

class KernelInvariant(Generic[T]):
    pass

# The virtual class must exist before nac3artiq.NAC3 is created.
class virtual(Generic[T]):
    pass

class Option(Generic[T]):
    _nac3_option: T

    def __init__(self, v: T):
        self._nac3_option = v

    def is_none(self):
        return self._nac3_option is None

    def is_some(self):
        return not self.is_none()

    def unwrap(self):
        if self.is_none():
            raise UnwrapNoneError()
        return self._nac3_option

    def __repr__(self) -> str:
        if self.is_none():
            return "none"
        else:
            return "Some({})".format(repr(self._nac3_option))

    def __str__(self) -> str:
        if self.is_none():
            return "none"
        else:
            return "Some({})".format(str(self._nac3_option))

def Some(v: T) -> Option[T]:
    return Option(v)

none = Option(None)

class _ConstGenericMarker:
    pass

def ConstGeneric(name, constraint):
    return TypeVar(name, _ConstGenericMarker, constraint)

def round64(x):
    return round(x)

def floor64(x):
    return floor(x)

def ceil64(x):
    return ceil(x)


# Delay NAC3 analysis until all referenced variables are supposed to exist on the CPython side.
registered_functions = dict()
registered_classes = dict()

def register_function(fun):
    assert allow_registration
    registered_functions[fun] = getmodule(fun)

def register_class(cls):
    assert allow_registration
    registered_classes[cls] = getmodule(cls)


def extern(function):
    """Decorates a function declaration defined by the core device runtime."""
    register_function(function)
    return function


def rpc(arg=None, flags={}):
    """Decorates a function or method to be executed on the host interpreter."""
    if arg is None:
        def inner_decorator(function):
            return rpc(function, flags)
        return inner_decorator
    register_function(arg)
    return arg

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


def compile(cls):
    """
    Registers a class to be compiled by ARTIQ.
    All classes containing kernels or portable methods must use this decorator.
    """
    register_class(cls)
    return cls


import device_db
core_arguments = device_db.device_db["core"]["arguments"]

builtins = {
    "int": int,
    "float": float,
    "bool": bool,
    "str": str,
    "list": list,
    "tuple": tuple,
    "Exception": Exception,

    "types": {
        "GenericAlias": GenericAlias,
        "ModuleType": ModuleType,
    },

    "typing": {
        "_GenericAlias": _GenericAlias,
        "TypeVar": TypeVar,
    },

    "numpy": {
        "int32": int32,
        "int64": int64,
        "uint32": uint32,
        "uint64": uint64,
        "float64": float64,
        "bool_": bool_,
        "str_": str_,
        "ndarray": ndarray,
    },

    "artiq": {
        "Kernel": Kernel,
        "KernelInvariant": KernelInvariant,
        "_ConstGenericMarker": _ConstGenericMarker,
        "none": none,
        "virtual": virtual,
        "Option": Option,

        # Decorator functions
        "compile": compile,
        "extern": extern,
        "kernel": kernel,
        "portable": portable,
        "rpc": rpc,
    },
}

compiler = nac3artiq.NAC3(core_arguments["target"], builtins)
allow_registration = True


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


class EmbeddingMap:
    def __init__(self):
        self.object_inverse_map = {}
        self.object_map = {}
        self.string_map = {}
        self.string_reverse_map = {}
        self.function_map = {}
        self.attributes_writeback = []

    def store_function(self, key, fun):
        self.function_map[key] = fun
        return key

    def store_object(self, obj):
        obj_id = id(obj)
        if obj_id in self.object_inverse_map:
            return self.object_inverse_map[obj_id]
        key = len(self.object_map) + 1
        self.object_map[key] = obj
        self.object_inverse_map[obj_id] = key
        return key

    def store_str(self, s):
        if s in self.string_reverse_map:
            return self.string_reverse_map[s]
        key = len(self.string_map)
        self.string_map[key] = s
        self.string_reverse_map[s] = key
        return key

    def retrieve_function(self, key):
        return self.function_map[key]

    def retrieve_object(self, key):
        return self.object_map[key]

    def retrieve_str(self, key):
        return self.string_map[key]


@compile
class Core:
    ref_period: KernelInvariant[float]

    def __init__(self):
        self.ref_period = core_arguments["ref_period"]

    def run(self, method, *args, **kwargs):
        global allow_registration

        embedding = EmbeddingMap()

        if allow_registration:
            compiler.analyze(registered_functions, registered_classes, special_ids, set())
            allow_registration = False

        if hasattr(method, "__self__"):
            obj = method.__self__
            name = method.__name__
        else:
            obj = method
            name = ""

        compiler.compile_method_to_file(obj, name, args, "module.elf", embedding)

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


@compile
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

@compile
class KernelContextManager:
    @kernel
    def __enter__(self):
        pass

    @kernel
    def __exit__(self):
        pass

@compile
class UnwrapNoneError(Exception):
    """raised when unwrapping a none value"""
    artiq_builtin = True

parallel = KernelContextManager()
legacy_parallel = KernelContextManager()
sequential = KernelContextManager()

special_ids = {
    "parallel": id(parallel),
    "legacy_parallel": id(legacy_parallel),
    "sequential": id(sequential),
}
