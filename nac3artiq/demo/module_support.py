from min_artiq import *
import module as module_definition

@compile
class TestModuleSupport:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        # Accessing classes
        obj = module_definition.A()
        obj.get_X()
        obj.set_x(2)
        
        # Calling functions
        module_definition.display_X()

        # Updating global variables
        module_definition.X = 9
        module_definition.display_X()

if __name__ == "__main__":
    TestModuleSupport().run()