#pragma once

#include "irrt/reference/header.hpp"
#include "irrt/reference/typeinfo.hpp"

namespace __nac3_impl::reference {
namespace {
constexpr const uint32_t REFCOUNT_ARRAY_MAGIC = 0xffff'ffff;
constexpr const uint32_t REFCOUNT_ARRAY_INLINE_MAGIC = 0xffff'fffe;

void* get_object_start(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return static_cast<void*>(static_cast<unsigned char*>(object) + sizeof(ObjectHeader));
}

const void* get_object_start(const void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return static_cast<const void*>(static_cast<const unsigned char*>(object) + sizeof(ObjectHeader));
}

ObjectHeader* get_object_header(void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<ObjectHeader*>(object);
}

const ObjectHeader* get_object_header(const void* object) {
    if (object == nullptr) {
        return nullptr;
    }

    return reinterpret_cast<const ObjectHeader*>(object);
}

template<typename SizeT>
const Typeinfo<SizeT>* get_object_typeinfo(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return reinterpret_cast<const Typeinfo<SizeT>*>(&__nac3_global_begin + header->typeinfo_offset);
    }

    return nullptr;
}

bool is_object_refcounted(const void* object) {
    if (const auto* const header = get_object_header(object)) {
        return header->refcount != 0;
    }

    return false;
}

void object_header_init(void* const object, bool is_refcounted, const void* const typeinfo) {
    if (auto* const header = get_object_header(object)) {
        header->refcount = is_refcounted ? 1 : 0;
        header->typeinfo_offset = static_cast<const unsigned char*>(typeinfo) - &__nac3_global_begin;
    }
}

template<typename SizeT>
void refcount_incr(void* const object) {
    if (is_object_refcounted(object)) {
        auto* const header = get_object_header(object);
        ++header->refcount;
    }
}

template<typename SizeT>
void refcount_decr(void* const object);

template<typename SizeT>
void walk_children(void* const object) {
    const auto* const typeinfo = get_object_typeinfo<SizeT>(object);
    const uint32_t num_refcounted_fields = typeinfo->refcounted_field_offsets[0];

    if (num_refcounted_fields == REFCOUNT_ARRAY_MAGIC) {
        // Array of pointer elements - dereference each
        auto* const obj_start = static_cast<unsigned char*>(get_object_start(object));
        const SizeT size = *reinterpret_cast<SizeT*>(obj_start);
        for (SizeT i = 0; i < size; ++i) {
            void* const field = obj_start + (i + 1) * sizeof(SizeT);
            refcount_decr<SizeT>(*reinterpret_cast<void**>(field));
        }
    } else if (num_refcounted_fields == REFCOUNT_ARRAY_INLINE_MAGIC) {
        // Array of inline elements with ObjectHeaders - pass address, don't dereference
        auto* const obj_start = static_cast<unsigned char*>(get_object_start(object));
        const SizeT size = *reinterpret_cast<SizeT*>(obj_start);
        const uint32_t stride = typeinfo->refcounted_field_offsets[1];
        for (SizeT i = 0; i < size; ++i) {
            void* const elem = obj_start + sizeof(SizeT) + i * stride;
            refcount_decr<SizeT>(elem);
        }
    } else {
        // Struct with fixed refcounted fields - dereference each
        for (uint32_t i = 0; i < num_refcounted_fields; ++i) {
            void* const field =
                static_cast<unsigned char*>(get_object_start(object)) + typeinfo->refcounted_field_offsets[i + 1];
            refcount_decr<SizeT>(*reinterpret_cast<void**>(field));
        }
    }
}

template<typename SizeT>
void refcount_decr(void* const object) {
    auto* const header = get_object_header(object);
    if (!header) return;

    if (header->refcount > 0) {
        // Refcounted object - decrement
        --header->refcount;
        if (header->refcount == 0) {
            walk_children<SizeT>(object);
            __builtin_free(object);
        }
    } else {
        // Non-refcounted object (e.g. inline tuple with refcount=0)
        // Walk children to decrement any refcounted sub-fields, but don't free
        walk_children<SizeT>(object);
    }
}
}  // namespace
}  // namespace __nac3_impl::reference

extern "C" {
void __nac3_object_header_init(void* object, bool is_refcounted, void* typeinfo) {
    __nac3_impl::reference::object_header_init(object, is_refcounted, typeinfo);
}

bool __nac3_is_object_refcounted(void* object) {
    return __nac3_impl::reference::is_object_refcounted(object);
}

void __nac3_refcount_incr(void* object) {
    __nac3_impl::reference::refcount_incr<uint32_t>(object);
}

void __nac3_refcount_incr64(void* object) {
    __nac3_impl::reference::refcount_incr<uint64_t>(object);
}

void __nac3_refcount_decr(void* object) {
    __nac3_impl::reference::refcount_decr<uint32_t>(object);
}

void __nac3_refcount_decr64(void* object) {
    __nac3_impl::reference::refcount_decr<uint64_t>(object);
}
}  // extern "C"
