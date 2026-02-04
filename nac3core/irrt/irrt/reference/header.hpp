#pragma once

#include "irrt/int_types.hpp"

extern "C" const unsigned char __nac3_global_begin;

namespace __nac3_impl::reference {
struct ObjectHeader {
    uint32_t refcount;
    int32_t typeinfo_offset;
};
}  // namespace __nac3_impl::reference
