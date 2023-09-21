# For Loop using an range() expression as its iterable, additionally reassigning the target on each iteration

@extern
def output_int32(x: int32, newline: bool=True):
    ...

def run() -> int32:
    i = 0
    for i in range(10):
        output_int32(i)
        i = 0
        output_int32(i)
    output_int32(i)
    return 0
