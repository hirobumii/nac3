# Constant-Time Reference Counting (CTRC)

CTRC is a second heap-reclamation strategy for NAC3, provided alongside the
default [codegen-emitted reference counting](refcounting.md). Inside a
`with critical(...):` block, allocation is served from a persistent, global slab
of fixed-size cells that gives **O(1) allocation** and **O(1) drop**, at the
cost of a fixed per-object size cap and a high-water-mark memory footprint. It
is intended for real-time sections of the kernel where the unbounded, recursive
work of the eager refcounting path introduces unacceptable latency.

The runtime lives in `nac3core/irrt/irrt/ctrc/`:

- `page.hpp` - the data layer: page and cell layout, size constants, and the
  platform page backend.
- `ctrc.hpp` - the slab logic: allocation, deferred drop, and pool growth, plus
  the `extern "C"` entry points.

The technique follows the constant-time reference counting strategy described in
[Being Lazy When It Counts: Practical Constant-Time Memory Management for
Functional Programming](https://link.springer.com/10.1007/978-981-97-2300-3_11).

## 1. Overview

A live object allocated by CTRC is an ordinary NAC3 heap object - an
`ObjectHeader` followed by user data - that happens to reside in a slab cell
rather than in a `malloc`'d block. It participates in reference counting like
any other object; what differs is only how its memory is obtained and reclaimed:

- **Allocation** draws one cell from the slab. This never touches the system
  allocator and never grows the slab (see the latency invariant below).
- **Drop** - reaching refcount zero - is a single push onto a per-page list.
  The recursive child-drop that the eager path performs immediately is instead
  deferred: it is paid off one level at a time by _subsequent_ allocations,
  which walk the children of one dropped cell as they recycle it.

CTRC objects are distinguished from ordinary heap objects by a marker bit in the
object header's `typeinfo_offset`. The decrement routine consults this bit to
decide whether a zero-refcount object is freed by the system allocator eagerly
(`__builtin_free`) or freed by the CTRC slab allocator and deferred
(`__nac3_ctrc_defer_drop`); the marker is set at header initialization when
allocation happens under CTRC mode.

### Allocable objects

CTRC mode only changes the allocation behavior of **heap-allocated** objects;
stack allocations (including scalars and other values that NAC3 places on the
stack) are untouched, in or out of a `with critical` block.

Because each cell has a fixed size, objects allocated under CTRC mode are
subject to the following constraints:

- **Size**: the total size, including the `ObjectHeader`, must not exceed
  `CTRC_CELL_SIZE` (128 bytes).
- **Alignment**: the requested alignment must not exceed `CTRC_CELL_SIZE`
  (128 bytes).

An allocation that violates either bound returns `nullptr` from `__nac3_alloc`
and raises `MemoryError` at the allocation site - the slab is never grown to
accommodate it (see the Latency Invariant below).

In practice, this means that the following types are allocable under CTRC mode
(assuming sufficient cell availability):

- `list`: Up to 112 bytes of element data (i.e. 14 elements of
  `int64`/`uint64`/`ptr`, or 28 elements of `int32`/`uint32`).
    - **Note:** Consumes 2 cells per object.
- `ndarray`: Both of the following constraints must be satisfied:
    - Up to 14 dimensions (for 64-bit targets) or 28 dimensions (for 32-bit
      targets).
    - Up to 112 bytes of element data (i.e. 14 elements of
      `int64`/`uint64`/`ptr`, or 28 elements of `int32`/`uint32`).
    - **Note:** Consumes 4 cells per object.
- User-defined types whose size is at most than 120 bytes (i.e. up to 15 fields
  of `int64`/`uint64`/`ptr`, or 30 fields of `int32`/`uint32`).

One important note is that the optimizer may elect to allocate the object on the
stack and will thus directly bypass the CTRC allocation size restriction.
However, this should be treated as an optimization artifact: the user should not
rely on this optimization to avoid the size limit as the optimizer may change in
future LLVM versions.

## 2. Latency Invariant

The defining property of the slab is that **allocation never grows it**. Inside
a `with critical(...):` block, cells are drawn only from already-reserved pages
and recycled cells. When both are exhausted, the slab allocation returns
`nullptr` and `__nac3_alloc` raises `MemoryError` - it does _not_ fall back to
acquiring more backing memory, because that would introduce unbounded latency
into the timing-critical region.

All backing-store growth happens exclusively in the internal `reserve` routine,
which is invoked via `__nac3_ctrc_enter` from the block prologue (for
`with critical(num_free_pages):`), never from within a block body. Consequently:

- The slab has a **high-water mark footprint**: pages are reserved but never
  returned to the system.
- An object that **escapes** a `with critical(...):` block remains valid. It is
  a normal refcounted object and is reclaimed by its own refcount through the
  deferred path whenever it finally reaches zero, even long after the block has
  exited.

## 3. Memory Layout

### Pages and Cells

The slab is organized into **pages** of `CTRC_PAGE_SIZE` (4096) bytes, each
subdivided into fixed-size **cells** of `CTRC_CELL_SIZE` (128) bytes. Any
allocation request whose total size (including the `ObjectHeader`) exceeds one
cell is rejected.

Pages are allocated at `CTRC_PAGE_SIZE`-aligned addresses. This is the key
trick that keeps drop O(1): given any cell address, its owning `Page` is
recovered by masking off the low bits of the address (`page_of`), with no side
table or per-object back-pointer.

The first cell-sized slot of each page holds the page header; the remaining
`CTRC_CELLS_PER_PAGE` (= `CTRC_PAGE_SIZE / CTRC_CELL_SIZE - 1` = 31) slots are
allocatable cells.

### Free / Drop Lists and the Recycled-Cell Link

Each page tracks three independent sources of available cells:

| Source       | Page field     | Meaning                                                                         |
| ------------ | -------------- | ------------------------------------------------------------------------------- |
| Free list    | `free_ptr`     | Dropped cells ready for reuse, with **no** pending child drops.                 |
| Drop list    | `drop_ptr`     | Dropped cells whose children still need **one** level of dropping before reuse. |
| Virgin cells | `free_counter` | Never-allocated cells remaining in the page.                                    |

The free and drop lists are singly-linked, but they store no pointers of their
own. A dropped cell holds no live object, so its header's now-dead `refcount`
slot is reused as a **32-bit cell-index link** - the index of the next cell in
the list within the same page - with the sentinel `CTRC_NO_CELL`
(`0xffffffff`) marking the end. `typeinfo_offset` is deliberately left intact
in a dropped cell so that the deferred drop can still re-derive the object's
refcounted children from its `Typeinfo`.

### The Available-Pages Stack

A page is _available_ if at least one cell can be popped from it, via any of the
three sources above. Available pages are chained into a singly-linked stack
through their `next_page` field, headed by the global `last_page`. A page is
unlinked from this stack the moment it becomes fully exhausted, and re-linked by
`defer_drop` the moment a cell is returned to a previously-exhausted page.

## 4. The Allocation / Drop Lifecycle

### Dropping a cell (`defer_drop`, O(1))

When a CTRC object's refcount reaches zero, its cell is pushed onto one of its
owning page's lists:

- If the object's `Typeinfo` shows it has **no** refcounted children, the cell
  goes straight onto the **free list**: it is immediately reusable.
- Otherwise the cell goes onto the **drop list**, deferring the one level of
  child-drop to whoever later recycles it.

If the owning page was fully exhausted, pushing a cell makes it available again,
so the page is re-linked into the available-pages stack.

### Popping a cell (`pop_free`)

Allocation pulls one cell from `last_page`, drawing from the three sources in
order of increasing cost:

1. **Free list** - a recycled cell with no pending work; cheapest.
2. **Virgin cells** - a never-allocated cell (`free_counter`).
3. **Drop list** - a recycled cell that still owes one level of child-drop.
   Only here is `drop_fields` run: it decrements the refcount of each refcounted
   child of the previously-dropped object. This is how deferred work is
   amortized - each allocation from the drop list pays exactly one level.

Grandchildren are thus handled transitively but still incrementally: a child
that itself reaches zero is re-deferred (if it is a CTRC object) or freed
eagerly (if it is an ordinary heap object), never recursed into synchronously.
Draining a page's book-keeping happens _before_ `drop_fields` runs, because
dropping children may re-defer cells onto this or any other page and re-link
them into the stack.

To guarantee bounded latency, `pop_free` **never grows the slab**: if
`last_page` is `nullptr`, it returns `nullptr`.

### Growing the pool (`reserve`)

`reserve(num_free_pages)` grows the pool until at least `num_free_pages` pages
worth of _available_ cells exist, obtaining a single contiguous, page-aligned
chunk from the platform backend and threading each new page onto the available
stack with all its cells virgin. It is a no-op if that many cells are already
available, and it is the **only** function that may allocate backing memory for
the slab.

The guarantee is deliberately on **free capacity, not cumulative allocation**.
It ensures that a `with critical(n)` block (assuming `n` is nonzero)
_additionally_ allocates pages if necessary, so that the block's body can
fulfill any allocation requests without the user knowing the resident footprint
of the slab.

Availability is tracked in `num_free_cells` - the sum, over every page, of its
free list, its drop list, and its virgin cells - maintained by one increment or
decrement in each of `reserve`, `pop_free` and `defer_drop`, so both hot paths
stay O(1). The shortfall is rounded up to whole pages, since pages are the unit
of backing memory.

Cells are counted rather than free _pages_ because free pages are not
well-defined under fragmentation: 31 pages each holding a single free cell are
not one free page, and this design has no compaction. One intended consequence
of this design is that `num_free_pages == 0` is a meaningful opt-out - it
allocates nothing and runs the block on whatever capacity is already free.

## 5. Page Backend

Backing memory is obtained through `page_backend_alloc`, which carves a
`CTRC_PAGE_SIZE`-aligned block from the runtime heap: it over-allocates via
`__builtin_malloc` by `CTRC_PAGE_SIZE - 1` bytes and aligns the result up.
Using the heap keeps the backend portable across every target with no
platform-specific code paths.

The original `__builtin_malloc` pointer is intentionally not retained: CTRC
pages are never returned to the system (the slab has a high-water-mark
footprint), so there is nothing to free. The only cost is up to
`CTRC_PAGE_SIZE - 1` bytes of alignment slack per `reserve` call.

## 6. CTRC Mode and Nesting

The slab is only consulted while execution is in **CTRC mode**. Mode is tracked
by a single global nesting counter, `ctrc_mode_depth`: `__nac3_ctrc_enter`
increments it and `__nac3_ctrc_exit` decrements it, so `with critical(...):`
blocks nest and restoring the prior mode on exit is a plain decrement.
Allocation is routed to the slab whenever the depth is nonzero
(`in_ctrc_mode()`), including inside callees invoked from the block body.

`__nac3_ctrc_enter(num_free_pages)` both reserves capacity and raises the depth.
It deliberately ignores the result of `reserve`: a failed reservation may still
leave enough already-free cells for the block, and any genuine shortfall
surfaces as a `MemoryError` at the point of allocation. Critically, it **must
not raise** - enter and exit are paired to keep the depth balanced, and an
unwind out of enter would leave the depth permanently skewed.

The matching decrement is equally guaranteed on the other side: the block is
lowered as a `try`/`finally`, so `__nac3_ctrc_exit` runs on **every** exit path.
This is what keeps the pairing balanced when, for example, an exception raised
inside the block propagates out: mode depth is still restored on the way through
the unwind.

The block prologue passes the block's page count to `__nac3_ctrc_enter`. When
`with critical():` is written with no argument, the count defaults to
`CTRC_DEFAULT_RESERVED_PAGES` (16 pages ≈ 496 objects / 64 KiB); `with critical(0):`
reserves nothing and runs on whatever capacity is already free.
