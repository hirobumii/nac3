#pragma once

#include "irrt/int_types.hpp"

namespace {

extern "C" void nac3_free(void *ptr);

template<typename SizeT>
void __nac3_rc_incr_impl(void *obj)  {
    SizeT *p = (SizeT *) obj;
    *(p - 1) += 1;
}

template<typename SizeT>
void __nac3_rc_decr_impl(void *obj) {
    SizeT *p = (SizeT *) obj;
    *(p - 1) -= 1;

    if (!*(p - 1))
        nac3_free(p - 2);
}

extern "C" {

void nac3_rc_decr(int8_t *ptr) {
    __nac3_rc_decr_impl<uint32_t>(ptr);
}

void nac3_rc_decr64(int8_t *ptr) {
    __nac3_rc_decr_impl<uint64_t>(ptr);
}

void nac3_rc_incr(int8_t *ptr) {
    __nac3_rc_incr_impl<uint32_t>(ptr);
}

void nac3_rc_incr64(int8_t *ptr) {
    __nac3_rc_incr_impl<uint64_t>(ptr);
}

}

}
