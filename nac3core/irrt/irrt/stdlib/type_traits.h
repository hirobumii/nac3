#pragma once

#include "irrt/stdlib/cstdint.h"

namespace __nac3_impl::stdlib {
namespace {
/**
 * @brief A compile-time constant value of type T.
 *
 * See https://en.cppreference.com/w/cpp/types/integral_constant.html.
 */
template<typename T, T v>
struct integral_constant {
    /**
     * @brief The value of the constant.
     */
    static constexpr T value = v;
};

/**
 * @brief A compile-time constant value of `true`.
 */
using true_type = integral_constant<bool, true>;

/**
 * @brief A compile-time constant value of `false`.
 */
using false_type = integral_constant<bool, false>;

/**
 * @brief A compile-time structure that provides a `true` value if T and U are the same type, and `false` otherwise.
 *
 * See https://en.cppreference.com/w/cpp/types/is_same.html.
 */
template<typename T, typename U>
struct is_same : false_type {};

template<typename T>
struct is_same<T, T> : true_type {};

/**
 * @brief A compile-time constant value of `true` if T and U are the same type, and `false` otherwise.
 */
template<typename T, typename U>
constexpr bool is_same_v = is_same<T, U>::value;

/**
 * @brief A compile-time structure that provides a member typedef `type` which is the signed version of the type T.
 *
 * See https://en.cppreference.com/w/cpp/types/make_signed.html.
 */
template<typename T>
struct make_signed {};

template<>
struct make_signed<unsigned long> {
    using type = long;
};

template<>
struct make_signed<unsigned long long> {
    using type = long long;
};

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
