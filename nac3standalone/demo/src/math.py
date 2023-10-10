@extern
def output_bool(x: bool):
    ...

@extern
def dbl_nan() -> float:
    ...

@extern
def dbl_inf() -> float:
    ...

def test_isnan():
    for x in [dbl_nan(), 0.0, dbl_inf()]:
        output_bool(isnan(x))

def test_isinf():
    for x in [dbl_inf(), 0.0, dbl_nan()]:
        output_bool(isinf(x))

def run() -> int32:
    test_isnan()
    test_isinf()

    return 0