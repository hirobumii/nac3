# IRRT (IR Runtime)

IRRT (short for "IR Runtime") is a runtime support library that provides helper
functions for operations that are too complex or verbose to be directly built as
inline LLVM IR in the NAC3 compiler sources. It is located under
`nac3core/irrt/` and is written in C++20 as a header-only library.

The C++ sources are compiled during the build process to generate target-
independent LLVM IR, which is then embedded into the `nac3core` library, and is
linked into every compiled module at codegen time.

## 1. Organization

All C++ IRRT sources are located within `irrt/` in the `nac3core` crate. The
main translation unit is `irrt.cpp`, which includes all headers in directory
traversal order. All headers are located under the `irrt/irrt/` directory, with
subdirectories for specific domains.

The build script `build.rs` houses the logic for compiling the C++ IRRT sources
into LLVM IR. See [Build Script](#build-script) for more details.

### Header Organization

The following is a non-exhaustive list of key headers and their responsibilities
(all files are relative to `irrt/irrt/`):

| File/Directory | Responsibility |
|------|----------------|
| `cc-builtins.hpp` | Defines wrappers over C functions that are provided as C compiler builtins. |
| `cslice.hpp` | Defines a struct that contains a pointer and a length. |
| `debug.hpp` | Defines debugging utilities used in IRRT. |
| `exception.hpp` | Defines the `Exception` struct and related functions for exception handling. |
| `list.hpp` | Defines the `List` struct and related functions for list operations. |
| `math.hpp` | Defines mathematical helper functions, such as integer exponentiation. |
| `range.hpp` | Defines the `Range` struct and related functions for range operations. |
| `slice.hpp` | Defines the `Slice` struct and related functions for slice operations. |
| `string.hpp` | Defines the `String` struct and related functions for string operations. |
| `stdlib/` | Contains C++ standard library shims for a freestanding environment - The structure follows the standard library headers. |
| `ndarray/` | Contains IRRT struct definitions and utilities for NDArray operations. |
| `reference/` | Defines type information utilities and reference counting implementations. |

### File Conventions

In general, all headers (except those that live under `irrt/stdlib/`) follow
these conventions:

- All headers must contain a `#pragma once` guard.
- Data types (especially pointee types) should be annotated with `const` where
  applicable.
- Functions should be written to be `size_t`-agnostic where possible, using
  `size_t` (or its derivative types).
- For external functions that can be called via the `call_extern!` Rust macro: -
  The function must be declared in an `extern "C"` block. - The function name
  must be prefixed with `__nac3_` (e.g. `__nac3_slice_index_bound`). - It is
  strongly recommended to prefix the function name with a domain (e.g.
  `__nac3_slice_`) to group related functions together. - Within the `extern
  "C"` block, a `using namespace __nac3_impl;` statement should be used to pull
  the internal implementations into scope. Additional `using namespace`
  statements can be added as needed to pull in nested namespaces within the same
  namespace nesting.
- For internal C++ implementations: - All functions should be encapsulated
  within the `__nac3_impl` namespace to prevent symbol leakage and group
  implementations. - All functions and structs should be declared within an
  anonymous namespace inside `__nac3_impl` to give them internal linkage and
  minimize footprint of unused functions. - Headers present in nested
  directories (e.g. `irrt/irrt/ndarray/`) should further nest their contents in
  a nested namespace (e.g. `__nac3_impl::ndarray`). Additional namespaces can be
  added as needed. - Best C++ practices (such as those outlined in the [C++ Core
  Guidelines](https://isocpp.github.io/CppCoreGuidelines/ CppCoreGuidelines))
  should be followed where applicable. For instance, using `static_cast` instead
  of C-style casts. - All structs and functions should be documented with
  comments, stating the purpose of the function, its parameters and return
  value. Structs should additionally document its corresponding Rust-side
  `ProxyType` if applicable. - No standard library headers may be included - If
  needed, minimal shims should be implemented under `irrt/stdlib/`.
- `static_assert`s should be used where compile-time invariants need to be
  enforced, for instance `static_assert(__builtin_offsetof(S, a) == 0)` to
  ensure that the layout of a struct matches the expected layout on the Rust
  side.

C++ Standard library shims should follow these conventions to mirror the
conventions of the standard library:

- All standard library shims should be placed under the `irrt/stdlib/`
  directory, following the same structure as the standard library (e.g.
  `irrt/stdlib/cstdint.h`).
- All standard library shims should be located within the `__nac3_impl::stdlib`
  namespace.
- Each shim header should be named after the standard library header it is
  shimming, suffixed with `.h` (e.g. `cstdint.h`).
- Headers that are originally from C (e.g. `stdint.h`) should be implemented in
  their corresponding C++ header (`cstdint.h`).
- A minimal set of types and utilities required for IRRT should be implemented
  in the shim headers. Further utilities can be added as needed, but should be
  kept minimal to avoid unnecessary bloat.
- If a reference implementation is provided by `cppreference`, the
  implementation in the shim should be consistent with the reference
  implementation where applicable.

## 2. Build Process

This section describes how the C++ IRRT sources are compiled into LLVM IR and
embedded into the Rust codebase. This process is handled by the `build.rs`
script in the `nac3core` crate.

During the build process of `nac3core`, the `build.rs` script performs the
following steps:

1. **Compile C++ sources to LLVM IR**: The script invokes `clang-irrt` (a clang
   binary from the custom Nix `llvm-tools-irrt` package) to compile the C++
   sources in `irrt/` into LLVM IR.

Of note are the following flags:

   - `-fno-exceptions -fno-rtti`: Disables C++ exceptions and RTTI, as they are
     incompatible with the C ABI expected by the Rust bindings.
   - `-emit-llvm -S`: Emits LLVM IR in human-readable `.ll` format.
   - `-fno-discard-value-names` (debug only): Preserves the original variable
     names where possible
   - `-DIRRT_DEBUG_ASSERT` (debug only): Enables additional runtime checks in
     the IRRT code.
   - `-Xclang -disable-O0-optnone` (debug only): Disables the default `optnone`
     function attribute, which prevents code generation that is not supported by
     the linker of `nac3binutils`.

The compilation processes utilizes `wasm` as its compilation target as it
produces pointer-width-specific but otherwise portable IR, which is necessary
for supporting both 32-bit and 64-bit targets (see the dual-width hack below).

2. **Filter and post-process the LLVM IR**: Since the raw LLVM IR is generated
   with `wasm` as its target platform and therefore contains IR directives
   specific to LLVM's Wasm backend, the build script applies a regex filter to
   only keep necessary portions of the IR.

This step comprises of two operations: The necessary declarations of the IR are
retained using a regex filter (see `build.rs` for the list of retained
declarations), then the target-specific sections of the IR are trimmed. The
result of this is a portable variant of LLVM IR that is stripped of all target-
specific features.

3. **Assemble LLVM IR into LLVM bitcode**: To reduce the size of the embedded IR
   and speed up parsing at codegen time, the filtered LLVM IR is assembled into
   LLVM bitcode using `llvm-as` (also from the `llvm-tools-irrt` package).

The LLVM bitcode is automatically regenerated whenever the C++ sources in
`irrt/` are modified.

To support both 32-bit and 64-bit targets, the build script executes the above
steps twice with different target flags (`wasm32` and `wasm64`) and generates
two variants of the IRRT LLVM IR. Both variants are embedded into the Rust
codebase, and the appropriate one is selected when `nac3core` is executed based
on the compilation target.

## 3. Usage in Rust

### Embedding IRRT into `nac3core`

The `load_irrt` function in `codengen/irrt/mod.rs` is responsible for loading
the appropriate variant of the IRRT LLVM bitcode based on the compilation
target. The LLVM bitcode is embedded via the use of the `include_bytes!` macro.

When `load_irrt` is called, it performs the following steps:

1. It resolves the size of `size_t` on the target platform, and loads the
   appropriate IRRT bitcode file as an in-memory LLVM module.
2. It applies attributes to the IRRT functions to ensure that they are inlined
   where appropriate.
3. It initializes the exception ID globals in the IRRT module using the
   `SymbolResolver` provided by the caller.
4. It returns the processed IRRT module to the caller, which is used to link
   with the other modules generated by the NAC3 compiler.

### Using IRRT Functions

Since extern functions in IRRT are implemented as normal function with the C
ABI, they can be called from Rust using the `call_extern!` macro defined in
`codegen/expr.rs`.

The general syntax of the `call_extern!` macro is as follows:

```rust
call_extern!(ctx: <ret_type> <name?> = [attrs] "<symbol>"(args…))
```

- `ctx` - `&mut CodeGenContext`.
- `<ret_type>` - LLVM type of the return value, or `void`.
- `<name?>` - local name for the result (`_` can be used to ignore the result).
- `[attrs]` - optional attribute list (e.g. `["nounwind"]`).
- `"<symbol>"` - the symbol name of the extern function to call in `&str`.
- `(args…)` - the arguments to pass to the extern function; should already be in
  the correct LLVM type.

See the documentation for `call_extern!` in `codegen/expr.rs` for more details
and examples.

## 4. C++ Structs and Rust `ProxyType`s

The IRRT sources contain C++ struct definitions that correspond to Rust
`ProxyType`s used in the compiler sources. These structs are defined in the IRRT
headers and are used to represent data structures such as `ObjectHeader`,
`List`, `NDArray`, etc. in the IRRT sources.

| C++ struct | Rust type(s) | C++ source |
|------------|-------------|------------|
| `reference::ObjectHeader` | `ObjectHeader{Type,Value}` | `reference/header.hpp` |
| `reference::Array<T>` | `RefCountedArray{Type,Value}` | `reference/array.hpp` |
| `reference::Typeinfo` | `Typeinfo{Type,Value}` | `reference/typeinfo.hpp` |
| `ndarray::NDArray` | `NDArray{Type,Value}` | `ndarray/def.hpp` |
| `List` | `List{Type,Value}` | `list.hpp` |
| `Exception` | `Exception{Type,Value}` | `exception.hpp` |
| `String` | `String{Type,Value}` | `string.hpp` |

Additionally, the IRRT sources also define a marker symbol `__nac3_global_begin`
(declared in `reference/header.hpp`; corresponding to
`ModuleContext::global_begin_ptr`) that acts as an anchor address for
NAC3-related globals.

The exactly type mapping of C++ struct fields to their corresponding `ProxyType`
`#[value_type]` annotation are as follows:

- Integer types are mapped to their corresponding fixed-width integer types
  (e.g. `int32_t` → `i32`, `size_t` → `usize`).
- Pointer types are mapped to `ptr`
- Structure types are mapped to their corresponding inline type
  (`ctx.module.get_struct_type(...)`).

## 5. Debugging

### Debugging IRRT

To utilize the most debugging information present in the IRRT, you are
recommended to compile `nac3core` using the debug profile to maximize the amount
of information preserved in the generated LLVM IR. When compiling in debug mode,
the generated LLVM IR will contain additional metadata and COMDAT entries, and
can also be utilized by debugging utilities such as `gdb` and `valgrind`.

Additionally, the `IRRT_DEBUG_ASSERT` macro can be enabled in release builds to
guard additional runtime checks in IRRT (it is enabled by default in debug
builds), which can help catch out-of-bounds errors and other issues in the IRRT
code.

To inspect the generated LLVM IR for IRRT, you can set the `DEBUG_DUMP_IRRT`
environment variable to `1` when building `nac3core`. This will cause the build
script to dump the raw and filtered per-target `.ll` files to `$OUT_DIR`. It is
recommended to clean the build directory before building with this option
enabled to ensure that the dumped files are up-to-date and there are no stale
incremental build files.

Print debugging is available on the C++ side by declaring either `printf` (for
`nac3standalone`), `core_log` or `rtio_log` (for `nac3artiq`) as extern
functions in the relevant IRRT header, and then calling them from the C++ code.
Refer to the corresponding documentation for these functions for details on how
to use them.
