#ifndef H100_TEST_SUPPORT_H
#define H100_TEST_SUPPORT_H

#include "h100_spindle_logic.h"

#include <stdint.h>

struct h100_spindle_input valid_input(void);
struct h100_spindle_config valid_config(void);
struct h100_spindle_context new_context(void);

void start_forward_running(
    struct h100_spindle_context *context,
    struct h100_spindle_input *input,
    const struct h100_spindle_config *config,
    struct h100_spindle_output *output);

void test_block_code_suite(void);
void test_state_machine_suite(void);
void test_invariant_suite(void);

void h100_test_require(
    int condition,
    const char *expression,
    const char *file,
    int line);

#define REQUIRE(expression) \
    h100_test_require((expression), #expression, __FILE__, __LINE__)

#endif
