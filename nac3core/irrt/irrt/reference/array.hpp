#pragma once

#include "irrt/int_types.hpp"
#include "irrt/reference/header.hpp"
#include "irrt/reference/typeinfo.hpp"

namespace __nac3_impl::reference {
template<typename SizeT>
struct Array {
    ObjectHeader header;
    SizeT refcounted_elems;
};
}  // namespace __nac3_impl::reference
