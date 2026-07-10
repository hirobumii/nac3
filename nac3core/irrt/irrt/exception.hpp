#pragma once

#include "irrt/stdlib/cstdint.h"

#include "irrt/cslice.hpp"

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
ExceptionId EXN_MEMORY_ERROR;
}

/**
 * @brief Extern function to `__nac3_raise`
 *
 * The parameter `err` points to an `Exception` instance.
 */
extern "C" void __nac3_raise(void* err);

namespace {
constexpr int64_t NO_PARAM = 0;
}  // namespace

namespace __nac3_impl {
namespace {
/**
 * @brief NAC3's Exception struct
 */
struct Exception {
    ExceptionId id;
    CSlice filename;
    int32_t line;
    int32_t column;
    CSlice function;
    CSlice msg;
    int64_t params[3];
};

void _raise_exception_helper(ExceptionId id,
                             const char* filename,
                             int32_t line,
                             const char* function,
                             const char* msg,
                             int64_t param0,
                             int64_t param1,
                             int64_t param2) {
    Exception e = {
        .id = id,
        .filename = {.base = reinterpret_cast<void*>(const_cast<char*>(filename)), .len = __builtin_strlen(filename)},
        .line = line,
        .column = 0,
        .function = {.base = reinterpret_cast<void*>(const_cast<char*>(function)), .len = __builtin_strlen(function)},
        .msg = {.base = reinterpret_cast<void*>(const_cast<char*>(msg)), .len = __builtin_strlen(msg)},
        .params = {param0, param1, param2},
    };
    __nac3_raise(reinterpret_cast<void*>(&e));
    __builtin_unreachable();
}
}  // namespace
}  // namespace __nac3_impl

/**
 * @brief Raise an exception with location details (location in the IRRT source files).
 * @param id The ID of the exception to raise.
 * @param msg A global constant C-string of the error message.
 *
 * `param0` to `param2` are optional format arguments of `msg`. They should be set to
 * `NO_PARAM` to indicate they are unused.
 */
#define raise_exception(id, msg, param0, param1, param2) \
    __nac3_impl::_raise_exception_helper(id, __FILE__, __LINE__, __FUNCTION__, msg, param0, param1, param2)
