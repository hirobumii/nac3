#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"

#include "irrt/ctrc/mode.hpp"
#include "irrt/reference/header.hpp"
#include "irrt/reference/typeinfo.hpp"

extern "C" {
/**
 * Forward-declared from `irrt/ctrc/ctrc.hpp`.
 */
void __nac3_ctrc_defer_drop(void* object);
}

namespace __nac3_impl::reference {
namespace {
/**
 * @brief A magic value for `Typeinfo::refcounted_field_offsets[0]`, indicating that the object is an array of pointer
 * elements.
 */
constexpr const uint32_t REFCOUNT_ARRAY_MAGIC = 0xffff'ffff;

/**
 * @brief A magic value for `Typeinfo::refcounted_field_offsets[0]`, indicating that the object is an array of inline
 * elements with ObjectHeaders, and that the second element of `refcounted_field_offsets` contains the stride between
 * elements.
 */
constexpr const uint32_t REFCOUNT_ARRAY_INLINE_MAGIC = 0xffff'fffe;

/**
 * @brief Returns a pointer to the start of the user data of the object after the `ObjectHeader`.
 */
[[gnu::always_inline]] void* get_object_start(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return static_cast<void*>(static_cast<unsigned char*>(object) + sizeof(ObjectHeader));
}

/**
 * @brief Returns a pointer to the object header for the given object.
 */
[[gnu::always_inline]] ObjectHeader* get_object_header(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<ObjectHeader*>(object);
}

/**
 * @brief Returns a const pointer to the object header for the given object.
 */
[[gnu::always_inline]] const ObjectHeader* get_object_header(const void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<const ObjectHeader*>(object);
}

/**
 * @brief Returns a pointer to the `Typeinfo` instance for the given object.
 */
[[gnu::always_inline]] const Typeinfo* get_object_typeinfo(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return reinterpret_cast<const Typeinfo*>(&__nac3_global_begin
                                                 + (header->typeinfo_offset & TYPEINFO_OFFSET_MASK));
    }

    return nullptr;
}

/**
 * @brief Checks if the given object is refcounted.
 *
 * An object is reference-counted if it has a non-zero reference count.
 */
[[gnu::always_inline]] bool is_object_refcounted(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return header->refcount != 0;
    }

    return false;
}

/**
 * @brief Checks if the given object was allocated from the CTRC slab.
 *
 * Note that this function does NOT check if the object is refcounted; See `is_object_refcounted`.
 */
[[gnu::always_inline]] bool is_object_ctrc_allocated(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return (header->typeinfo_offset & TYPEINFO_OFFSET_CTRC_BIT) != 0;
    }

    return false;
}

/**
 * @brief Initializes the object header for a newly allocated object.
 *
 * @param is_refcounted Whether the object should be initialized as refcounted (refcount=1) or non-refcounted
 * (refcount=0).
 * @param typeinfo A pointer to the `Typeinfo` instance for the object's type.
 */
[[gnu::always_inline]] void object_header_init(void* const object, bool is_refcounted, const void* const typeinfo) {
    if (auto* const header = get_object_header(object)) {
        header->refcount = is_refcounted ? 1 : 0;
        int32_t typeinfo_offset =
            static_cast<int32_t>(static_cast<const unsigned char*>(typeinfo) - &__nac3_global_begin)
            & TYPEINFO_OFFSET_MASK;
        if (is_refcounted && ctrc::in_ctrc_mode()) {
            // set the CTRC bit if this object is refcounted and allocated when CTRC mode is active
            typeinfo_offset |= TYPEINFO_OFFSET_CTRC_BIT;
        }
        header->typeinfo_offset = typeinfo_offset;
    }
}

/**
 * @brief Increments the reference count of the given object by `count` if it is refcounted.
 */
[[gnu::always_inline]] void refcount_incr_by(void* const object, const size_t count) {
    if (is_object_refcounted(object)) {
        auto* const header = get_object_header(object);
        header->refcount += static_cast<uint32_t>(count);
    }
}

/**
 * @brief Increments the reference count of the given object if it is refcounted.
 */
[[gnu::always_inline]] void refcount_incr(void* const object) {
    refcount_incr_by(object, 1);
}

/**
 * @brief Decrements the reference count of the given object if it is refcounted.
 */
void refcount_decr(void* object);

/**
 * @brief Decrements the reference count of each refcounted child (field or element) of the given object.
 */
void walk_children(void* const object) {
    const auto* const typeinfo = get_object_typeinfo(object);
    const uint32_t num_refcounted_fields = typeinfo->refcounted_field_offsets[0];

    if (num_refcounted_fields == REFCOUNT_ARRAY_MAGIC) {
        // Array of pointer elements - dereference each element and pass to `refcount_decr`
        auto* const obj_start = static_cast<unsigned char*>(get_object_start(object));
        const size_t size = *reinterpret_cast<size_t*>(obj_start);
        for (size_t i = 0; i < size; ++i) {
            void* const field = obj_start + (i + 1) * sizeof(size_t);
            refcount_decr(*reinterpret_cast<void**>(field));
        }
    } else if (num_refcounted_fields == REFCOUNT_ARRAY_INLINE_MAGIC) {
        // Array of inline elements with ObjectHeaders - directly pass each element to `refcount_decr`
        auto* const obj_start = static_cast<unsigned char*>(get_object_start(object));
        const size_t size = *reinterpret_cast<size_t*>(obj_start);
        const uint32_t stride = typeinfo->refcounted_field_offsets[1];
        for (size_t i = 0; i < size; ++i) {
            void* const elem = obj_start + sizeof(size_t) + i * stride;
            refcount_decr(elem);
        }
    } else {
        // Struct with fixed refcounted fields - dereference each refcounted field and pass to `refcount_decr`
        for (uint32_t i = 0; i < num_refcounted_fields; ++i) {
            void* const field =
                static_cast<unsigned char*>(get_object_start(object)) + typeinfo->refcounted_field_offsets[i + 1];
            refcount_decr(*reinterpret_cast<void**>(field));
        }
    }
}

/**
 * @brief Eagerly reclaims a heap object, recursively drops its children, and returns its memory to the system
 * allocator.
 */
void free_heap_object(void* const object) {
    walk_children(object);
    __builtin_free(object);
}

void refcount_decr(void* const object) {
    auto* const header = get_object_header(object);
    if (!header) {
        return;
    }

    if (header->refcount > 0) {
        // refcounted object - decrement refcount and reclaim if refcount reaches zero
        --header->refcount;
        if (header->refcount == 0) {
            if (is_object_ctrc_allocated(object)) {
                // CTRC objects are managed by the CTRC slab allocator - Let the CTRC allocator handle it
                __nac3_ctrc_defer_drop(object);
            } else {
                free_heap_object(object);
            }
        }
    } else {
        // non-refcounted object - just walk children to decrement any refcounted sub-fields
        walk_children(object);
    }
}
}  // namespace
}  // namespace __nac3_impl::reference

extern "C" {
using namespace __nac3_impl;
using namespace __nac3_impl::reference;

/**
 * @brief See `codegen::types::ObjectHeaderValue::init`.
 */
[[gnu::always_inline]] void __nac3_object_header_init(void* object, bool is_refcounted, void* typeinfo) {
    object_header_init(object, is_refcounted, typeinfo);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::is_refcounted`.
 */
[[gnu::always_inline]] bool __nac3_is_object_refcounted(void* object) {
    return is_object_refcounted(object);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::increment_refcount`.
 */
[[gnu::always_inline]] void __nac3_refcount_incr(void* object) {
    refcount_incr(object);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::increment_refcount_by`.
 */
[[gnu::always_inline]] void __nac3_refcount_incr_by(void* object, size_t count) {
    refcount_incr_by(object, count);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::decrement_refcount`.
 */
[[gnu::always_inline]] void __nac3_refcount_decr(void* object) {
    refcount_decr(object);
}
}  // extern "C"
