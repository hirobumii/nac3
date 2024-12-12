#pragma once

#include "irrt/int_types.hpp"

namespace {
template<typename SizeT>
int32_t __nac3_str_eq_impl(const char* str1, SizeT len1, const char* str2, SizeT len2) {
    if (len1 != len2){
        return 0;
    }
    return (__builtin_strncmp(str1, str2, static_cast<SizeT>(len1)) == 0) ? 1 : 0;
}
}  // namespace

extern "C" {
int32_t nac3_str_eq(const char* str1, uint64_t len1, const char* str2, uint64_t len2) {
    return __nac3_str_eq_impl<uint64_t>(str1, len1, str2, len2);
}

int32_t nac3_str_eq_i32(const char* str1, uint32_t len1, const char* str2, uint32_t len2) {
    return __nac3_str_eq_impl<uint32_t>(str1, len1, str2, len2);
}
}