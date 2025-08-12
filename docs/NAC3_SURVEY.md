# NAC3 Survey Report

## 1. Overview

NAC3 is a modern, Rust-based compiler for a specialized dialect of Python used by the ARTIQ (Advanced Real-Time Infrastructure for Quantum) physics experiment control system. Its main goals are to improve compilation speed, enhance type safety, and provide a more predictable compilation process compared to its predecessor.

## 2. Architecture and Core Components

The project is highly modular, with its components organized into several Rust crates:

*   **`nac3parser`**: This crate is responsible for parsing Python code. It uses the LALRPOP parser generator and is based on the parser from the RustPython project.
*   **`nac3ast`**: Defines the Abstract Syntax Tree (AST) for the parsed Python code. It uses a format compatible with CPython's `ASDL`.
*   **`nac3core`**: This is the heart of the compiler. It takes the AST from `nac3parser` and performs type-checking and code generation. It is designed to be a general-purpose Python-to-machine-code compiler, independent of the ARTIQ-specific parts of the project.
*   **`nac3ld`**: A custom linker for RISC-V and ARM architectures.
*   **`nac3artiq`**: Implements the ARTIQ-specific language features and integrates the compiler with the ARTIQ ecosystem.
*   **`nac3standalone`**: A command-line tool that provides a standalone version of the compiler for the core language.

## 3. Compilation Process and Backend

*   **Parsing**: The compilation process begins in `nac3parser`, where the Python source code is tokenized and parsed into an AST.
*   **Type Checking**: `nac3core` then performs extensive type-checking on the AST to ensure type safety.
*   **Code Generation**: After type-checking, `nac3core` generates machine code. **This is done using LLVM (version 16.0)**, via the `inkwell` Rust wrapper. The compiler supports generating code for x86, ARM, and RISC-V architectures.

## 4. Development History

The repository does not contain a public changelog or history file. A detailed report on the project's development timeline cannot be provided without access to the `git` history.

## Summary of Answers to Your Questions:

*   **How it parses Python?** It uses a LALRPOP-based parser in the `nac3parser` crate.
*   **How it compiles?** It uses the `nac3core` crate for type-checking and code generation.
*   **Does it rely on LLVM?** Yes, it uses LLVM 16 for code generation via the `inkwell` crate in Rust.
*   **What about its development history?** No information about its development history could be found in the repository.
