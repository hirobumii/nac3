#pragma once

namespace __nac3_impl {
namespace {
template<typename SizeT>
struct CSlice {
    void* base;
    SizeT len;
};
}  // namespace
}  // namespace __nac3_impl
