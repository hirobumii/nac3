@extern
def output_int32(x: int32):
    ...

def run() -> int32:
    for i in range(4):
        output_int32(i)
        if i < 2:
            continue
        else:
            break

    n = [0, 1, 2, 3]
    for i in n:
        output_int32(i)
        if i < 2:
            continue
        else:
            break

    return 0