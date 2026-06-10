<div align="center">

![icon](https://git.m-labs.hk/M-Labs/nac3/raw/branch/master/nac3.svg)

</div>

# NAC3
NAC3 is a major rewrite of the compiler for the [ARTIQ](https://m-labs.hk/artiq) physics experiment control and data acquisition system. It features greatly improved compilation speeds, innovative memory management (CTRC), a much better type system, and more predictable and transparent operation. It has been introduced in ARTIQ version 10.

NAC3 has a modular design and its applicability reaches beyond ARTIQ. The ``nac3core`` module does not contain anything specific to ARTIQ, and can be used in any project that requires compiling Python to machine code.

## For developers

This repository contains:
- ``nac3ast``: Python abstract syntax tree definition (based on RustPython).
- ``nac3parser``: Python parser (based on RustPython).
- ``nac3core``: Core compiler library, containing type-checking and code generation.
- ``nac3standalone``: Standalone compiler tool (core language only).
- ``nac3binutils``: Contains binary tools (linker, symbolizer, etc.)
- ``nac3artiq``: Integration with ARTIQ and implementation of ARTIQ-specific extensions to the core language.
- ``runkernel``: Simple program that runs compiled ARTIQ kernels on the host and displays RTIO operations. Useful for testing without hardware.

Use ``nix develop`` in this repository to enter a development shell.
If you are using a different shell than bash you can use e.g. ``nix develop --command fish``.

Build NAC3 with ``cargo build --release``. See the demonstrations in ``nac3artiq`` and ``nac3standalone``.

### Pre-Commit Hooks

You are strongly recommended to use the provided pre-commit hooks to automatically reformat files and check for non-optimal Rust practices using Clippy. Run `pre-commit install` to install the hook and `pre-commit` will automatically run `cargo fmt` and `cargo clippy` for you.

Several things to note:

- If `cargo fmt` or `cargo clippy` returns an error, the pre-commit hook will fail. You should fix all errors before trying to commit again.
- If `cargo fmt` reformats some files, the pre-commit hook will also fail. You should review the changes and, if satisfied, try to commit again.
