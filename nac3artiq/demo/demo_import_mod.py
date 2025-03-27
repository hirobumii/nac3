from min_artiq import kernel, KernelInvariant, nac3
import min_artiq


@nac3
class Demo:
    core: KernelInvariant[min_artiq.Core]
    led0: KernelInvariant[min_artiq.TTLOut]
    led1: KernelInvariant[min_artiq.TTLOut]

    def __init__(self):
        self.core = min_artiq.Core()
        self.led0 = min_artiq.TTLOut(self.core, 18)
        self.led1 = min_artiq.TTLOut(self.core, 19)

    @kernel
    def run(self):
        self.core.reset()
        while True:
            with min_artiq.parallel:
                self.led0.pulse(100.*min_artiq.ms)
                self.led1.pulse(100.*min_artiq.ms)
            self.core.delay(100.*min_artiq.ms)


if __name__ == "__main__":
    Demo().run()
