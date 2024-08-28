#pragma once

#include "irrt/int_types.hpp"

template<typename SizeT>
struct CSlice {
    uint8_t* base;
    SizeT len;
};