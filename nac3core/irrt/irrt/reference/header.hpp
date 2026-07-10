#pragma once

#include "irrt/stdlib/cstdint.h"

/**
 * @brief A dummy global variable acting as a marker for the beginning of NAC3-related globals.
 *
 * This field can be accessed via `ModuleContext::global_begin_ptr` in the compiler sources.
 */
extern "C" const unsigned char __nac3_global_begin;

namespace __nac3_impl::reference {
/**
 * @brief The object header structure present in all composite types in NAC3.
 *
 * Corresponds to `ObjectHeader{Type,Value}` in the compiler sources.
 */
struct ObjectHeader {
    /**
     * @brief The reference count for the object.
     *
     * The refcount is zero if the object is not heap-allocated or otherwise should not be freed.
     */
    uint32_t refcount;

    /**
     * @brief The offset to the `typeinfo` structure for this object, relative to `__nac3_global_begin`.
     *
     * The lowest 3 bits are reserved for flags:
     *
     * - Bit 0 (`TYPEINFO_OFFSET_CTRC_BIT`) marks objects allocated from the CTRC slab.
     * - Bits 1-2 are unused.
     */
    int32_t typeinfo_offset;
};

/**
 * @brief Flag bit for `ObjectHeader::typeinfo_offset` marking an object as allocated from the CTRC slab.
 *
 * Objects marked with this bit shall not be freed via `__builtin_free`.
 */
constexpr const int32_t TYPEINFO_OFFSET_CTRC_BIT = 0x1;

/**
 * @brief Mask recovering the actual typeinfo offset from `ObjectHeader::typeinfo_offset` by clearing the reserved flag
 * bits.
 */
constexpr const int32_t TYPEINFO_OFFSET_MASK = static_cast<int32_t>(~0x7);
}  // namespace __nac3_impl::reference
