#pragma once

#include "irrt/stdlib/cstddef.h"

namespace __nac3_impl::ctrc {
namespace {
/**
 * @brief The CTRC-mode nesting depth.
 *
 * Nonzero while execution is inside (any nesting of) a `with critical:` block: `__nac3_ctrc_enter` increments it and
 * `__nac3_ctrc_exit` decrements it, so restoring the prior mode on exit is a plain decrement.
 *
 * When nonzero, all allocations are routed to the CTRC slab, including those in callees of the block body.
 *
 * Kept in its own header so that `reference.hpp` (which sets the header marker bit at object initialization) can read
 * the mode without including the slab machinery.
 */
size_t ctrc_mode_depth = 0;

/**
 * @brief Returns whether allocation is currently in CTRC mode.
 */
[[gnu::always_inline]] bool in_ctrc_mode() {
    return ctrc_mode_depth > 0;
}
}  // namespace
}  // namespace __nac3_impl::ctrc
