@extern
def output_int32(x: int32, newline: bool=True):
	...

@extern
def output_int64(x: int64, newline: bool=True):
	...

@extern
def output_uint32(x: uint32, newline: bool=True):
	...

@extern
def output_uint64(x: uint64, newline: bool=True):
	...

@extern
def output_int32_list(x: list[int32], newline: bool=True):
	...

@extern
def output_asciiart(x: int32):
	...

@extern
def output_str(x: str, newline: bool=True):
	...

def test_output_int32():
	output_int32(-128)

def test_output_int64():
	output_int64(int64(-256))

def test_output_uint32():
	output_uint32(uint32(128))

def test_output_uint64():
	output_uint64(uint64(256))

def test_output_asciiart():
	for i in range(17):
		output_asciiart(i)
	output_asciiart(0)

def test_output_int32_list():
	output_int32_list([0, 1, 3, 5, 10])

def test_output_str_family():
	output_str("hello ", newline=False)
	output_str("world")

def run() -> int32:
	test_output_int32()
	test_output_int64()
	test_output_uint32()
	test_output_uint64()
	test_output_asciiart()
	test_output_int32_list()
	test_output_str_family()
	return 0