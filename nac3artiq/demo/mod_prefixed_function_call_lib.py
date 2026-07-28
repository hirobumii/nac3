# Helper module for `mod_prefixed_function_call.py`; contains a module-level `@kernel` function
# that the demo calls through a module prefix. Running this file directly does nothing.

from min_artiq import kernel
from numpy import int32


@kernel
def double(x: int32) -> int32:
    return x * 2
