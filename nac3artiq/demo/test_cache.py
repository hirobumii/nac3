from min_artiq import *
from numpy import int32

# Simulate CacheError from ARTIQ
class CacheError(Exception):
    pass

@nac3
class _Cache:
    core: KernelInvariant[Core]
    # Properly declare core_cache as a KernelInvariant
    core_cache: KernelInvariant[dict[str, list[int32]]]

    def __init__(self):
        self.core = Core()
        self.core_cache = {}

    def build(self):
        # Simplified version of EnvExperiment.build()
        # Core and core_cache are already set in __init__
        pass

    @kernel
    def get(self, key: str) -> list[int32]:
        return self.core_cache.get(key, [])

    @kernel
    def put(self, key: str, value: list[int32]):
        self.core_cache[key] = value

    @kernel
    def get_put(self, key: str, value: list[int32]):
        self.get(key)
        self.put(key, value)

class CacheTest:
    def create(self, cls):
        return cls()

    def assertEqual(self, a, b):
        assert a == b, f"Expected {a} to equal {b}"

    def assertRaises(self, exc_type):
        class RaiseContext:
            def __init__(self, test_case, expected_exc):
                self.test_case = test_case
                self.expected_exc = expected_exc
            def __enter__(self):
                return self
            def __exit__(self, exc_type, exc_val, exc_tb):
                if exc_type is None:
                    raise AssertionError(f"Expected {self.expected_exc}")
                if not issubclass(exc_type, self.expected_exc):
                    raise AssertionError(f"Expected {self.expected_exc}, got {exc_type}")
                return True
        return RaiseContext(self, exc_type)

    def test_get_empty(self):
        exp = self.create(_Cache)
        self.assertEqual(exp.get("x1"), [])

    def test_put_get(self):
        exp = self.create(_Cache)
        exp.put("x2", [int32(1), int32(2), int32(3)])
        self.assertEqual(exp.get("x2"), [int32(1), int32(2), int32(3)])

    def test_replace(self):
        exp = self.create(_Cache)
        exp.put("x3", [int32(1), int32(2), int32(3)])
        exp.put("x3", [int32(1), int32(2), int32(3), int32(4), int32(5)])
        self.assertEqual(exp.get("x3"), [int32(1), int32(2), int32(3), int32(4), int32(5)])

    def test_borrow(self):
        exp = self.create(_Cache)
        exp.put("x4", [int32(1), int32(2), int32(3)])
        with self.assertRaises(CacheError):
            exp.get_put("x4", [])

if __name__ == "__main__":
    test = CacheTest()
    test.test_get_empty()
    test.test_put_get()
    test.test_replace()
    test.test_borrow()
