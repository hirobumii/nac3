#pragma once

#include "irrt/int_types.hpp"

template<typename SizeT>
struct CSlice {
    void* base;
    SizeT len;
};