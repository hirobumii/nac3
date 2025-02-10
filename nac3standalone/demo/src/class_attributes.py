@extern
def output_int32(x: int32):
    ...

@extern
def output_strln(x: str):
    ...


class A:
    a: int32 = 1
    b: int32
    c: str = "test"
    d: str

    def __init__(self):
        self.b = 2
        self.d = "test"

        output_int32(self.a)  # Attributes can be accessed within class        


def run() -> int32:
    output_int32(A.a)  # Attributes can be directly accessed with class name
    # A.b # Only attributes can be accessed in this way
    # A.a = 2  # Attributes are immutable

    obj = A()
    output_int32(obj.a)  # Attributes can be accessed by class objects

    output_strln(obj.c)
    output_strln(obj.d)

    return 0

