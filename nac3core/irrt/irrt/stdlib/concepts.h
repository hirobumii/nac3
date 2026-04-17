#pragma once

#include "irrt/stdlib/type_traits.h"

namespace __nac3_impl::stdlib {
namespace {
/**
 * @brief Concept that checks if two types are the same.
 *
 * See https://en.cppreference.com/w/cpp/concepts/same_as.
 */
template<typename T, typename U>
concept same_as = is_same_v<T, U> && is_same_v<U, T>;
}  // namespace
}  // namespace __nac3_impl::stdlib
