from min_artiq import *
import tests.string_attribute_issue337 as issue337
import tests.support_class_attr_issue102 as issue102
import tests.global_variables as global_variables

@nac3
class TestModuleSupport:
    core: KernelInvariant[Core]

    def __init__(self):
        self.core = Core()

    @kernel
    def run(self):
        # Accessing classes
        issue337.Demo().run()
        obj = issue102.Demo()
        obj.attr3 = 3
        
        # Calling functions
        global_variables.inc_X()
        global_variables.display_X()

        # Updating global variables
        global_variables.X = 9
        global_variables.display_X()

if __name__ == "__main__":
    TestModuleSupport().run()