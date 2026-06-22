# Reference Counting

NAC3 uses automatic, codegen-emitted reference counting for heap objects with
shared ownership. The subsystem is ARTIQ-agnostic and lives in `nac3core`.

The Rust implementation lives in `nac3core/src/codegen/types/reference.rs` (core
types and operations), `types/typeinfo.rs` (type metadata), and `types/mod.rs`
(the `RefType` trait and typeinfo registration). Emission sites are in
`codegen/stmt.rs` and `codegen/expr.rs`. The C++ IRRT runtime support lives in
`nac3core/irrt/irrt/reference/`.

## 1. Overview

Heap objects with shared ownership need deterministic cleanup. Every refcounted
heap object is prefixed with an `ObjectHeader`; increment/decrement calls are
emitted by codegen and implemented in IRRT. Decrementing a reference count to
zero frees the object and recursively decrements its refcounted children.

## 2. Object Layout

### Object Header (`ObjectHeader`)

Every refcounted object and tuples carries an `ObjectHeader` at byte offset 0.
It is 8 bytes wide and consists of two fields:

- **`refcount`** (`uint32_t`): A value of `0` means the object is not
  heap-allocated and should never be freed; a value of `1` or greater is the
  live reference count.
- **`typeinfo_offset`** (`int32_t`): An offset to this type's `Typeinfo`
  metadata, relative to the global symbol `__nac3_global_begin`. Using a
  relative offset allows the location of the `typeinfo` to fit within 32 bits.

The C++ definition is in `reference/header.hpp`; the mirroring Rust struct
fields are in `ObjectHeaderStructFields` in `reference.rs`.

### `RefCountedArray`

List and ndarray data buffers are stored in a `RefCountedArray`. Its memory
layout is:

```c
struct RefCountedArray {
    ObjectHeader header;                  // offset  0, 8 bytes
    size_t       refcounted_elems;        // offset  8, sizeof(size_t) bytes
    uint8_t      _pad[8 - sizeof(size_t)] // see below
    T            elems[];                 // offset 16
};
```

The `refcounted_elems` count field is padded so that `elems` always begins at a
**fixed 16-byte offset** from the start of the struct, regardless of whether
`size_t` is 32 or 64 bits wide. This ensures that the location of `elems` is
independent of the type of the elements, and makes element access a
constant-offset operation on every target.

The C++ definition is in `reference/array.hpp`; the mirroring Rust type is
`RefCountedArrayType` in `reference.rs`.

### `Typeinfo`

`Typeinfo` is per-type metadata used by the decrement operation to walk an
object's refcounted children. It contains a pointer to the type's name and a
length-prefixed array of byte offsets within the struct at which refcounted
fields live.

Two sentinel "count" values select special traversal modes instead of treating
the array as a simple list of field offsets:

- **`REFCOUNT_ARRAY_MAGIC`**: The field is an array of object pointers; walk
  each element with pointer-size stride.
- **`REFCOUNT_ARRAY_INLINE_MAGIC`**: The field is an array of inline
  header-bearing elements (e.g. tuples); the second entry in the offsets array
  gives the per-element stride.

The C++ definition is in `reference/typeinfo.hpp`; the mirroring Rust type is
in `typeinfo.rs`.

## 3. Runtime Functions

The IRRT provides the following functions for reference counting (declared in
`reference/reference.hpp`):

- **`__nac3_object_header_init(obj, is_refcounted, typeinfo)`**: Initializes the
  header of a newly allocated object by setting `refcount` to `1` or `0`
  (depending on if the object is refcounted), and computes `typeinfo_offset`
  relative to `__nac3_global_begin`.
- **`__nac3_refcount_incr`** / **`__nac3_refcount_decr`** (plus `…64`-suffix
  variants for 64-bit `size_t`): Increment and decrement the reference count.
- **`__nac3_is_object_refcounted`**: Returns whether an object's `refcount` is
  non-zero.

**Increment** bumps `refcount` only when the object is heap-allocated (i.e.
`refcount > 0`).

**Decrement** decrements `refcount` when non-zero and, on reaching zero, walks
all refcounted children via `Typeinfo` then frees the allocation. When `refcount`
is already zero (e.g. an inline tuple that is never heap-allocated directly), the
decrement still walks children but does not free. There are three child-traversal
modes: fixed struct fields, pointer array, and inline array - selected by the
`Typeinfo` count value (see [here](#-Typeinfo-)).

On the Rust side, `ObjectHeaderValue` provides `init`, `increment_refcount`, and
`decrement_refcount` methods, plus null-checked `safe_*` variants that guard with
a null check before calling - used for nullable objects such as `Option`.

## 4. When Increment/Decrement Are Emitted

Codegen emits refcount operations at the following points:

- **Function call arguments**: Refcounted arguments are incremented before a
  call. After `@extern` calls specifically, they are also decremented - see
  [§6](#6-extern-abi) for the rationale.
- **Variable assignment**: The new value is incremented and the old value is
  decremented.
- **List element assignment**: The new element is incremented and the old element
  is decremented.
- **NDArray view creation**: The `base` buffer is incremented when it is shared
  between two ndarrays.

**Known limitation:** Expression temporaries are not decremented at scope exit -
only named locals are cleaned up on function exit.

## 5. Which Types Are Refcounted

The `is_obj_id_refcounted` function in `reference.rs` defines the allow-list of
refcounted types:

| Category | Types |
|----------|-------|
| Refcounted | `list`, `ndarray`, `option`, user-defined classes |
| Not refcounted | `int32`, `int64`, `uint32`, `uint64`, `float`, `bool`, `str`, `range`, `exception`, `enumerate`, `tuple`, `none` |

Per-type notes:

- **`list`** - contains an `ObjectHeader`, a pointer to a `RefCountedArray` data
  buffer, and a length. Both the list object and its data buffer are separately
  refcounted.
- **`ndarray`** - contains `itemsize`, `ndims`, `shape`, `strides`, `data`,
  `base`, and `offset`. The `shape`, `strides`, `data`, and `base` fields each
  point to a `RefCountedArray` object, all of which are separately refcounted.
- **`option`** - backed by a single-element `RefCountedArray`. A null pointer
  represents `None`; a non-null pointer represents `Some`.
- **`tuple`** - carries an `ObjectHeader` but is not refcounted. Tuples are only
  walked for child refcounts when nested inside an array, via the inline-magic
  traversal mode.
- **`str`** - an inline `{ptr, len}` pair (`cslice`), not itself refcounted, but
  may appear as a field of a refcounted type.

By convention, all types that carry an `ObjectHeader` shall use
`RefCountedArray` for any heap-allocated data buffer.

## 6. `@extern` ABI {#extern}

### The Rule

**A refcounted object always crosses an `@extern` boundary as a base pointer -
the `ObjectHeader` is at byte offset 0.** NAC3 never passes an extern function a
pre-offset payload pointer for a refcounted object; it is the callee's
responsibility to offset past the header to reach the payload.

### Object Pointer vs. Payload Pointer

Every refcounted `ProxyValue` internally holds a single pointer that points at
the `ObjectHeader`. Two accessors expose what can be done with it:

- **`.header(ctx)`**: Reinterprets the pointer as pointing at the header - no
  GEP required.
- **`.inner_ptr(ctx)`**: Emits a GEP by `sizeof(ObjectHeader)` to reach the
  payload immediately following the header.

The IRRT mirrors this symmetrically: `get_object_header(p)` treats `p` as a
header pointer; `get_object_start(p)` adds `sizeof(ObjectHeader)` to reach the
payload. Both sides receive the same base pointer at the header.

### Two Stacked Object Layers

A `list[T]` (and similarly an ndarray) involves **two** separate refcounted
objects, each with their own `ObjectHeader` at offset 0:

1. The **list object** itself: `{ObjectHeader, data_ptr, len}`. Extern receives
   a pointer to this header.
2. Its **data buffer**: a separate `RefCountedArray`, also with a header at
   offset 0. The actual elements live at a fixed 16-byte offset into this buffer
   (see §2).

Notably, ndarrays also have `shape` and `strides` as separate `RefCountedArray`
objects.

### Increment/Decrement Around Extern Calls

Before any call, refcounted arguments are incremented via `.header()` on the
base pointer. After an `@extern` call only, they are then decremented. This
brackets the foreign call so the callee may hold a reference to the object for
the duration of the call without NAC3 freeing it underneath, and releases the
NAC3-side reference afterward.

The extern call is assumed to manually increment any refcounted objects it wants
to hold onto beyond the call, and to decrement them when done. Notably,
`__nac3_refcount_incr` and `__nac3_refcount_decr` can be declared via `@extern`
and exposed to the Python side for this purpose, with the following caveats:

- The functions are compiler intrinsics and **has zero ABI or API guarantees**.
- The functions **do not** check that the pointer is a valid refcounted object;
  notably, the function **cannot** differentiate between primitive values from
  values prefixed with an object header.
- The functions **do** correctly handle null pointers.
- The caller of `__nac3_refcount_decr` **must** ensure that the refcount of the
  object prior to the call is greater than one, or else the object will be
  double-freed.

### Non-Refcounted Types at the Boundary

- **`str`** crosses as a `cslice { void* data; size_t len; }` by value; `data`
  points directly at bytes with no header to skip.
- **`range`** and **`exception`** cross as plain structs, not object pointers.
