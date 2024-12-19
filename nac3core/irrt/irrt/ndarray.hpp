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

template<typename SizeT>
void __nac3_ndarray_calc_broadcast_impl(const SizeT* lhs_dims,
                                        SizeT lhs_ndims,
                                        const SizeT* rhs_dims,
                                        SizeT rhs_ndims,
                                        SizeT* out_dims) {
    SizeT max_ndims = lhs_ndims > rhs_ndims ? lhs_ndims : rhs_ndims;

    for (SizeT i = 0; i < max_ndims; ++i) {
        const SizeT* lhs_dim_sz = i < lhs_ndims ? &lhs_dims[lhs_ndims - i - 1] : nullptr;
        const SizeT* rhs_dim_sz = i < rhs_ndims ? &rhs_dims[rhs_ndims - i - 1] : nullptr;
        SizeT* out_dim = &out_dims[max_ndims - i - 1];

        if (lhs_dim_sz == nullptr) {
            *out_dim = *rhs_dim_sz;
        } else if (rhs_dim_sz == nullptr) {
            *out_dim = *lhs_dim_sz;
        } else if (*lhs_dim_sz == 1) {
            *out_dim = *rhs_dim_sz;
        } else if (*rhs_dim_sz == 1) {
            *out_dim = *lhs_dim_sz;
        } else if (*lhs_dim_sz == *rhs_dim_sz) {
            *out_dim = *lhs_dim_sz;
        } else {
            __builtin_unreachable();
        }
    }
}

template<typename SizeT>
void __nac3_ndarray_calc_broadcast_idx_impl(const SizeT* src_dims,
                                            SizeT src_ndims,
                                            const NDIndexInt* in_idx,
                                            NDIndexInt* out_idx) {
    for (SizeT i = 0; i < src_ndims; ++i) {
        SizeT src_i = src_ndims - i - 1;
        out_idx[src_i] = src_dims[src_i] == 1 ? 0 : in_idx[src_i];
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

void __nac3_ndarray_calc_broadcast(const uint32_t* lhs_dims,
                                   uint32_t lhs_ndims,
                                   const uint32_t* rhs_dims,
                                   uint32_t rhs_ndims,
                                   uint32_t* out_dims) {
    return __nac3_ndarray_calc_broadcast_impl(lhs_dims, lhs_ndims, rhs_dims, rhs_ndims, out_dims);
}

void __nac3_ndarray_calc_broadcast64(const uint64_t* lhs_dims,
                                     uint64_t lhs_ndims,
                                     const uint64_t* rhs_dims,
                                     uint64_t rhs_ndims,
                                     uint64_t* out_dims) {
    return __nac3_ndarray_calc_broadcast_impl(lhs_dims, lhs_ndims, rhs_dims, rhs_ndims, out_dims);
}

void __nac3_ndarray_calc_broadcast_idx(const uint32_t* src_dims,
                                       uint32_t src_ndims,
                                       const NDIndexInt* in_idx,
                                       NDIndexInt* out_idx) {
    __nac3_ndarray_calc_broadcast_idx_impl(src_dims, src_ndims, in_idx, out_idx);
}

void __nac3_ndarray_calc_broadcast_idx64(const uint64_t* src_dims,
                                         uint64_t src_ndims,
                                         const NDIndexInt* in_idx,
                                         NDIndexInt* out_idx) {
    __nac3_ndarray_calc_broadcast_idx_impl(src_dims, src_ndims, in_idx, out_idx);
}
}