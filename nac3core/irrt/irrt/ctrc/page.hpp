#pragma once

#ifdef IRRT_CTRC

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"

#include "irrt/reference/header.hpp"

namespace __nac3_impl::ctrc {
namespace {
/**
 * @brief The size of a CTRC page in bytes. Must be a power of two.
 *
 * Pages are allocated at `CTRC_PAGE_SIZE`-aligned addresses so that the owning page of any cell can be recovered by
 * masking the low bits of the cell address.
 */
constexpr const size_t CTRC_PAGE_SIZE = 4096;

/**
 * @brief The size of a CTRC cell in bytes, including the `ObjectHeader`. Must be a power of two.
 *
 * Any allocation request larger than one cell is rejected.
 */
constexpr const size_t CTRC_CELL_SIZE = 128;

/**
 * @brief The number of cells in a page.
 *
 * The first cell-sized slot of a page holds the page header.
 */
constexpr const size_t CTRC_CELLS_PER_PAGE = CTRC_PAGE_SIZE / CTRC_CELL_SIZE - 1;

/**
 * @brief Sentinel cell-index link marking the end of a per-page free/drop list.
 */
constexpr const uint32_t CTRC_NO_CELL = 0xffff'ffff;

/**
 * @brief A fixed-size allocation cell of a CTRC page.
 *
 * A live cell holds an ordinary NAC3 object: an `ObjectHeader` followed by user data. A dropped cell reuses the
 * `refcount` slot of its header as a 32-bit cell-index link chaining it into its page's free or drop list.
 */
struct Cell {
    reference::ObjectHeader header;
    unsigned char data[CTRC_CELL_SIZE - sizeof(reference::ObjectHeader)];
};
static_assert(sizeof(Cell) == CTRC_CELL_SIZE);

/**
 * @brief A CTRC page: the page header followed by `CTRC_CELLS_PER_PAGE` cells.
 *
 * A page is "available" if at least one cell can be popped from it.
 */
struct Page {
    /**
     * @brief Head of the free list: dropped cells ready for reuse with no pending child drops.
     */
    Cell* free_ptr;

    /**
     * @brief Head of the drop list: dropped cells whose children still need one level of dropping before reuse.
     */
    Cell* drop_ptr;

    /**
     * @brief The next page in the available-pages stack. Only meaningful while this page is linked into the stack.
     */
    Page* next_page;

    /**
     * @brief The number of never-allocated cells remaining in this page.
     */
    size_t free_counter;

    unsigned char padding[CTRC_CELL_SIZE - 3 * sizeof(void*) - sizeof(size_t)];

    Cell cells[CTRC_CELLS_PER_PAGE];
};
static_assert(sizeof(Page) == CTRC_PAGE_SIZE);

/**
 * @brief Returns the page owning the given cell.
 */
[[gnu::always_inline]] Page* page_of(const Cell* const cell) {
    return reinterpret_cast<Page*>(reinterpret_cast<size_t>(cell) & ~(CTRC_PAGE_SIZE - 1));
}

/**
 * @brief Returns the index of the given cell within its owning page.
 */
[[gnu::always_inline]] uint32_t cell_index(const Page* const page, const Cell* const cell) {
    return static_cast<uint32_t>(cell - page->cells);
}

/**
 * @brief Returns the cell with the given cell-index link, or `nullptr` for `CTRC_NO_CELL`.
 */
[[gnu::always_inline]] Cell* cell_at(Page* const page, const uint32_t index) {
    return index == CTRC_NO_CELL ? nullptr : &page->cells[static_cast<size_t>(index)];
}

/**
 * @brief Allocates a contiguous, `CTRC_PAGE_SIZE`-aligned chunk of `num_pages` uninitialized pages from the runtime
 * heap, or returns `nullptr` on failure.
 *
 * The block is over-allocated by `CTRC_PAGE_SIZE - 1` and aligned up to ensure `CTRC_PAGE_SIZE` alignment.
 */
Page* page_backend_alloc(const size_t num_pages) {
    void* const mem = __builtin_malloc(num_pages * CTRC_PAGE_SIZE + CTRC_PAGE_SIZE - 1);
    if (mem == nullptr) {
        return nullptr;
    }

    const size_t addr = (reinterpret_cast<size_t>(mem) + CTRC_PAGE_SIZE - 1) & ~(CTRC_PAGE_SIZE - 1);
    return reinterpret_cast<Page*>(addr);
}
}  // namespace
}  // namespace __nac3_impl::ctrc

#endif  // IRRT_CTRC
