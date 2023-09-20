# Different cases for using boolean variables in boolean contexts.
# Tests whether all boolean variables (expressed as i8s) are lowered into i1s before used in branching instruction (`br`)

def bfunc(b: bool) -> bool:
	return not b

def run() -> int32:
	b1 = True
	b2 = False

	if b1:
		pass

	if not b2:
		pass

	while b2:
		pass

	l = [i for i in range(10) if b2]

	b_and = True and False
	b_or = True or False

	b_and = b1 and b2
	b_or = b1 or b2

	bfunc(b1)

	return 0