# For Loop using a list as its iterable

@extern
def output_int32(x: int32):
    ...

def run() -> int32:
    l = [0, 1, 2, 3, 4]

    # i: int32  # declaration-without-initializer not yet supported
    i = 0       # i must be declared before the loop; this is not necessary in Python
    for i in l:
        output_int32(i)
        i = 0
        output_int32(i)
    output_int32(i)
    return 0
