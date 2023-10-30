@extern
def output_bool(x: bool):
    ...

@extern
def dbg_stack_address(x: str) -> uint64:
    ...

def run() -> int32:
    a = dbg_stack_address("a")
    b = dbg_stack_address("b")

    output_bool(a == b)

    return 0