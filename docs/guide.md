# Developer Guide

Practical information for building, testing, debugging, and extending NAC3.

## Building

### With Nix

```
$ nix develop     # enter the dev shell (bash)
$ nix develop --command zsh      # or use your preferred shell
$ cargo build --release
```

The Nix flake provides LLVM, `clang-irrt`, and all other dependencies.

### PGO Build

The flake includes a profile-guided optimization (PGO) build for nac3artiq. PGO recompiles LLVM itself using profiling data collected from a real ARTIQ compilation, which improves codegen throughput.

```
$ nix build .#nac3artiq-pgo -L
```

The PGO pipeline has three stages, all handled automatically by Nix:

1. **Instrumented build** (`nac3artiq-instrumented`): builds nac3artiq against an instrumented LLVM that records branch frequency data during execution.
2. **Profile collection** (`nac3artiq-profile`): runs the instrumented compiler on the `nac3devices` ARTIQ example to produce `llvm.profdata`.
3. **PGO build** (`nac3artiq-pgo`): rebuilds LLVM with the collected profile applied, then builds nac3artiq against this optimized LLVM.

The intermediate packages can also be built individually if needed (e.g., `nix build .#nac3artiq-profile` to just collect profile data).

### IRRT Build

The `nac3core` build script (`build.rs`) compiles the C++ runtime under `nac3core/irrt/` to LLVM bitcode. If you modify IRRT sources, `cargo` will automatically rebuild. To inspect the generated IR:

```
$ DEBUG_DUMP_IRRT=1 cargo build -p nac3core
```

This writes `irrt.ll` and `irrt-filtered.ll` to the cargo output directory (printed by cargo as `OUT_DIR`).

## Running nac3standalone

The standalone compiler expects a Python file with a `run()` entry point:

```
$ cargo run --release -p nac3standalone -- my_program.py
```

This produces `module.o`. Link it against your runtime stubs (e.g., the demo `output_*` functions) to get an executable.

Useful flags:

- `-O0` / `-O2` / `-O3`: optimization level
- `--emit-llvm-ir`: write `main.ll` for each compilation stage
- `--emit-llvm-bc`: write `main.bc` (bitcode)
- `-T 0`: use all available threads for compilation

### Running demos

The `nac3standalone/demo/` directory contains example programs and a helper
script that compiles, links, and runs them in one step. From the demo directory:

```
$ cd nac3standalone/demo
$ ./run_demo.sh -- src/demo_test.py
```

`run_demo.sh` does three things:

1. Compiles the Python source with `nac3standalone`, producing `module.o`.
2. Compiles `demo.c` (the C runtime stubs for `output_int32`, `output_bool`, etc.) with clang.
3. Links both object files (plus `liblinalg.a` for linear algebra demos) into an executable and runs it.

Options:

- `--debug`: use the debug build of nac3standalone instead of release.
- `-i686`: cross-compile to 32-bit x86 (uses `--triple i686-unknown-linux-gnu` and links against the 32-bit linalg stub).
- `--out OUTFILE`: redirect the program output to a file instead of stdout.
- Extra nac3standalone flags can be passed after `--`: e.g., `./run_demo.sh -- --emit-llvm-ir src/demo_test.py`.

### Checking demos

`check_demos.sh` runs every `src/*.py` demo through both the Python interpreter and the NAC3 compiler, then diffs the output:

```
$ cd nac3standalone/demo
$ ./check_demos.sh
```

This is the same check that the Nix build runs. Pass `-i686` to also verify 32-bit output. Individual demos can be checked with `check_demo.sh`:

```
$ ./check_demo.sh src/demo_test.py
```

## Running nac3artiq + runkernel locally

For testing ARTIQ kernels without hardware, use `runkernel`. It provides stub implementations of `now_mu`, `at_mu`, `delay_mu`, `rtio_output`, and a few other ARTIQ syscalls.

The workflow:

1. Compile your kernel. nac3artiq produces `module.elf` (and optionally `debug.elf`) when invoked through the ARTIQ `Core.run()` method. The demo under `nac3artiq/demo/` shows the minimal setup, including `min_artiq.py` (a self-contained ARTIQ-like environment) and `device_db.py`.
2. Run through runkernel:
   ```
   $ cargo run --release -p runkernel -- module.elf
   ```
   `runkernel` loads the ELF, looks up `__modinit__`, and executes it. RTIO calls print their arguments so you can trace the output timeline.

### Running the demo

```
$ cd nac3artiq/demo
$ python demo.py
```

This uses `min_artiq.py` to set up the compiler, compiles the demo kernels, and produces `module.elf`. You can then run it with `runkernel` as above.

## Testing

```
$ cargo test      # all tests
$ cargo test -p nac3core      # core tests only
$ cargo test -p nac3parser    # parser tests only
```

## Extending the Compiler

### Adding a new type to codegen

The canonical pattern for adding type support in `codegen/types/`:

1. Create a new file (e.g., `codegen/types/mytype.rs`).
2. Define a struct that wraps the LLVM struct layout.
3. Implement `ProxyType` for it. This provides the interface for creating instances, accessing fields, and converting to/from LLVM values.
4. Register the type in `codegen/types/mod.rs`.
5. Add handling in `gen_expr` and `gen_stmt` where the type appears (attribute access, method calls, etc.).

### Adding a new builtin function

1. Add a variant to the `PrimDef` enum in `toplevel/helper.rs`.
2. In `make_primitives()` (same file), register the function's type signature with the `TopLevelComposer`.
3. If the function needs special type-checking logic (e.g., it accepts heterogeneous argument types returns a type derived from its arguments, or cannot be expressed as a simple signature), add a branch to `try_fold_special_call()` in `typecheck/type_inferencer/mod.rs`. This is where builtins like `len()`, `virtual()`, and NumPy array constructors perform their custom type inference.
4. Implement code generation. For simple functions, add a branch in `codegen/builtin_fns.rs`. For NumPy functions, use `codegen/numpy.rs`.
5. If the function needs custom calling conventions (like RPC), create a `GenCall` callback and assign it to the `TopLevelDef::Function`'s `codegen_callback` field.
6. Register the function in the frontend's builtin registry (`DefaultBuiltinRegistry` or `ArtiqBuiltinRegistry`).
