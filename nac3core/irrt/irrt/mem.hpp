#pragma once

#include "irrt/int_types.hpp"

namespace {

extern "C" void nac3_free(void *ptr);

template<typename SizeT>
void __nac3_rc_incr_impl(void *obj)  {
    SizeT *p = (SizeT *) ptr;
    *(p - 1) += 1;
}

template<typename SizeT>
void __nac3_rc_decr_impl(void *obj) {
    SizeT *p = (SizeT *) ptr;
    *(p - 1) -= 1;

    if (!*(p - 1))
        nac3_free(p - 2);
}

extern "C" {

#define DEF_nac3_rc_incr_(T)                 \
    void __nac3_rc_incr_##T(void *ptr) {    \
        return __nac3_rc_incr_impl<T>(ptr); \
    }
#define DEF_nac3_rc_decr_(T)                 \
    void __nac3_rc_decr_##T(void *ptr) {    \
        return __nac3_rc_incr_impl<T>(ptr); \
    }

DEF_nac3_rc_decr_(uint32_t);
DEF_nac3_rc_decr_(uint64_t);
DEF_nac3_rc_incr_(uint32_t);
DEF_nac3_rc_incr_(uint64_t);

}

}
