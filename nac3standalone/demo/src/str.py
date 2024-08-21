@extern
def output_bool(x: bool):
    ...


def str_eq():
    output_bool("" == "")
    output_bool("a" == "")
    output_bool("a" == "b")
    output_bool("b" == "a")
    output_bool("a" == "a")
    output_bool("test string" == "test string")
    output_bool("test string1" == "test string2")


def str_ne():
    output_bool("" != "")
    output_bool("a" != "")
    output_bool("a" != "b")
    output_bool("b" != "a")
    output_bool("a" != "a")
    output_bool("test string" != "test string")
    output_bool("test string1" != "test string2")


def run() -> int32:
    str_eq()
    str_ne()

    return 0
