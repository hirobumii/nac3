#pragma once

template<typename SizeT>
struct CSlice {
    void* base;
    SizeT len;
};
