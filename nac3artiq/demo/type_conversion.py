from min_artiq import *
enumerated_tuple = enumerate((1, 2, 3))
enumerated_list = enumerate([1, 2, 3])

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
        b = enumerated_list

        # Doesn't work yet, need the gencode part to support it
        # for x in enumerated_tuple:
        #     x[0]
        #     x[1]
        # for y in enumerated_list:
        #     y[0]
        #     y[1]

    def run(self):
        self.test()



if __name__ == "__main__":
    Demo().run()
