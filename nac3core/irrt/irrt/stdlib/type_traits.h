#pragma once

#include "irrt/stdlib/cstdint.h"

namespace __nac3_impl::stdlib {
namespace {
template<typename T, T v>
struct integral_constant {
    static constexpr T value = v;
};

using true_type = integral_constant<bool, true>;
using false_type = integral_constant<bool, false>;

template<typename T, typename U>
struct is_same : false_type {};

template<typename T>
struct is_same<T, T> : true_type {};

template<typename T, typename U>
constexpr bool is_same_v = is_same<T, U>::value;

template<typename T>
struct make_signed {};

template<>
struct make_signed<uint32_t> {
    using type = int32_t;
};

template<>
struct make_signed<uint64_t> {
    using type = int64_t;
};

template<typename T>
using make_signed_t = typename make_signed<T>::type;
}  // namespace
}  // namespace __nac3_impl::stdlib
