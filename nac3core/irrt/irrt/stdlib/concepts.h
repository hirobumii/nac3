#pragma once

#include "irrt/stdlib/type_traits.h"

namespace __nac3_impl::stdlib {
namespace {
template<typename T, typename U>
concept same_as = is_same_v<T, U> && is_same_v<U, T>;
}  // namespace
}  // namespace __nac3_impl::stdlib
