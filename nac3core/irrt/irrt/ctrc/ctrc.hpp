#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"

#include "irrt/ctrc/mode.hpp"
#include "irrt/ctrc/page.hpp"
#include "irrt/debug.hpp"
#include "irrt/exception.hpp"
#include "irrt/reference/reference.hpp"

// Constant-time reference counting (CTRC) slab allocator: the slab logic (allocation, deferred
// drop, pool growth) and the extern "C" entry points. See docs/ctrc.md for the design overview.
namespace __nac3_impl::ctrc {
namespace {
/**
 * @brief The stack of pages with at least one available cell, or `nullptr` if the reserved pool
 * and all recycled cells are exhausted.
 */
Page* last_page = nullptr;

/**
 * @brief The number of cells currently available for allocation across all pages.
 */
size_t num_free_cells = 0;

/**
 * @brief Pushes a dropped cell onto its owning page's free or drop list.
 *
 * The cell must hold an object whose refcount has reached zero. Cells of objects without
 * refcounted children go straight to the free list; cells with (potential) children go to the
 * drop list, deferring the child-drop to `pop_free`.
 */
void defer_drop(Cell* const cell) {
    Page* const page = page_of(cell);
    ++num_free_cells;

    // a page with no available cells is not in the available-pages stack; it becomes available now
    const bool was_exhausted = page->free_ptr == nullptr && page->drop_ptr == nullptr && page->free_counter == 0;
    if (was_exhausted) {
        page->next_page = last_page;
        last_page = page;
    }

    // the array magic values are nonzero, so array objects always take the drop path
    const auto* const typeinfo = reference::get_object_typeinfo(cell);
    const bool has_children = typeinfo->refcounted_field_offsets[0] != 0;

    if (has_children) {
        cell->header.refcount = page->drop_ptr == nullptr ? CTRC_NO_CELL : cell_index(page, page->drop_ptr);
        page->drop_ptr = cell;
    } else {
        cell->header.refcount = page->free_ptr == nullptr ? CTRC_NO_CELL : cell_index(page, page->free_ptr);
        page->free_ptr = cell;
    }
}

/**
 * @brief Drop the fields of an object within the given cell.
 *
 * Grandchildren that are also CTRC are deferred for dropping, while heap-allocated grandchildren are freed immediately.
 */
void drop_fields(Cell* const cell) {
    reference::walk_children(cell);
}

/**
 * @brief Pops an available cell from the slab, or returns `nullptr` if the reserved pool and all
 * recycled cells are exhausted.
 *
 * Cells are drawn in order of cost: recycled cells with no pending drops, then never-allocated
 * cells, then recycled cells from the drop list - cells in the drop list are dropped before being returned.
 */
Cell* pop_free() {
    if (last_page == nullptr) {
        return nullptr;
    }

    Page* const page = last_page;
    Cell* result = page->free_ptr;
    bool need_drop = false;

    if (result != nullptr) {
        page->free_ptr = cell_at(page, result->header.refcount);
    } else if (page->free_counter > 0) {
        result = &page->cells[--page->free_counter];
    } else {
        // non-null: an exhausted page would not be in the available-pages stack
        result = page->drop_ptr;
        page->drop_ptr = cell_at(page, result->header.refcount);
        need_drop = true;
    }

    // page exhausted - unlink it from the available-pages stack
    if (page->free_ptr == nullptr && page->drop_ptr == nullptr && page->free_counter == 0) {
        last_page = page->next_page;
    }

    --num_free_cells;

    // perform bookkeeping after cell is popped, since a child in the same page may be dropped,
    // corrupting the free list
    if (need_drop) {
        drop_fields(result);
    }

    return result;
}

/**
 * @brief Grows the pool until it holds at least `num_free_pages` pages worth of *available* cells.
 * No-op if that many cells are already available.
 *
 * Returns whether that many cells are available after the call. This is the ONLY function that may
 * allocate backing memory for the slab.
 */
bool reserve(const size_t num_free_pages) {
    const size_t num_needed_cells = num_free_pages * CTRC_CELLS_PER_PAGE;
    if (num_needed_cells <= num_free_cells) {
        return true;
    }

    // round the shortfall up to whole pages - pages are the unit of backing memory
    const size_t grow = (num_needed_cells - num_free_cells + CTRC_CELLS_PER_PAGE - 1) / CTRC_CELLS_PER_PAGE;
    Page* const chunk = page_backend_alloc(grow);
    if (chunk == nullptr) {
        return false;
    }

    for (size_t i = 0; i < grow; ++i) {
        Page* const page = &chunk[i];
        page->free_ptr = nullptr;
        page->drop_ptr = nullptr;
        page->free_counter = CTRC_CELLS_PER_PAGE;
        page->next_page = last_page;
        last_page = page;
    }

    num_free_cells += grow * CTRC_CELLS_PER_PAGE;
    return true;
}

/**
 * @brief Allocates one cell from the slab, or returns `nullptr` if `size` exceeds the cell
 * capacity or the slab is exhausted.
 *
 * `size` is the total object size including the `ObjectHeader`. The returned memory is
 * uninitialized: the caller must initialize the object header before the object is used or
 * dropped.
 */
void* alloc(const size_t size) {
    if (size > CTRC_CELL_SIZE) {
        return nullptr;
    }

    return pop_free();
}
}  // namespace
}  // namespace __nac3_impl::ctrc

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::ctrc;

/**
 * @brief Allocates `size` bytes (including the `ObjectHeader`) from the CTRC slab.
 *
 * Returns `nullptr` if `size` exceeds the cell capacity or the slab is exhausted.
 */
void* __nac3_ctrc_alloc(size_t size) {
    return alloc(size);
}

/**
 * @brief Grows the CTRC pool until it holds at least `num_free_pages` pages worth of available
 * cells, returning whether that many cells are available afterwards.
 */
bool __nac3_ctrc_reserve(size_t num_free_pages) {
    return reserve(num_free_pages);
}

/**
 * @brief Defers the drop of a CTRC-allocated object whose refcount has reached zero.
 */
void __nac3_ctrc_defer_drop(void* object) {
    defer_drop(static_cast<Cell*>(object));
}

/**
 * @brief Enters CTRC mode by growing the CTRC pool to hold at least `num_free_pages` pages worth of available cells,
 * and setting the current allocation mode to the CTRC slab allocator.
 *
 * Note that the pool only grows if the requested capacity exceeds the cells already available. If the allocation cannot
 * be satisfied, this function will fail silently - The original pool size is retained, and any allocations that exceeds
 * the pool size will raise a `MemoryError` at the point of allocation.
 */
void __nac3_ctrc_enter(size_t num_free_pages) {
    // Note: We deliberately ignore the return value of `reserve` here, since the failure to reserve pages may still
    // leave sufficient cells for allocation during the critical region.
    // Moreover, this function must not raise, since this function must be paired with `__nac3_ctrc_exit` at all times
    // to update the CTRC mode depth, and an unwind during this function would leave the depth unbalanced.
    reserve(num_free_pages);
    ++ctrc_mode_depth;
}

/**
 * @brief Exits CTRC mode, restoring the allocation mode of the enclosing extent.
 */
void __nac3_ctrc_exit() {
    debug_assert(ctrc_mode_depth > 0);
    --ctrc_mode_depth;
}

/**
 * @brief Allocates `size` bytes with `align` bytes of alignment for refcounted heap objects, selecting the allocator
 * based on the current allocation mode.
 *
 * In CTRC mode, `nullptr` is returned if the request cannot be satisfied due to either oversized object, unsatisfiable
 * alignment, or memory exhaustion of the CTRC slab.
 *
 * The `align` argument is ignored for objects allocated by `malloc`.
 *
 * `[[gnu::malloc]]` marks the returned pointer `noalias`, indicating that this function returns memory that does not
 * alias any other accessible object. Note that this does not imply `nounwind` or the lack of side effects - This
 * function may raise and will mutate global slab state.
 */
[[gnu::malloc]] void* __nac3_alloc(size_t size, size_t align) {
    void* ptr;
    if (in_ctrc_mode()) {
        ptr = (align > CTRC_CELL_SIZE) ? nullptr : alloc(size);
    } else {
        ptr = __builtin_malloc(size);
    }

    if (ptr == nullptr) {
        raise_exception(EXN_MEMORY_ERROR, "Failed to allocate {0} bytes", static_cast<int64_t>(size), NO_PARAM,
                        NO_PARAM);
    }

    return ptr;
}
}  // extern "C"
