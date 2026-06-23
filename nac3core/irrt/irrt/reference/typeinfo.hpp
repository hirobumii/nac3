#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/string.hpp"

namespace __nac3_impl::reference {
/**
 * @brief Typeinfo for reference counted types.
 *
 * Corresponds to `Typeinfo{Type,Value}` in the compiler sources.
 */
struct Typeinfo {
    /**
     * @brief The name of the type.
     */
    String* name;

    /**
     * @brief The byte offsets of the reference counted fields in the type.
     *
     * The first element of the array is the number of offsets, followed by the offsets themselves.
     */
    uint32_t* refcounted_field_offsets;
};
}  // namespace __nac3_impl::reference
