A = ConstGeneric("A", int32)
B = ConstGeneric("B", uint32)
T = TypeVar("T")

class ConstGenericClass(Generic[A]):
    def __init__(self):
        pass

class ConstGeneric2Class(Generic[A, B]):
    def __init__(self):
        pass

class HybridGenericClass2(Generic[A, T]):
    pass

class HybridGenericClass3(Generic[T, A, B]):
    pass

def make_generic_2() -> ConstGenericClass[Literal[2]]:
    return ...

def make_generic2_1_2() -> ConstGeneric2Class[Literal[1], Literal[2]]:
    return ...

def make_hybrid_class_2_int32() -> HybridGenericClass2[Literal[2], int32]:
    return ...

def make_hybrid_class_i32_0_1() -> HybridGenericClass3[int32, Literal[0], Literal[1]]:
    return ...

def consume_generic_2(instance: ConstGenericClass[Literal[2]]):
    pass

def consume_generic2_1_2(instance: ConstGeneric2Class[Literal[1], Literal[2]]):
    pass

def consume_hybrid_class_2_i32(instance: HybridGenericClass2[Literal[2], int32]):
    pass

def consume_hybrid_class_i32_0_1(instance: HybridGenericClass3[int32, Literal[0], Literal[1]]):
    pass

def f():
    consume_generic_2(make_generic_2())
    consume_generic2_1_2(make_generic2_1_2())
    consume_hybrid_class_2_i32(make_hybrid_class_2_int32())
    consume_hybrid_class_i32_0_1(make_hybrid_class_i32_0_1())

def run() -> int32:
    return 0