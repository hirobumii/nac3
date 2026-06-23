#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"
#include "irrt/stdlib/concepts.h"

#include "irrt/reference/header.hpp"

namespace __nac3_impl::reference {
/**
 * @brief A reference-counted array.
 *
 * Corresponds to `RefCountedArray{Type,Value}` in the compiler sources.
 */
template<typename T = void>
struct Array {
    /**
     * @brief Returns a type-erased pointer to the data of the array.
     */
    void* data()
        requires stdlib::same_as<T, void>
    {
        return static_cast<T*>(&elems);
    }

    /**
     * @brief Returns a type-erased const pointer to the data of the array.
     */
    const void* data() const
        requires stdlib::same_as<T, void>
    {
        return static_cast<const T*>(&elems);
    }

    /**
     * @brief Returns a pointer to the data of the array, cast to the specified target type.
     */
    template<typename Target>
    Target* data()
        requires stdlib::same_as<T, void>
    {
        return reinterpret_cast<Target*>(&elems);
    }

    /**
     * @brief Returns a const pointer to the data of the array, cast to the specified target type.
     */
    template<typename Target>
    const Target* data() const
        requires stdlib::same_as<T, void>
    {
        return reinterpret_cast<const Target*>(&elems);
    }

    /**
     * @brief Returns a typed pointer to the data of the array.
     */
    T* data()
        requires(!stdlib::same_as<T, void>)
    {
        return reinterpret_cast<T*>(&elems);
    }

    /**
     * @brief Returns a typed const pointer to the data of the array.
     */
    const T* data() const
        requires(!stdlib::same_as<T, void>)
    {
        return reinterpret_cast<const T*>(&elems);
    }

    ObjectHeader header;
    size_t refcounted_elems;

    // Since C++ doesn't have flexible array members like C, we declare the `elems` array with a size of 1.
    // In practice, this struct will always be allocated with enough memory to hold the actual number of elements
    // needed.
    //
    // https://devblogs.microsoft.com/oldnewthing/20040826-00/?p=38043
    //
    // Align `elems` to 8 bytes to ensure that the element storage is aligned for both 32-bit and 64-bit targets.
    alignas(8) uint8_t elems[1];
};
}  // namespace __nac3_impl::reference
