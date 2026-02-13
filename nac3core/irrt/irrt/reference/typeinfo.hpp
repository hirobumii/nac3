#pragma once

#include "irrt/int_types.hpp"
#include "irrt/string.hpp"

namespace __nac3_impl::reference {
template<typename SizeT>
struct Typeinfo {
    String<SizeT>* name;
    uint32_t* refcounted_field_offsets;
};
}  // namespace __nac3_impl::reference
