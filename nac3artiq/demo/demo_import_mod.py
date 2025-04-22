from min_artiq import kernel, KernelInvariant, nac3
import min_artiq as artiq


@nac3
class Demo:
    core: KernelInvariant[artiq.Core]
    led0: KernelInvariant[artiq.TTLOut]
    led1: KernelInvariant[artiq.TTLOut]

    def __init__(self):
        self.core = artiq.Core()
        self.led0 = artiq.TTLOut(self.core, 18)
        self.led1 = artiq.TTLOut(self.core, 19)

    @kernel
    def run(self):
        self.core.reset()
        while True:
            with artiq.parallel:
                self.led0.pulse(100.*artiq.ms)
                self.led1.pulse(100.*artiq.ms)
            self.core.delay(100.*artiq.ms)


if __name__ == "__main__":
    Demo().run()
