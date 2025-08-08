#!/usr/bin/env python3

import sys
import importlib.util
import importlib.machinery
import math
import numpy as np
import numpy.typing as npt
import scipy as sp
import pathlib

from numpy import int32, int64, uint32, uint64
from scipy import special
from typing import TypeVar, Generic, Literal, Union

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

class _ConstGenericMarker:
    pass

def ConstGeneric(name, constraint):
    return TypeVar(name, _ConstGenericMarker, constraint)

N = TypeVar("N", bound=np.uint64)
class _NDArrayDummy(Generic[T, N]):
    pass

# https://stackoverflow.com/questions/67803260/how-to-create-a-type-alias-with-a-throw-away-generic
NDArray = Union[npt.NDArray[T], _NDArrayDummy[T, N]]

def _bool(x):
    if isinstance(x, np.ndarray):
        return np.bool_(x)
    else:
        return bool(x)

def _float(x):
    if isinstance(x, np.ndarray):
        return np.float64(x)
    else:
        return float(x)

def round_away_zero(x):
    if isinstance(x, np.ndarray):
        return np.vectorize(round_away_zero)(x)
    else:
        if x >= 0.0:
            return math.floor(x + 0.5)
        else:
            return math.ceil(x - 0.5)

def _floor(x):
    if isinstance(x, np.ndarray):
        return np.vectorize(_floor)(x)
    else:
        return math.floor(x)

def _ceil(x):
    if isinstance(x, np.ndarray):
        return np.vectorize(_ceil)(x)
    else:
        return math.ceil(x)

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

    def output_strln(x):
        print(x, end='')

    def output_int32_list(x):
        print([int(e) for e in x])

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
        elif name == "output_str":
            return output_strln
        elif name == "output_int32_list":
            return output_int32_list
        elif name in {
            "output_bool",
            "output_int32",
            "output_int64",
            "output_uint32",
            "output_uint64",
            "output_strln",
            "output_range",
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
    module.bool = _bool
    module.float = _float
    module.TypeVar = TypeVar
    module.ConstGeneric = ConstGeneric
    module.Generic = Generic
    module.Literal = Literal
    module.extern = extern
    module.Option = Option
    module.Some = Some
    module.none = none

    # Builtin Math functions
    module.round = round_away_zero
    module.round64 = round_away_zero
    module.np_round = np.round
    module.floor = _floor
    module.floor64 = _floor
    module.np_floor = np.floor
    module.ceil = _ceil
    module.ceil64 = _ceil
    module.np_ceil = np.ceil

    # NumPy NDArray factory functions
    module.ndarray = NDArray
    module.np_ndarray = np.ndarray
    module.np_empty = np.empty
    module.np_zeros = np.zeros
    module.np_ones = np.ones
    module.np_full = np.full
    module.np_eye = np.eye
    module.np_identity = np.identity
    module.np_array = np.array
    module.np_arange = np.arange

    # NumPy NDArray view functions
    module.np_broadcast_to = np.broadcast_to
    module.np_transpose = np.transpose
    module.np_reshape = np.reshape

    # NumPy NDArray property getters
    module.np_size = np.size
    module.np_shape = np.shape
    module.np_strides = lambda ndarray: ndarray.strides

    # NumPy Math functions
    module.np_isnan = np.isnan
    module.np_isinf = np.isinf
    module.np_min = np.min
    module.np_minimum = np.minimum
    module.np_argmin = np.argmin
    module.np_max = np.max
    module.np_maximum = np.maximum
    module.np_argmax = np.argmax
    module.np_sin = np.sin
    module.np_cos = np.cos
    module.np_exp = np.exp
    module.np_exp2 = np.exp2
    module.np_log = np.log
    module.np_log10 = np.log10
    module.np_log2 = np.log2
    module.np_fabs = np.fabs
    module.np_trunc = np.trunc
    module.np_sqrt = np.sqrt
    module.np_rint = np.rint
    module.np_tan = np.tan
    module.np_arcsin = np.arcsin
    module.np_arccos = np.arccos
    module.np_arctan = np.arctan
    module.np_sinh = np.sinh
    module.np_cosh = np.cosh
    module.np_tanh = np.tanh
    module.np_arcsinh = np.arcsinh
    module.np_arccosh = np.arccosh
    module.np_arctanh = np.arctanh
    module.np_expm1 = np.expm1
    module.np_cbrt = np.cbrt
    module.np_arctan2 = np.arctan2
    module.np_copysign = np.copysign
    module.np_fmax = np.fmax
    module.np_fmin = np.fmin
    module.np_ldexp = np.ldexp
    module.np_hypot = np.hypot
    module.np_nextafter = np.nextafter
    module.np_any = np.any
    module.np_all = np.all

    # SciPy Math functions
    module.sp_spec_erf = special.erf
    module.sp_spec_erfc = special.erfc
    module.sp_spec_gamma = special.gamma
    module.sp_spec_gammaln = special.gammaln
    module.sp_spec_j0 = special.j0
    module.sp_spec_j1 = special.j1

    # Linalg functions
    module.np_dot = np.dot
    module.np_linalg_cholesky = np.linalg.cholesky
    module.np_linalg_qr = np.linalg.qr
    module.np_linalg_svd = np.linalg.svd
    module.np_linalg_inv = np.linalg.inv
    module.np_linalg_pinv = np.linalg.pinv
    module.np_linalg_matrix_power = np.linalg.matrix_power
    module.np_linalg_det = np.linalg.det
    
    module.sp_linalg_lu = lambda x: sp.linalg.lu(x, True)
    module.sp_linalg_schur = sp.linalg.schur
    module.sp_linalg_hessenberg = lambda x: sp.linalg.hessenberg(x, True)

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
