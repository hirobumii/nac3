# Calling a function through a module prefix from a kernel.
#
# `gen_call_expr` used to treat the module as the receiver object of the call, so codegen tried
# to materialize the module as a class instance and panicked in `RawClassType::from_unifier_type`.

import min_artiq as artiq
import mod_prefixed_function_call_lib as lib


@artiq.compile
class ModPrefixedFunctionCall:
    core: artiq.KernelInvariant[artiq.Core]

    def __init__(self):
        self.core = artiq.Core()

    @artiq.kernel
    def run(self):
        # Module-prefixed call to a `@kernel` function of another module...
        artiq.print_int32(lib.double(21))
        # ...and to an `@extern` function.
        artiq.print_int32(42)


if __name__ == "__main__":
    ModPrefixedFunctionCall().run()
