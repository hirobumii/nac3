#pragma once

#include "irrt/reference/header.hpp"

namespace __nac3_impl::reference {
template<typename SizeT>
struct Array {
    void* data() { return static_cast<void*>(&elems); }

    ObjectHeader header;
    SizeT refcounted_elems;

    // Since C++ doesn't have flexible array members like C, we declare the `elems` array with a size of 1.
    // In practice, this struct will always be allocated with enough memory to hold the actual number of elements
    // needed.
    //
    // https://devblogs.microsoft.com/oldnewthing/20040826-00/?p=38043
    uint8_t elems[1];
};
}  // namespace __nac3_impl::reference
