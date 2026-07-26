# RPC (Remote Procedure Calls)

RPC is the mechanism by which a compiled ARTIQ kernel calls back into the host
Python runtime. It is ARTIQ-specific and lives almost entirely in
`nac3artiq/src/`:

- `codegen.rs` - all marshalling and codegen (the bulk of the implementation).
- `lib.rs` - `@rpc` decorator detection and callback registration.
- `symbol_resolver.rs` - host-object tracking for attributes writeback.

## 1. Overview

There are two kinds of RPC:

- **Synchronous (`@rpc`)**: The kernel sends a call, blocks until the return
  value is available, then demarshals and continues with the result.
- **Asynchronous (`@rpc(async)`)**: Fire-and-forget; the kernel sends the call
  without waiting for a return value.

When a function is decorated with `@rpc`, the `nac3artiq` frontend detects this
during top-level composition and registers an `rpc_codegen_callback` as the
function's codegen callback. When codegen encounters a call to an RPC function,
this callback runs in place of compiling a normal function body.

The callback proceeds in four stages:

1. Build the type tag describing the function's signature.
2. Marshal each argument from NAC3's in-memory layout to the wire format.
3. Call `rpc_send` (or `rpc_send_async` for async RPCs).
4. For synchronous RPCs only: call `rpc_recv` in a loop and demarshal the return
   value back into NAC3 types.

## 2. Runtime ABI

The kernel communicates with the host via two C-ABI functions:

- **`rpc_send(service_id, tag, args)`** / **`rpc_send_async(...)`**: Initiates
  an RPC call. `service_id` is an `i32` uniquely identifying the callee (its
  `DefinitionId` as assigned by the kernel compiler); `tag` is a `{*i8, size_t}`
  descriptor encoding the signature; and `args` is an array of type-erased
  pointers to each argument's wire representation.
- **`rpc_recv(*i8) -> i32`**: Receives the return value into a caller-provided
  buffer. Returns `0` when the response is complete, or a positive integer `N`
  meaning "allocate `N` more bytes and call again." The caller loops, growing a
  stack-allocated buffer, until `rpc_recv` returns `0`.

## 3. Type Tag Format

Each RPC call is described by a type tag: a compact byte string that encodes the
full signature. The `gen_rpc_tag` function emits this string. The encoding
alphabet is:

| Byte | Meaning |
|------|---------|
| `i` / `I` | `int32` / `int64` |
| `f` / `b` | `float` / `bool` |
| `s` / `n` | `str` / `none` |
| `t<count><fields…>` | tuple: a one-byte field count followed by recursive field tags |
| `l<elem>` | list: recursive element tag |
| `a<ndims><dtype>` | ndarray: a one-byte ndim count followed by recursive dtype tag |
| `O` (leading) | method receiver / object parameter |
| `:` | separates argument tags from the return tag |

A full signature takes the form `[O] <arg tags…> : <return tag>`. For example, a
method taking `(int64, float)` and returning `int64` produces the tag `OIf:I`.

Tags are interned as deduplicated private LLVM globals and passed to `rpc_send`
as a `{*i8, size_t}` descriptor pointing at the tag bytes.

## 4. Wire Layout vs. NAC3 Layout

The on-the-wire format **strips object headers and inlines composites**. This
differs from how NAC3 represents the same types in memory; see
[refcounting.md](refcounting.md) for NAC3's internal layout.

| Type | NAC3 layout | Wire layout |
|------|-------------|-------------|
| scalars (`i`, `I`, `f`, `b`, `s`, `n`) | same | same |
| list | `{ObjectHeader, ptr, len}` | `{ptr, len}` |
| tuple | `{ObjectHeader, {fields…}}` | inline `{fields…}` |
| ndarray | refcounted object | inline descriptor `{*data, shape[ndims]}` |

Only scalars are bit-compatible - they can be `memcpy`'d directly between NAC3
memory and the wire. All composite types require per-element marshalling in both
directions.

## 5. Argument Marshalling (Send Path)

Before calling `rpc_send`, the marshalling code assembles a `void**` array with
one pointer per argument, each pointing at the argument's wire representation on
the stack. The per-argument marshalling dispatches as follows:

- **Scalars and strings**: Stored as-is and pointed at directly.
- **Lists**: If the element type is bit-compatible (scalars only), the elements
  are bulk-`memcpy`'d into a flat buffer. Otherwise, each element is
  individually marshalled (handling nested ndarrays and composites) and a
  pointer array is built.
- **Tuples**: Each field is marshalled inline, one field at a time.
- **NDArrays**: The array is made contiguous first, then a wire descriptor
  `{*data, shape[ndims]}` is written by a dedicated helper.

## 6. Return-Value Demarshalling (Sync Only)

For synchronous RPCs, after `rpc_send` returns the kernel calls `rpc_recv` in a
loop:

1. Call `rpc_recv(buffer)` with a stack-allocated buffer.
2. If it returns a positive `N`, grow the stack buffer by `N` bytes and call
   again.
3. Repeat until `rpc_recv` returns `0`.

This protocol allows the host to return variable-length and deeply nested data
without the kernel needing to know the size upfront.

Once the loop completes, the demarshalling code reconstructs NAC3 values from
the wire data. This is the inverse of marshalling: `ObjectHeader`s are re-
inserted, refcounted objects (lists, ndarrays, tuples) are allocated on the
heap, and nested types are processed recursively.

Because the wire buffers are stack-allocated, the CTRC mode (see
[ctrc.md](ctrc.md)) never affects them. Only the demarshalled NAC3 objects
follow the prevailing allocation mode, so an RPC inside a `with critical(...):`
block returns slab-allocated values built out of stack-allocated wire data.

A `None` return value short-circuits this process entirely: the kernel calls
`rpc_recv(null)` exactly once and skips all demarshalling.

## 7. Attributes Writeback

After a kernel exits, any mutable globals and mutable RPC-compatible class
fields that the kernel may have modified are shipped back to the host so that
host-side Python objects reflect the mutations. This is implemented as a
synthesized **async** RPC (fire-and-forget, so it does not block the kernel's
exit path).

Only attributes whose types can be described by the tag format are eligible for
writeback - `gen_rpc_tag` is used to filter candidates. The host-side Python
objects to write back into are tracked by `global_value_ids` in
`symbol_resolver.rs`.
