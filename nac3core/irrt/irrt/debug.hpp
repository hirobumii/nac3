#pragma once

// Set in nac3core/build.rs
#ifdef IRRT_DEBUG_ASSERT
#define IRRT_DEBUG_ASSERT_BOOL true
#else
#define IRRT_DEBUG_ASSERT_BOOL false
#endif

#define raise_debug_assert(SizeT, msg, param1, param2, param3)                                                         \
    raise_exception(SizeT, EXN_ASSERTION_ERROR, "IRRT debug assert failed: " msg, param1, param2, param3);

#define debug_assert_eq(SizeT, lhs, rhs)                                                                               \
    if (IRRT_DEBUG_ASSERT_BOOL && (lhs) != (rhs))                                                                      \
    {                                                                                                                  \
        raise_debug_assert(SizeT, "LHS = {0}. RHS = {1}", lhs, rhs, NO_PARAM);                                         \
    }

#define debug_assert(SizeT, expr)                                                                                      \
    if (IRRT_DEBUG_ASSERT_BOOL && !(expr))                                                                             \
    {                                                                                                                  \
        raise_debug_assert(SizeT, "Got false.", NO_PARAM, NO_PARAM, NO_PARAM);                                         \
    }