#pragma once

#if __STDC_VERSION__ >= 202000
using int8_t = _BitInt(8);
using uint8_t = unsigned _BitInt(8);
using int32_t = _BitInt(32);
using uint32_t = unsigned _BitInt(32);
using int64_t = _BitInt(64);
using uint64_t = unsigned _BitInt(64);
#else

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-type"
using int8_t = _ExtInt(8);
using uint8_t = unsigned _ExtInt(8);
using int32_t = _ExtInt(32);
using uint32_t = unsigned _ExtInt(32);
using int64_t = _ExtInt(64);
using uint64_t = unsigned _ExtInt(64);
#pragma clang diagnostic pop

#endif

// The type of an index or a value describing the length of a range/slice is always `int32_t`.
using SliceIndex = int32_t;
