@extern
def output_int32(x: int32):
    ...

# This should do nothing, but still is valid syntax
@staticmethod
def non_member_static_method(x: int32) -> int32:
    return x + 1

class A:
    a: int32 = 3
    b: int32 = 2

    x: int32

    def __init__(self, x: int32):
        self.x = x

    def add(self, y: int32) -> int32:
        return self.x + y

    @staticmethod
    def static_add(x: int32, y: int32) -> int32:
        return x + y

    @staticmethod
    def with_attr() -> int32:
        return A.b + 1

class B:
    @staticmethod
    def static_add(x: int32, y: int32) -> int32:
        return A.static_add(x, y)

def run() -> int32:
    a = A(1)
    output_int32(a.add(2))
    output_int32(A.a)
    output_int32(A.static_add(1, 2))
    output_int32(a.static_add(2, 1)) # static methods can be called on instances
    output_int32(A.with_attr())

    output_int32(non_member_static_method(2))

    output_int32(B.static_add(0, 3))

    return 0
