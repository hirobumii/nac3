# Break within try statement within a loop
# Taken from https://book.pythontips.com/en/latest/for_-_else.html

@extern
def output_int32(x: int32):
    ...

@extern
def output_float64(x: float):
    ...

@extern
def output_str(x: str):
    ...

def run() -> int32:
    for n in range(2, 10):
        for x in range(2, n):
            try:
                if n % x == 0:
                    output_int32(n)
                    output_str(" equals ")
                    output_int32(x)
                    output_str(" * ")
                    output_float64(n / x)
            except:  # Assume this is intended to catch x == 0
                break
        else:
            # loop fell through without finding a factor
            output_int32(n)
            output_str(" is a prime number")

    return 0