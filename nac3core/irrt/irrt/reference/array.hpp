#pragma once

#include "irrt/stdlib/concepts.h"

#include "irrt/reference/header.hpp"

namespace __nac3_impl::reference {
template<typename SizeT, typename T = void>
struct Array {
    void* data()
        requires stdlib::same_as<T, void>
    {
        return static_cast<T*>(&elems);
    }

    const void* data() const
        requires stdlib::same_as<T, void>
    {
        return static_cast<const T*>(&elems);
    }

    template<typename Target>
    Target* data()
        requires stdlib::same_as<T, void>
    {
        return reinterpret_cast<Target*>(&elems);
    }

    template<typename Target>
    const Target* data() const
        requires stdlib::same_as<T, void>
    {
        return reinterpret_cast<const Target*>(&elems);
    }

    T* data()
        requires(!stdlib::same_as<T, void>)
    {
        return reinterpret_cast<T*>(&elems);
    }

    const T* data() const
        requires(!stdlib::same_as<T, void>)
    {
        return reinterpret_cast<const T*>(&elems);
    }

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
