#include "test_support.h"

#include <math.h>

static int active_state(uint32_t state)
{
    return state == H100_STARTING || state == H100_RUNNING;
}

static void test_output_invariants_for_all_boolean_combinations(void)
{
    static const double speeds[] = {0.0, 5999.0, 12000.0, 24001.0, NAN};
    static const uint32_t frequencies[] = {0, 1990, 2000, 2010};
    int state;
    unsigned int flags;
    unsigned int previous_reset;
    unsigned int held_reverse;
    unsigned int speed_index;
    unsigned int frequency_index;

    for (state = H100_STOPPED; state <= H100_FAULT; ++state) {
        for (flags = 0; flags < 256; ++flags) {
            for (previous_reset = 0; previous_reset < 2; ++previous_reset) {
                for (held_reverse = 0; held_reverse < 2; ++held_reverse) {
                    for (speed_index = 0;
                         speed_index < sizeof(speeds) / sizeof(speeds[0]);
                         ++speed_index) {
                        for (frequency_index = 0;
                             frequency_index <
                                 sizeof(frequencies) / sizeof(frequencies[0]);
                             ++frequency_index) {
                            struct h100_spindle_input input = valid_input();
                            struct h100_spindle_config config = valid_config();
                            struct h100_spindle_context context = new_context();
                            struct h100_spindle_output output;
                            uint32_t base_reason;

                            context.x_state = state;
                            context.x_previous_reset = (int)previous_reset;
                            context.x_fault_latched = (flags >> 7) & 1u;
                            context.x_fault_code = context.x_fault_latched ?
                                H100_BLOCK_LINK : H100_BLOCK_NONE;
                            context.x_held_frequency = 2000;
                            context.x_held_target_hz = 200.0;
                            context.x_held_reverse = (int)held_reverse;

                            input.i_machine_enabled = flags & 1u;
                            input.i_run_request = (flags >> 1) & 1u;
                            input.i_forward_request = (flags >> 2) & 1u;
                            input.i_reverse_request = (flags >> 3) & 1u;
                            input.i_reset = (flags >> 4) & 1u;
                            input.i_link_fault = (flags >> 5) & 1u;
                            input.i_any_command_disabled = (flags >> 6) & 1u;
                            input.i_main_status = (flags >> 7) & 1u ?
                                H100_STATUS_IN_OPERATION : 0;
                            input.i_speed_command_rpm = speeds[speed_index];
                            input.i_output_frequency_decihz =
                                frequencies[frequency_index];

                            base_reason = h100_configuration_block(&input, &config);
                            h100_spindle_step(&context, &input, &config, &output);

                            REQUIRE(output.o_state <= H100_FAULT);
                            REQUIRE(output.o_main_control == H100_CONTROL_STOP ||
                                output.o_main_control == H100_CONTROL_FORWARD ||
                                output.o_main_control == H100_CONTROL_REVERSE);
                            REQUIRE(!(output.o_forward_running &&
                                output.o_reverse_running));
                            REQUIRE(!output.o_forward_running || output.o_running);
                            REQUIRE(!output.o_reverse_running || output.o_running);
                            REQUIRE(!output.o_fault_latched ||
                                output.o_main_control == H100_CONTROL_STOP);
                            REQUIRE(output.o_ready ==
                                (base_reason == H100_BLOCK_NONE &&
                                 !output.o_fault_latched));
                            REQUIRE(output.o_at_speed ==
                                (!input.i_run_request ||
                                 (output.o_state == H100_RUNNING &&
                                  output.o_running)));
                            REQUIRE(output.o_block_code <= H100_BLOCK_INTERNAL_STATE);
                            REQUIRE(output.o_fault_code <= H100_BLOCK_INTERNAL_STATE);
                            if (output.o_main_control != H100_CONTROL_STOP) {
                                REQUIRE(active_state(output.o_state));
                            }
                            if (output.o_state == H100_STOPPED) {
                                REQUIRE(output.o_given_frequency == 0);
                            }
                        }
                    }
                }
            }
        }
    }
}

static void test_minimum_maximum_and_frequency_resolution_boundaries(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_forward_request = 1;
    input.i_speed_command_rpm = 6000.0;
    context.x_state = H100_STOPPED;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_ARMING);
    REQUIRE(output.o_given_frequency == 1000);

    context = new_context();
    context.x_state = H100_STOPPED;
    input.i_speed_command_rpm = 24000.0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_ARMING);
    REQUIRE(output.o_given_frequency == 4000);

    context = new_context();
    context.x_state = H100_STOPPED;
    input.i_frequency_decimals_f169 = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_ARMING);
    REQUIRE(output.o_given_frequency == 20000);
}

static void test_feedback_scaling_and_tolerance_edges(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    start_forward_running(&context, &input, &config, &output);
    REQUIRE(fabs(output.o_speed_feedback_rpm - 12000.0) < 0.000001);

    input.i_output_frequency_decihz = 1990;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_RUNNING);

    input.i_output_frequency_decihz = 2010;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_RUNNING);

    input.i_output_frequency_decihz = 1989;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STARTING);

    input.i_output_frequency_decihz = 2011;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STARTING);

    context = new_context();
    input = valid_input();
    config = valid_config();
    config.c_rated_rpm = 0.0;
    input.i_output_frequency_decihz = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_speed_feedback_rpm == 0.0);

    config = valid_config();
    config.c_expected_reference_f004_centihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_speed_feedback_rpm == 0.0);
}

static void test_stop_waits_for_both_feedback_conditions(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    start_forward_running(&context, &input, &config, &output);
    input.i_run_request = 0;
    input.i_forward_request = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STOPPING);
    REQUIRE(output.o_given_frequency == 2000);

    input.i_main_status = H100_STATUS_IN_OPERATION;
    input.i_output_frequency_decihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STOPPING);
    REQUIRE(output.o_given_frequency == 2000);

    input.i_main_status = 0;
    input.i_output_frequency_decihz = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STOPPING);
    REQUIRE(output.o_given_frequency == 2000);

    input.i_output_frequency_decihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STOPPED);
    REQUIRE(output.o_given_frequency == 0);
}

static void test_fault_reset_requires_the_complete_stopped_edge(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    context.x_state = H100_FAULT;
    context.x_fault_latched = 1;
    context.x_fault_code = H100_BLOCK_DIRECTION;

    input.i_reset = 1;
    input.i_run_request = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);

    input.i_run_request = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);

    input.i_reset = 0;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_link_fault = 1;
    input.i_reset = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);

    input.i_reset = 0;
    input.i_link_fault = 0;
    input.i_current_fault = 1;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_reset = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);

    input.i_reset = 0;
    input.i_current_fault = 0;
    input.i_output_frequency_decihz = 1;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_reset = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);

    input.i_reset = 0;
    input.i_output_frequency_decihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_reset = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(!output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_NONE);
    REQUIRE(output.o_state == H100_STOPPED);
}

static void test_machine_off_and_direction_change_paths(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    start_forward_running(&context, &input, &config, &output);
    input.i_machine_enabled = 0;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_state == H100_STOPPING);
    REQUIRE(output.o_main_control == H100_CONTROL_STOP);

    context = new_context();
    input = valid_input();
    start_forward_running(&context, &input, &config, &output);
    input.i_forward_request = 0;
    input.i_reverse_request = 1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_DIRECTION_CHANGE);
    REQUIRE(output.o_main_control == H100_CONTROL_STOP);
}

void test_invariant_suite(void)
{
    test_output_invariants_for_all_boolean_combinations();
    test_minimum_maximum_and_frequency_resolution_boundaries();
    test_feedback_scaling_and_tolerance_edges();
    test_stop_waits_for_both_feedback_conditions();
    test_fault_reset_requires_the_complete_stopped_edge();
    test_machine_off_and_direction_change_paths();
}
