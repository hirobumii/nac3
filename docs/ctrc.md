# Constant-Time Reference Counting (CTRC)

CTRC is a second heap-reclamation strategy for NAC3, provided alongside the
default [codegen-emitted reference counting](refcounting.md). Inside a
`with ctrc:` block, allocation is served from a persistent, global slab of
fixed-size cells that gives **O(1) allocation** and **O(1) drop**, at the cost
of a fixed per-object size cap and a high-water-mark memory footprint. It is
intended for real-time kernels where the unbounded, recursive work of the eager
refcounting path is unacceptable.

The runtime lives in `nac3core/irrt/irrt/ctrc/`:

- `page.hpp` - the data layer: page and cell layout, size constants, and the
  platform page backend.
- `ctrc.hpp` - the slab logic: allocation, deferred drop, and pool growth, plus
  the `extern "C"` entry points.

The technique is ported from the Koka runtime's constant-time reference
counting (`ctrc-koka.c` at the repository root is the reference implementation).

## 1. Overview

A live object allocated by CTRC is an ordinary NAC3 heap object — an
`ObjectHeader` followed by user data — that happens to reside in a slab cell
rather than in a `malloc`'d block. It participates in reference counting like
any other object; what differs is only how its memory is obtained and reclaimed:

- **Allocation** draws one cell from the slab. This never touches the system
  allocator and never grows the slab (see the latency invariant below).
- **Drop** — reaching refcount zero — is a single push onto a per-page list.
  The recursive child-drop that the eager path performs immediately is instead
  deferred: it is paid off one level at a time by *subsequent* allocations,
  which walk the children of one dropped cell as they recycle it.

CTRC objects are distinguished from ordinary heap objects by a marker bit in the
object header's `typeinfo_offset` (see [Reference Counting](refcounting.md) for
the header layout). The decrement routine consults this bit to decide whether a
zero-refcount object is freed eagerly (`__builtin_free`) or deferred onto the
slab (`__nac3_ctrc_defer_drop`); the marker is set at header initialization when
allocation happens under CTRC mode. That routing and the mode machinery live in
`reference/`, not in `ctrc/`.

## 2. Latency Invariant

The defining property of the slab is that **allocation never grows it**. Inside
a `with ctrc:` block, cells are drawn only from already-reserved pages and
recycled cells. When both are exhausted, `__nac3_ctrc_alloc` returns `nullptr`
and the caller raises `MemoryError` — it does *not* fall back to acquiring more
backing memory, because that would introduce unbounded latency into the timing-
critical region.

All backing-store growth happens exclusively in `__nac3_ctrc_reserve`, which is
called from the block prologue (for `with ctrc(size):`), never from within a
block body. Consequently:

- The slab has a **high-water-mark footprint**: pages are reserved but never
  returned to the system.
- An object that **escapes** a `with ctrc:` block remains valid. It is a normal
  refcounted object and is reclaimed by its own refcount through the deferred
  path whenever it finally reaches zero, even long after the block has exited.

## 3. Memory Layout

### Pages and Cells

The slab is organized into **pages** of `CTRC_PAGE_SIZE` (4096) bytes, each
subdivided into fixed-size **cells** of `CTRC_CELL_SIZE` (128) bytes. Both are
powers of two. The cell size is the single tunable size cap of the slab
(`SMALL_BLOCK` in the Koka reference): any allocation request whose total size,
including the `ObjectHeader`, exceeds one cell is rejected.

Pages are allocated at `CTRC_PAGE_SIZE`-aligned addresses. This is the key
trick that keeps drop O(1): given any cell address, its owning `Page` is
recovered by masking off the low bits of the address (`page_of`), with no side
table or per-object back-pointer.

The first cell-sized slot of each page holds the page header; the remaining
`CTRC_CELLS_PER_PAGE` (= `CTRC_PAGE_SIZE / CTRC_CELL_SIZE - 1` = 31) slots are
allocatable cells.

### Free / Drop Lists and the Recycled-Cell Link

Each page tracks three independent sources of available cells:

| Source         | Page field     | Meaning                                                        |
| -------------- | -------------- | ------------------------------------------------------------- |
| Free list      | `free_ptr`     | Dropped cells ready for reuse, with **no** pending child drops. |
| Drop list      | `drop_ptr`     | Dropped cells whose children still need **one** level of dropping before reuse. |
| Virgin cells   | `free_counter` | Never-allocated cells remaining in the page.                  |

The free and drop lists are singly-linked, but they store no pointers of their
own. A dropped cell holds no live object, so its header's now-dead `refcount`
slot is reused as a **32-bit cell-index link** — the index of the next cell in
the list within the same page — with the sentinel `CTRC_NO_CELL`
(`0xffffffff`) marking the end. `typeinfo_offset` is deliberately left intact
in a dropped cell so that the deferred drop can still re-derive the object's
refcounted children from its `Typeinfo`.

### The Available-Pages Stack

A page is *available* if at least one cell can be popped from it, via any of the
three sources above. Available pages are chained into a singly-linked stack
through their `next_page` field, headed by the global `last_page`. A page is
unlinked from this stack the moment it becomes fully exhausted, and re-linked by
`defer_drop` the moment a cell is returned to a previously-exhausted page.

## 4. The Allocation / Drop Lifecycle

### Dropping a cell (`defer_drop`, O(1))

When a CTRC object's refcount reaches zero, its cell is pushed onto one of its
owning page's lists — no recursion, no child traversal:

- If the object's `Typeinfo` shows it has **no** refcounted children, the cell
  goes straight onto the **free list**: it is immediately reusable.
- Otherwise the cell goes onto the **drop list**, deferring the one level of
  child-drop to whoever later recycles it.

If the owning page was fully exhausted, pushing a cell makes it available again,
so the page is re-linked into the available-pages stack.

### Popping a cell (`pop_free`)

Allocation pulls one cell from `last_page`, drawing from the three sources in
order of increasing cost:

1. **Free list** — a recycled cell with no pending work; cheapest.
2. **Virgin cells** — a never-allocated cell (`free_counter`).
3. **Drop list** — a recycled cell that still owes one level of child-drop.
   Only here is `drop_fields` run: it decrements the refcount of each refcounted
   child of the previously-dropped object. This is how deferred work is amortized
   — each allocation from the drop list pays exactly one level.

Grandchildren are thus handled transitively but still incrementally: a child
that itself reaches zero is re-deferred (if it is a CTRC object) or freed
eagerly (if it is an ordinary heap object), never recursed into synchronously.
Draining a page's book-keeping happens *before* `drop_fields` runs, because
dropping children may re-defer cells onto this or any other page and re-link
them into the stack.

Unlike the Koka reference, `pop_free` **never grows the slab**: if `last_page`
is `nullptr`, it returns `nullptr` (honoring the latency invariant).

### Growing the pool (`reserve`)

`reserve(num_pages)` raises the persistent pool to `num_pages` total pages,
obtaining a single contiguous, page-aligned chunk from the platform backend and
threading each new page onto the available stack with all its cells virgin. It
is a no-op if the pool already holds at least that many pages, and it is the
**only** function that may allocate backing memory for the slab.

## 5. Page Backend

Backing memory is obtained through `page_backend_alloc`, which carves a
`CTRC_PAGE_SIZE`-aligned block from the runtime heap: it over-allocates via
`__builtin_malloc` by `CTRC_PAGE_SIZE - 1` bytes and aligns the result up.
Using the heap keeps the backend portable across every target — hosts and
`mmap`-less on-device platforms (RISC-V VexRiscv, Zynq Cortex-A9) alike — with
no platform-specific code paths.

The original `__builtin_malloc` pointer is intentionally not retained: CTRC
pages are never returned to the system (the slab has a high-water-mark
footprint), so there is nothing to free. The only cost is up to
`CTRC_PAGE_SIZE - 1` bytes of alignment slack per `reserve` call, which is
negligible because growth happens in bulk and rarely.

## 6. Entry Points

The slab is driven through three `extern "C"` functions in `ctrc.hpp`, called
from generated code:

| Function                             | Role                                                                 |
| ------------------------------------ | -------------------------------------------------------------------- |
| `__nac3_ctrc_alloc(size)`            | Allocate one cell of at least `size` bytes (header included). Returns `nullptr` if `size` exceeds cell capacity or the slab is exhausted; never grows the slab. |
| `__nac3_ctrc_reserve(num_pages)`     | Grow the persistent pool to `num_pages` pages; returns whether the pool holds at least that many afterwards. The sole growth site. |
| `__nac3_ctrc_defer_drop(object)`     | Defer the drop of a CTRC object whose refcount has reached zero (O(1)). |
