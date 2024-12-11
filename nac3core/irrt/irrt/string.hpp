#pragma once

#include "irrt/int_types.hpp"
namespace {
template<typename SizeT>
int32_t __nac3_str_eq_impl(const char* str1, SizeT len1, const char* str2, SizeT len2) {
    if (str1 == str2) return 1;
    if (len1 != len2) return 0;
    for (SizeT i = 0; i < len1; ++i) {
        if (static_cast<unsigned char>(str1[i]) != static_cast<unsigned char>(str2[i])) {
            return 0;
        }
    }
    return 1;
}
}  // namespace

extern "C" {
int32_t nac3_str_eq(const char* str1, uint64_t len1, const char* str2, uint64_t len2) {
    return __nac3_str_eq_impl<uint64_t>(str1, len1, str2, len2);
}
}