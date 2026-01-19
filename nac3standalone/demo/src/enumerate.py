@extern
def output_int32(x: int32):
    ...

@extern
def output_str(s: str):
    ...

# ---- Tuple enumeration tests ----
def test_tuple_basic():
    for a, b in enumerate((1, 2, 3, 4)):
        output_int32(a)
        output_int32(b)

def test_tuple_with_start():
    for c, d in enumerate((10, 20, 30), 5):
        output_int32(c)
        output_int32(d)

def test_tuple_single_element():
    for e, f in enumerate((42,), 7):
        output_int32(e)
        output_int32(f)


# ---- List enumeration tests ----
def test_list_basic():
    for g, h in enumerate([5, 6, 7, 8]):
        output_int32(g)
        output_int32(h)

def test_list_with_start():
    for i, j in enumerate([100, 200, 300], 3):
        output_int32(i)
        output_int32(j)

def test_list_single_element():
    for k, l in enumerate([99], 2):
        output_int32(k)
        output_int32(l)


# ---- Empty containers ----
def test_empty_tuple():
    for m, n in enumerate((), 1):
        output_int32(m)

def test_empty_list():
    for o, p in enumerate([], 4):
        output_int32(o)


# ---- Nested tuple elements in list ----
def test_list_of_tuples():
    for q in enumerate([(2, 3), (6, 7), (8, 9)], 1):
        output_int32(q[0])
        output_int32(q[1][0])
        output_int32(q[1][1])


# ---- Iterating over previously defined variables ----
def test_variable_tuple():
    my_tuple = (11, 12, 13, 14)
    for r, s in enumerate(my_tuple):
        output_int32(r)
        output_int32(s)

def test_variable_list():
    my_list = [21, 22, 23, 24]
    for t, u in enumerate(my_list, 10):
        output_int32(t)
        output_int32(u)


# ---- Tuple unpacking ----
def test_unpack_list_of_tuples():
    pairs = [(31, 32), (33, 34), (35, 36)]
    for v in enumerate(pairs):
        output_int32(v[0])
        output_int32(v[1][0])
        output_int32(v[1][1])

# ---- Enumerate with different types ----
def test_different_types():
    mixed_list = [("a", 1), ("b", 2), ("c", 3)]
    for w, x in enumerate(mixed_list):
        output_int32(w)
        output_str(x[0])
        output_int32(x[1])


# ---- Main entry point ----
def run() -> int32:
    # simple tuple/list
    test_tuple_basic()
    test_tuple_with_start()
    test_tuple_single_element()

    test_list_basic()
    test_list_with_start()
    test_list_single_element()

    # empty cases
    test_empty_tuple()
    test_empty_list()

    # tuples inside lists
    test_list_of_tuples()

    # iteration over previously defined variables
    test_variable_tuple()
    test_variable_list()

    # unpacking tests
    test_unpack_list_of_tuples()

    # different types
    test_different_types()

    return 0
