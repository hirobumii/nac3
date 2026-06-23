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
     */
    int32_t typeinfo_offset;
};
}  // namespace __nac3_impl::reference
