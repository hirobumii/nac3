#pragma once

using int8_t = _BitInt(8);
using uint8_t = unsigned _BitInt(8);
using int32_t = _BitInt(32);
using uint32_t = unsigned _BitInt(32);
using int64_t = _BitInt(64);
using uint64_t = unsigned _BitInt(64);

// NDArray indices are always `uint32_t`.
using NDIndex = uint32_t;
// The type of an index or a value describing the length of a range/slice is always `int32_t`.
using SliceIndex = int32_t;