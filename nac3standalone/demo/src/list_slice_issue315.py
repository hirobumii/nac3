@extern
def output_int32(x: int32):
    ...

@extern
def output_int32_list(x: list[int32]):
    ...

def run() -> int32:
    bl = [True, False]

    bl1 = bl[:]
    bl1[1:] = [True]
    output_int32_list([int32(b) for b in bl1])
    output_int32_list([int32(b) for b in bl1])

    return 0
