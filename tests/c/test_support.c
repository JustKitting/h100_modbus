#include "test_support.h"

#include <stdio.h>
#include <stdlib.h>

struct h100_spindle_input valid_input(void)
{
    struct h100_spindle_input input = {0};
    input.i_control_mode_f001 = 2;
    input.i_frequency_source_f002 = 2;
    input.i_reference_f004_centihz = 4000;
    input.i_maximum_f005_centihz = 4000;
    input.i_panel_stop_f024 = 1;
    input.i_slave_address_f163 = 1;
    input.i_baud_selector_f164 = 2;
    input.i_data_mode_f165 = 3;
    input.i_frequency_decimals_f169 = 0;
    return input;
}

struct h100_spindle_config valid_config(void)
{
    struct h100_spindle_config config = {0};
    config.c_rated_rpm = 24000.0;
    config.c_minimum_rpm = 6000.0;
    config.c_maximum_rpm = 24000.0;
    config.c_expected_reference_f004_centihz = 4000;
    config.c_expected_maximum_f005_centihz = 4000;
    config.c_at_speed_tolerance_hz = 1.0;
    return config;
}

struct h100_spindle_context new_context(void)
{
    struct h100_spindle_context context = {0};
    context.x_state = H100_STOPPING;
    return context;
}

void start_forward_running(
    struct h100_spindle_context *context,
    struct h100_spindle_input *input,
    const struct h100_spindle_config *config,
    struct h100_spindle_output *output)
{
    h100_spindle_step(context, input, config, output);
    input->i_machine_enabled = 1;
    input->i_run_request = 1;
    input->i_forward_request = 1;
    input->i_speed_command_rpm = 12000.0;
    h100_spindle_step(context, input, config, output);
    input->i_given_frequency_readback = 2000;
    h100_spindle_step(context, input, config, output);
    input->i_main_status = H100_STATUS_IN_OPERATION;
    input->i_output_frequency_decihz = 2000;
    h100_spindle_step(context, input, config, output);
    REQUIRE(output->o_state == H100_RUNNING);
}

void h100_test_require(
    int condition,
    const char *expression,
    const char *file,
    int line)
{
    if (!condition) {
        fprintf(stderr, "%s:%d: requirement failed: %s\n", file, line, expression);
        abort();
    }
}
