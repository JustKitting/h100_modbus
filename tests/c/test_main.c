#include "test_support.h"

#include <stdio.h>

int main(void)
{
    test_block_code_suite();
    test_state_machine_suite();
    test_invariant_suite();
    puts("h100 spindle sequencer tests passed");
    return 0;
}
