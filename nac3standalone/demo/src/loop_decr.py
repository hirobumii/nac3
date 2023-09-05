# For Loop using a decreasing range() expression as its iterable

@extern
def output_int32(x: int32):
    ...

def run() -> int32:
    i = 0
    for i in range(10, 0, -1):
        output_int32(i)
    output_int32(i)
    return 0
