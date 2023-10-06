#!/usr/bin/env python3

import sys
import importlib.util
import importlib.machinery
import numpy as np
import pathlib
import scipy

from numpy import int32, int64, uint32, uint64
from typing import TypeVar, Generic

T = TypeVar('T')
class Option(Generic[T]):
    _nac3_option: T

    def __init__(self, v: T):
        self._nac3_option = v

    def is_none(self):
        return self._nac3_option is None
    
    def is_some(self):
        return not self.is_none()
    
    def unwrap(self):
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

def patch(module):
    def dbl_nan():
        return np.nan

    def dbl_inf():
        return np.inf

    def output_asciiart(x):
        if x < 0:
            sys.stdout.write("\n")
        else:
            sys.stdout.write(" .,-:;i+hHM$*#@  "[x])

    def output_float(x):
        print("%f" % x)

    def dbg_stack_address(_):
        return 0

    def extern(fun):
        name = fun.__name__
        if name == "dbl_nan":
            return dbl_nan
        elif name == "dbl_inf":
            return dbl_inf
        elif name == "output_asciiart":
            return output_asciiart
        elif name == "output_float64":
            return output_float
        elif name in {
            "output_bool",
            "output_int32",
            "output_int64",
            "output_int32_list",
            "output_uint32",
            "output_uint64",
            "output_str",
        }:
            return print
        elif name == "dbg_stack_address":
            return dbg_stack_address
        else:
            raise NotImplementedError

    module.int32 = int32
    module.int64 = int64
    module.uint32 = uint32
    module.uint64 = uint64
    module.TypeVar = TypeVar
    module.Generic = Generic
    module.extern = extern
    module.Option = Option
    module.Some = Some
    module.none = none

    # NumPy Math functions
    module.isnan = np.isnan
    module.isinf = np.isinf
    module.sin = np.sin
    module.cos = np.cos
    module.exp = np.exp
    module.exp2 = np.exp2
    module.log = np.log
    module.log10 = np.log10
    module.log2 = np.log2
    module.fabs = np.fabs
    module.floor = np.floor
    module.ceil = np.ceil
    module.trunc = np.trunc
    module.sqrt = np.sqrt
    module.rint = np.rint
    module.tan = np.tan
    module.arcsin = np.arcsin
    module.arccos = np.arccos
    module.arctan = np.arctan
    module.sinh = np.sinh
    module.cosh = np.cosh
    module.tanh = np.tanh
    module.arcsinh = np.arcsinh
    module.arccosh = np.arccosh
    module.arctanh = np.arctanh
    module.expm1 = np.expm1
    module.cbrt = np.cbrt
    module.arctan2 = np.arctan2
    module.copysign = np.copysign
    module.fmax = np.fmax
    module.fmin = np.fmin
    module.ldexp = np.ldexp
    module.hypot = np.hypot
    module.nextafter = np.nextafter

    # SciPy Math Functions
    module.erf = scipy.special.erf
    module.erfc = scipy.special.erfc
    module.gamma = scipy.special.gamma
    module.gammaln = scipy.special.gammaln
    module.j0 = scipy.special.j0
    module.j1 = scipy.special.j1


def file_import(filename, prefix="file_import_"):
    filename = pathlib.Path(filename)
    modname = prefix + filename.stem

    path = str(filename.resolve().parent)
    sys.path.insert(0, path)

    try:
        spec = importlib.util.spec_from_loader(
            modname,
            importlib.machinery.SourceFileLoader(modname, str(filename)),
        )
        module = importlib.util.module_from_spec(spec)
        patch(module)
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(path)

    return module


def main():
    demo = file_import(sys.argv[1])
    demo.run()


if __name__ == "__main__":
    main()
