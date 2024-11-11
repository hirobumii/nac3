#pragma once

#include "irrt/cslice.hpp"
#include "irrt/int_types.hpp"

/**
 * @brief The int type of ARTIQ exception IDs.
 */
using ExceptionId = int32_t;

/*
 * Set of exceptions C++ IRRT can use.
 * Must be synchronized with `setup_irrt_exceptions` in `nac3core/src/codegen/irrt/mod.rs`.
 */
extern "C" {
ExceptionId EXN_INDEX_ERROR;
ExceptionId EXN_VALUE_ERROR;
ExceptionId EXN_ASSERTION_ERROR;
ExceptionId EXN_TYPE_ERROR;
}

/**
 * @brief Extern function to `__nac3_raise`
 *
 * The parameter `err` could be `Exception<int32_t>` or `Exception<int64_t>`. The caller
 * must make sure to pass `Exception`s with the correct `SizeT` depending on the `size_t` of the runtime.
 */
extern "C" void __nac3_raise(void* err);

namespace {
/**
 * @brief NAC3's Exception struct
 */
template<typename SizeT>
struct Exception {
    ExceptionId id;
    CSlice<SizeT> filename;
    int32_t line;
    int32_t column;
    CSlice<SizeT> function;
    CSlice<SizeT> msg;
    int64_t params[3];
};

constexpr int64_t NO_PARAM = 0;

template<typename SizeT>
void _raise_exception_helper(ExceptionId id,
                             const char* filename,
                             int32_t line,
                             const char* function,
                             const char* msg,
                             int64_t param0,
                             int64_t param1,
                             int64_t param2) {
    Exception<SizeT> e = {
        .id = id,
        .filename = {.base = reinterpret_cast<void*>(const_cast<char*>(filename)),
                     .len = static_cast<SizeT>(__builtin_strlen(filename))},
        .line = line,
        .column = 0,
        .function = {.base = reinterpret_cast<void*>(const_cast<char*>(function)),
                     .len = static_cast<SizeT>(__builtin_strlen(function))},
        .msg = {.base = reinterpret_cast<void*>(const_cast<char*>(msg)),
                .len = static_cast<SizeT>(__builtin_strlen(msg))},
    };
    e.params[0] = param0;
    e.params[1] = param1;
    e.params[2] = param2;
    __nac3_raise(reinterpret_cast<void*>(&e));
    __builtin_unreachable();
}
}  // namespace

/**
 * @brief Raise an exception with location details (location in the IRRT source files).
 * @param SizeT The runtime `size_t` type.
 * @param id The ID of the exception to raise.
 * @param msg A global constant C-string of the error message.
 *
 * `param0` to `param2` are optional format arguments of `msg`. They should be set to
 * `NO_PARAM` to indicate they are unused.
 */
#define raise_exception(SizeT, id, msg, param0, param1, param2) \
    _raise_exception_helper<SizeT>(id, __FILE__, __LINE__, __FUNCTION__, msg, param0, param1, param2)
