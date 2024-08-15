#pragma once

#include <irrt/int_types.hpp>

namespace cstr
{
/**
 * @brief Implementation of `strlen()`.
 */
uint32_t length(const char *str)
{
    uint32_t length = 0;
    while (*str != '\0')
    {
        length++;
        str++;
    }
    return length;
}
} // namespace cstr