@extern
def output_bool(x: bool):
    ...


def str_eq():
    # Basic cases
    output_bool("" == "")
    output_bool("a" == "")
    output_bool("a" == "b")
    output_bool("b" == "a")
    output_bool("a" == "a")

    # Longer identical strings
    output_bool("test string" == "test string")
    output_bool("Lorem ipsum dolor sit amet" == "Lorem ipsum dolor sit amet")

    # Different by one character
    output_bool("test string1" == "test string2")

    # Numeric strings
    output_bool("123" == "123")
    output_bool("123" == "321")

    # Different lengths
    output_bool("abc" == "abcde")

    # Leading and trailing spaces
    output_bool(" leading space" == "leading space")
    output_bool("trailing space " == "trailing space")
    output_bool("  " == "  ")

def str_ne():
    # Basic cases
    output_bool("" != "")
    output_bool("a" != "")
    output_bool("a" != "b")
    output_bool("b" != "a")
    output_bool("a" != "a")

    # Longer identical strings
    output_bool("test string" != "test string")

    # Different by one character
    output_bool("test string1" != "test string2")

    # Numeric strings
    output_bool("123" != "123")
    output_bool("123" != "321")

    # Different lengths
    output_bool("abc" != "abcde")

    # Leading and trailing spaces
    output_bool(" leading space" != "leading space")
    output_bool("trailing space " != "trailing space")
    output_bool("  " != "  ")

def run() -> int32:
    str_eq()
    str_ne()

    return 0
