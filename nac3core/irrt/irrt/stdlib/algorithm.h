#pragma once

namespace __nac3_impl::stdlib {
namespace {
template<typename T>
const T& max(const T& a, const T& b) {
    return a > b ? a : b;
}

template<typename T>
const T& min(const T& a, const T& b) {
    return a > b ? b : a;
}
}  // namespace
}  // namespace __nac3_impl::stdlib
