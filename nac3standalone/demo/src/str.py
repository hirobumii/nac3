@extern
def output_bool(x: bool):
    ...

def test_str_eq():
    output_bool("" == "")
    output_bool("a" == "")
    output_bool("a" == "a")
    output_bool("a" == "b")
    output_bool("test string" == "test string")
    output_bool("Lorem ipsum dolor sit amet" == "Lorem ipsum dolor sit amet")
    output_bool("test1" == "test2")
    output_bool("123" == "123")
    output_bool("123" == "321")
    output_bool("abc" == "abcde")
    output_bool("a" == "aa")
    output_bool(" " == " ")
    output_bool(" a " == " a ")

def test_str_ne():
    output_bool("" != "")
    output_bool("a" != "")
    output_bool("a" != "a")
    output_bool("a" != "b")
    output_bool("test string" != "test string")
    output_bool("Lorem ipsum dolor sit amet" != "Lorem ipsum dolor sit amet")
    output_bool("test1" != "test2")
    output_bool("123" != "123")
    output_bool("123" != "321")
    output_bool("abc" != "abcde")
    output_bool("a" != "aa")
    output_bool(" " != " ")
    output_bool(" a " != " a ")
    
def run() -> int32:
    test_str_eq()
    test_str_ne()
    return 0