#pragma once

#include <irrt/int_types.hpp>
#include <irrt/slice.hpp>

namespace
{
/**
 * @brief A list in NAC3.
 *
 * The `items` field is opaque. You must rely on external contexts to
 * know how to interpret it.
 */
template <typename SizeT> struct List
{
    uint8_t *items;
    SizeT len;
};
} // namespace