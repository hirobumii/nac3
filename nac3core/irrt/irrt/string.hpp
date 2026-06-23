#pragma once

#include "irrt/stdlib/cstddef.h"

namespace __nac3_impl {
struct String {
    char* ptr;
    size_t len;
};

namespace {
bool __nac3_str_eq_impl(const char* str1, size_t len1, const char* str2, size_t len2) {
    if (len1 != len2) {
        return 0;
    }
    return __builtin_memcmp(str1, str2, len1) == 0;
}
}  // namespace
}  // namespace __nac3_impl

extern "C" {
using namespace __nac3_impl;

bool nac3_str_eq(const char* str1, size_t len1, const char* str2, size_t len2) {
    return __nac3_str_eq_impl(str1, len1, str2, len2);
}
}
