#pragma once

#include "irrt/reference/header.hpp"

namespace __nac3_impl::reference {
template<typename SizeT>
struct Array {
    void* data() { return static_cast<void*>(this + 1); }

    ObjectHeader header;
    SizeT refcounted_elems;
};
}  // namespace __nac3_impl::reference
