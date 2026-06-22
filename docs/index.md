# NAC3 Developer Documentation

NAC3 is a Python-to-machine-code compiler. It compiles a statically-typed subset of Python to LLVM IR, for use in
[ARTIQ](https://m-labs.hk/artiq). The compiler is written in Rust and uses
[inkwell](https://github.com/TheDan64/inkwell) as its LLVM binding.

This documentation is intended for developers working on NAC3 itself. For user-facing language documentation, see the
[ARTIQ manual](https://m-labs.hk/artiq/manual/).

## Contents

- [Architecture](architecture.md) - Crate layout, compilation pipeline, and how the pieces fit together.
- [Code Generation](codegen.md) - LLVM IR generation, the `CodeGenerator` trait, parallel compilation, IRRT, and type layouts.
- [Developer Guide](guide.md) - Building, debugging, extending codegen/types, running nac3artiq locally, and common pitfalls.
- [IRRT](irrt.md) - The C++ IR Runtime: directory map, build process, conventions, Rust bindings, and the `call_extern!` pattern.
- [Reference Counting](refcounting.md) - Codegen-emitted reference counting: object layout, IRRT runtime functions, and the `@extern` ABI.
- [RPC](rpc.md) - Kernel↔host remote procedure calls: type tags, wire layout, argument marshalling, and return demarshalling.
