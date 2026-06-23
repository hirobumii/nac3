#pragma once

#include "irrt/stdlib/cstddef.h"
#include "irrt/stdlib/cstdint.h"

#include "irrt/reference/header.hpp"
#include "irrt/reference/typeinfo.hpp"

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
void* get_object_start(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return static_cast<void*>(static_cast<unsigned char*>(object) + sizeof(ObjectHeader));
}

/**
 * @brief Returns a pointer to the object header for the given object.
 */
ObjectHeader* get_object_header(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<ObjectHeader*>(object);
}

/**
 * @brief Returns a const pointer to the object header for the given object.
 */
const ObjectHeader* get_object_header(const void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<const ObjectHeader*>(object);
}

/**
 * @brief Returns a pointer to the `Typeinfo` instance for the given object.
 */
const Typeinfo* get_object_typeinfo(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return reinterpret_cast<const Typeinfo*>(&__nac3_global_begin + header->typeinfo_offset);
    }

    return nullptr;
}

/**
 * @brief Checks if the given object is refcounted.
 *
 * An object is reference-counted if it has a non-zero reference count.
 */
bool is_object_refcounted(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return header->refcount != 0;
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
void object_header_init(void* const object, bool is_refcounted, const void* const typeinfo) {
    if (auto* const header = get_object_header(object)) {
        header->refcount = is_refcounted ? 1 : 0;
        header->typeinfo_offset = static_cast<const unsigned char*>(typeinfo) - &__nac3_global_begin;
    }
}

/**
 * @brief Increments the reference count of the given object if it is refcounted.
 */
void refcount_incr(void* const object) {
    if (is_object_refcounted(object)) {
        auto* const header = get_object_header(object);
        ++header->refcount;
    }
}

/**
 * @brief Decrements the reference count of the given object if it is refcounted.
 */
void refcount_decr(void* const object) {
    static constexpr auto walk_children = [](void* const object) {
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
    };

    auto* const header = get_object_header(object);
    if (!header) {
        return;
    }

    if (header->refcount > 0) {
        // refcounted object - decrement refcount and free if refcount reaches zero
        --header->refcount;
        if (header->refcount == 0) {
            walk_children(object);
            __builtin_free(object);
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
void __nac3_object_header_init(void* object, bool is_refcounted, void* typeinfo) {
    object_header_init(object, is_refcounted, typeinfo);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::is_refcounted`.
 */
bool __nac3_is_object_refcounted(void* object) {
    return is_object_refcounted(object);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::increment_refcount`.
 */
void __nac3_refcount_incr(void* object) {
    refcount_incr(object);
}

/**
 * @brief See `codegen::types::ObjectHeaderValue::decrement_refcount`.
 */
void __nac3_refcount_decr(void* object) {
    refcount_decr(object);
}
}  // extern "C"
