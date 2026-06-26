# Code Generation

This document covers the internals of `nac3core`'s type system and code generation pipeline. It is meant to orient developers on the critical types and the flow from typed AST to LLVM IR; the fine details live in Rustdoc comments on the relevant structs and functions.

## Type System

### Types and the Unifier

`Type` is a `UnificationKey`: a handle into the unification table. It is not a type description by itself. To inspect what a `Type` actually represents, look it up through the `Unifier`:

```rust
let ty_enum: &TypeEnum = &*unifier.get_ty(some_type);
match ty_enum {
    TypeEnum::TObj { obj_id, fields, params, .. } => { /* ... */ }
    TypeEnum::TFunc(sig) => { /* ... */ }
    // ...
}
```

The `Unifier` owns a `UnificationTable` implementing union-find. Unification merges two types by constraining them to be equal; if the constraint is contradictory, a `TypeError` is returned. During type inference, `TVar` nodes start unconstrained (or range-constrained) and get progressively pinned down as the inferencer processes the AST.

`SharedUnifier` (`Arc<Mutex<(UnificationTable, u32, Vec<Call>)>>`) is used when unifiers need to be shared across threads; each module gets its own unifier during analysis, and the shared form is stored in `TopLevelContext::unifiers`.

### PrimitiveStore

`PrimitiveStore` holds `Type` handles for all builtin primitive types (`int32`, `int64`, `uint32`, `uint64`, `float`, `bool`, `str`, `none`, `exception`, `option`, `ndarray`, etc.). It is created once during `TopLevelComposer` initialization and threaded through the entire pipeline.

### TopLevelDef and DefinitionId

Every class, function, and module registered with the compiler gets a `TopLevelDef` entry and a `DefinitionId` (index into the definition list).

`TopLevelDef::Function` has two important maps:

- `instance_to_symbol`: maps a string key (derived from concrete type variable bindings) to the LLVM symbol name for that instantiation.
- `instance_to_stmt`: maps the same key to a `FunInstance` containing the typed AST body, call site information, and type substitutions.

When a generic function is called with specific type arguments, the codegen looks up (or creates) an entry in these maps. If a new entry is created, a new `CodeGenTask` is queued.

`TopLevelDef::Function` can also carry a `codegen_callback` (`GenCall`), which entirely replaces normal code generation for that function. nac3artiq uses this for RPC functions, where instead of compiling a function body, the generated code serializes arguments and calls into the ARTIQ RPC runtime.

## Monomorphization

NAC3 compiles generic functions by monomorphization: each distinct combination of concrete type arguments produces a separate LLVM function. The `ConcreteTypeStore` manages this mapping.

The flow:

1. During codegen, a call to a generic function triggers `gen_func_instance()`.
2. The type variable bindings are collected into a substitution key (a sorted string of variable ID/type pairs).
3. If `instance_to_symbol` already has this key, the existing symbol is reused.
4. Otherwise a new `CodeGenTask` is created with the concrete substitutions and placed on the `WorkerRegistry` queue.

Because workers run in parallel, `gen_func_instance()` must handle the race where two workers try to instantiate the same function simultaneously. The default implementation uses the lock on the `TopLevelDef` to serialize this check.

## CodeGenContext

`CodeGenContext` is the per-function state during IR generation. It holds:

- **`builder`**: the LLVM `Builder` for emitting instructions.
- **`var_assignment`**: maps variable names to `VarValue` (an LLVM pointer plus an optional `StaticValue` for compile-time-known values).
- **`type_cache` / `alloca_type_cache`**: caches `Type` to LLVM `BasicTypeEnum` conversions. `alloca_type_cache` is specifically for in-memory representations (e.g., `bool` is `i8` in memory but `i1` in the ABI).
- **`loop_target`**: `(header, exit)` basic blocks for the current loop, used by `break`/`continue`.
- **`unwind_target`**: the landing pad for exception handling.
- **`return_buffer`** / **`return_target`**: for functions that need a single return point (e.g., when exception cleanup is involved).

`CodeGenContext` derefs to `ModuleContext`, which provides access to the LLVM `Context`, `Module`, target-specific integer types (`i32`, `i64`, `size_t`), and the type context for converting nac3 types to LLVM types.

## Expression and Statement Generation

Expression codegen (`codegen/expr.rs`) and statement codegen (`codegen/stmt.rs`) are the two largest files in the codebase. They follow the AST structure closely:

- `gen_expr()` dispatches on `ExprKind` and returns an `RtValue` (a pair of `Type` and an optional LLVM value).
- `gen_stmt()` dispatches on `StmtKind` and returns `()` (control flow is handled through the builder's current basic block).

Both are implemented as free functions that take a `&mut dyn CodeGenerator` and `&mut CodeGenContext`. The `CodeGenerator` trait methods delegate to these free functions by default, letting frontends override specific behaviors without duplicating the rest.

## Parallel Compilation

`WorkerRegistry` manages a pool of codegen worker threads. Each worker:

1. Receives `CodeGenTask` items from a shared channel.
2. Creates (or reuses) a `ModuleContext` with its own LLVM `Context`.
3. Calls `gen_func_impl()` to generate the function body.
4. When the function calls another function that needs a new instantiation, the worker calls `registry.add_task()` to queue it.
5. After finishing a task, writes the module bitcode to a buffer and signals completion.

The registry tracks outstanding tasks with a counter and a condvar. When all tasks are done, the main thread collects the per-worker LLVM bitcode buffers, links them into one module, and proceeds with optimization.

Workers are created with `WorkerRegistry::create_workers()`, which takes a `Vec<Box<G>>` of `CodeGenerator` instances (one per thread). This is where the frontend passes in its custom generator type.

## Type Layouts

The `codegen/types/` directory contains proxy types that map nac3 types to LLVM struct layouts. Each proxy type implements `ProxyType` and provides methods for accessing fields, creating instances, and generating related operations.

The important proxy types:

- `ListType`: a `{ptr, len}` struct. The pointer references a heap-allocated array of elements.
- `NDArrayType`: the representation of `numpy.ndarray`. Contains data pointer, number of dimensions, shape array, and strides. Broadcasting and indexing operations are in `codegen/types/ndarray/`.
- `StringType`: a `{ptr, len}` pair for UTF-8 data.
- `RangeType`: `{start, stop, step}` integers.
- `TupleType`: an LLVM struct with one field per element.
- `ExceptionType`: carries exception class ID, message, parameters, and source location fields.
- `OptionType`: a tagged union with a flag byte and optional value.

## Exception Handling

NAC3 uses LLVM's `landingpad`-based exception handling with a personality function. The personality symbol is set via `TopLevelContext::personality_symbol` (nac3artiq sets this to `__nac3_personality`).

The flow for a `try`/`except` block:

1. `gen_stmt` for `Try` sets `ctx.unwind_target` to a landing pad block.
2. Calls within the `try` body are emitted as `invoke` instructions targeting both a normal continuation and the landing pad.
3. The landing pad dispatches on exception class ID to the matching `except` clause.
4. `raise` compiles to a call to `__nac3_raise` followed by `unreachable`.

Each exception class is assigned a numeric ID via `SymbolResolver::get_exception_id()`, and the IRRT uses `SymbolResolver::get_string_id()` for exception name strings.

## IRRT Functions

When you need a runtime helper that is too complex for inline LLVM IR, add it to the IRRT (`nac3core/irrt/`). The C++ source is compiled to target-independent LLVM bitcode and linked into every compilation. See `irrt/irrt.cpp` and the submodule headers.

To call an IRRT function from Rust codegen, declare it in the appropriate `codegen/irrt/*.rs` module and call it through the LLVM builder. Alternatively, use the `call_extern!` macro directly if there is only one call site to the IRRT function.

## Builtin Functions

Builtin functions (e.g., `int32()`, `len()`, `range()`, `np_zeros()`) are registered during `TopLevelComposer` initialization. The `PrimDef` enum in `toplevel/helper.rs` lists every builtin type and function.

Most builtins have their code generation in `codegen/builtin_fns.rs`. NumPy operations are in `codegen/numpy.rs`. The implementations receive `CodeGenContext` and the call arguments, and return the result as LLVM values.

When adding a new builtin:

1. Add a variant to `PrimDef`.
2. Register the type and signature in `make_primitives()`.
3. Write the codegen implementation.
4. If the builtin needs a `GenCall` callback (because it requires custom calling conventions), set `codegen_callback` on the `TopLevelDef::Function`.
