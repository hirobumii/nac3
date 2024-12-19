#pragma once

#include "irrt/int_types.hpp"

// TODO: To be deleted since NDArray with strides is done.

namespace {
template<typename SizeT>
SizeT __nac3_ndarray_calc_size_impl(const SizeT* list_data, SizeT list_len, SizeT begin_idx, SizeT end_idx) {
    __builtin_assume(end_idx <= list_len);

    SizeT num_elems = 1;
    for (SizeT i = begin_idx; i < end_idx; ++i) {
        SizeT val = list_data[i];
        __builtin_assume(val > 0);
        num_elems *= val;
    }
    return num_elems;
}

template<typename SizeT>
void __nac3_ndarray_calc_nd_indices_impl(SizeT index, const SizeT* dims, SizeT num_dims, NDIndexInt* idxs) {
    SizeT stride = 1;
    for (SizeT dim = 0; dim < num_dims; dim++) {
        SizeT i = num_dims - dim - 1;
        __builtin_assume(dims[i] > 0);
        idxs[i] = (index / stride) % dims[i];
        stride *= dims[i];
    }
}
}  // namespace

extern "C" {
uint32_t __nac3_ndarray_calc_size(const uint32_t* list_data, uint32_t list_len, uint32_t begin_idx, uint32_t end_idx) {
    return __nac3_ndarray_calc_size_impl(list_data, list_len, begin_idx, end_idx);
}

uint64_t
__nac3_ndarray_calc_size64(const uint64_t* list_data, uint64_t list_len, uint64_t begin_idx, uint64_t end_idx) {
    return __nac3_ndarray_calc_size_impl(list_data, list_len, begin_idx, end_idx);
}

void __nac3_ndarray_calc_nd_indices(uint32_t index, const uint32_t* dims, uint32_t num_dims, NDIndexInt* idxs) {
    __nac3_ndarray_calc_nd_indices_impl(index, dims, num_dims, idxs);
}

void __nac3_ndarray_calc_nd_indices64(uint64_t index, const uint64_t* dims, uint64_t num_dims, NDIndexInt* idxs) {
    __nac3_ndarray_calc_nd_indices_impl(index, dims, num_dims, idxs);
}
}