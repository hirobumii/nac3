@extern
def output_int32(x: int32):
    ...

class A:
    z: int32
    def __init__(self, val: int32):
        self.z = val

    def add(self, w: int32) -> int32:
        return self.z + w

class B:
    val: int32 = 10

class C:
    @staticmethod
    def static_add(a: int32, b: int32) -> int32:
        return a + b

def run() -> int32:
    a = A(1)
    output_int32(a.add(2))
    output_int32(B.val)
    output_int32(C.static_add(1, 2))

    return 0
