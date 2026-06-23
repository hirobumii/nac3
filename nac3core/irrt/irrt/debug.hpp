#pragma once

#include "irrt/exception.hpp"

// Set in nac3core/build.rs
#ifdef IRRT_DEBUG_ASSERT
#define IRRT_DEBUG_ASSERT_BOOL true
#else
#define IRRT_DEBUG_ASSERT_BOOL false
#endif

#define raise_debug_assert(msg, param1, param2, param3) \
    raise_exception(EXN_ASSERTION_ERROR, "IRRT debug assert failed: " msg, param1, param2, param3)

#define debug_assert_eq(lhs, rhs)                                           \
    if constexpr (IRRT_DEBUG_ASSERT_BOOL) {                                 \
        if ((lhs) != (rhs)) {                                               \
            raise_debug_assert("LHS = {0}. RHS = {1}", lhs, rhs, NO_PARAM); \
        }                                                                   \
    }

#define debug_assert(expr)                                                  \
    if constexpr (IRRT_DEBUG_ASSERT_BOOL) {                                 \
        if (!(expr)) {                                                      \
            raise_debug_assert("Got false.", NO_PARAM, NO_PARAM, NO_PARAM); \
        }                                                                   \
    }
