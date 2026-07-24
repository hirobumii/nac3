# EXPECT: __nac3_raise called. Exception details:
# EXPECT:           ID: 1
# EXPECT:     Location: src/critical_negative.py:LINE:COL
# EXPECT:     Function: run
# EXPECT:      Message: "critical() expects a non-negative page count, got {0}"
# EXPECT:       Params: {0}=-1, {1}=0, {2}=0
#
# The page count passed to `critical(n)` must be non-negative; a negative reservation
# is rejected with `ValueError` at region entry, before any allocation happens.
#
# Unlike the `MemoryError` demos, this check lives in codegen
# (`gen_critical_num_free_pages`) rather than IRRT, so the raise originates from the
# Python source location rather than from `ctrc.hpp`.


def run() -> int32:
    with critical(-1):
        pass

    return 0
