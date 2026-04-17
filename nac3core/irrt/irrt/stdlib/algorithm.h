#pragma once

namespace __nac3_impl::stdlib {
namespace {
/**
 * @brief Returns the maximum of the two arguments.
 *
 * See https://en.cppreference.com/w/cpp/algorithm/max.html.
 */
template<typename T>
const T& max(const T& a, const T& b) {
    return a > b ? a : b;
}

/**
 * @brief Returns the minimum of the two arguments.
 *
 * See https://en.cppreference.com/w/cpp/algorithm/min.html.
 */
template<typename T>
const T& min(const T& a, const T& b) {
    return a > b ? b : a;
}
}  // namespace
}  // namespace __nac3_impl::stdlib
