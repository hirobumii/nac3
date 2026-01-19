from min_artiq import *
import numpy as np

enumerated_tuple = enumerate((1, 2, 3))
enumerated_tuple2 = enumerate((1.1, 2.2, 3.3, 4.4))
enumerated_tuple3 = enumerate(())
enumerated_list = enumerate([1, 2, 3])
enumerated_list2 = enumerate(["a", "b", "c", "d"])
enumerated_list3 = enumerate([])

@compile
class Demo:
    core: KernelInvariant[Core]
    led0: KernelInvariant[TTLOut]
    led1: KernelInvariant[TTLOut]

    def __init__(self):
        self.core = Core()
        self.led0 = TTLOut(self.core, 18)
        self.led1 = TTLOut(self.core, 19)

    @kernel
    def test(self):
        a = enumerated_tuple
        b = enumerated_tuple2
        c = enumerated_list
        d = enumerated_list2

        for x in enumerated_tuple:
            x[0]
            x[1]
        for y in enumerated_tuple2:
            y[0]
            y[1]
        for z in enumerated_tuple3:
            z[0]
            z[1]
        for p in enumerated_list:
            p[0]
            p[1]
        for q in enumerated_list2:
            q[0]
            q[1]
        for r in enumerated_list3:
            r[0]
            r[1]

    def run(self):
        self.test()



if __name__ == "__main__":
    Demo().run()
