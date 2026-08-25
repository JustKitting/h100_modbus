#include "test_support.h"

#include <float.h>
#include <math.h>

static void require_configuration_code(
    struct h100_spindle_input input,
    struct h100_spindle_config config,
    uint32_t expected)
{
    REQUIRE(h100_configuration_block(&input, &config) == expected);
}

static void test_every_configuration_block_code(void)
{
    struct h100_spindle_input input;
    struct h100_spindle_config config;

    require_configuration_code(valid_input(), valid_config(), H100_BLOCK_NONE);

    input = valid_input();
    input.i_link_fault = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_LINK);

    input = valid_input();
    input.i_any_command_disabled = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_COMMAND_DISABLED);

    input = valid_input();
    input.i_control_mode_f001 = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_F001);

    input = valid_input();
    input.i_frequency_source_f002 = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_F002);

    input = valid_input();
    input.i_panel_stop_f024 = 0;
    require_configuration_code(input, valid_config(), H100_BLOCK_F024);

    input = valid_input();
    input.i_slave_address_f163 = 2;
    require_configuration_code(input, valid_config(), H100_BLOCK_F163);

    input = valid_input();
    input.i_baud_selector_f164 = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_F164);

    input = valid_input();
    input.i_data_mode_f165 = 2;
    require_configuration_code(input, valid_config(), H100_BLOCK_F165);

    input = valid_input();
    input.i_frequency_decimals_f169 = 2;
    require_configuration_code(input, valid_config(), H100_BLOCK_F169);

    config = valid_config();
    config.c_expected_reference_f004_centihz = 0;
    require_configuration_code(valid_input(), config, H100_BLOCK_EXPLICIT_FREQUENCY);

    config = valid_config();
    config.c_expected_maximum_f005_centihz = 0;
    require_configuration_code(valid_input(), config, H100_BLOCK_EXPLICIT_FREQUENCY);

    input = valid_input();
    input.i_reference_f004_centihz = 3999;
    require_configuration_code(input, valid_config(), H100_BLOCK_F004);

    input = valid_input();
    input.i_maximum_f005_centihz = 3999;
    require_configuration_code(input, valid_config(), H100_BLOCK_F005);

    config = valid_config();
    config.c_rated_rpm = NAN;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_minimum_rpm = NAN;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_maximum_rpm = NAN;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_at_speed_tolerance_hz = NAN;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_rated_rpm = 0.0;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_minimum_rpm = 0.0;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_maximum_rpm = 5999.0;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_maximum_rpm = 24001.0;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    config = valid_config();
    config.c_at_speed_tolerance_hz = -0.001;
    require_configuration_code(valid_input(), config, H100_BLOCK_RPM_LIMITS);

    input = valid_input();
    input.i_current_fault = 1;
    require_configuration_code(input, valid_config(), H100_BLOCK_VFD_FAULT);
}

static struct h100_spindle_output request_run(
    struct h100_spindle_input *input,
    struct h100_spindle_config *config,
    struct h100_spindle_context *context)
{
    struct h100_spindle_output output;
    input->i_machine_enabled = 1;
    input->i_run_request = 1;
    if (input->i_speed_command_rpm == 0.0) {
        input->i_speed_command_rpm = 12000.0;
    }
    h100_spindle_step(context, input, config, &output);
    return output;
}

static void require_run_fault(
    struct h100_spindle_input input,
    struct h100_spindle_config config,
    struct h100_spindle_context context,
    uint32_t expected)
{
    struct h100_spindle_output output = request_run(&input, &config, &context);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == expected);
    REQUIRE(output.o_block_code == expected);
    REQUIRE(output.o_main_control == H100_CONTROL_STOP);
    REQUIRE(output.o_state == H100_FAULT);
}

static void test_every_run_refusal_code(void)
{
    struct h100_spindle_input input;
    struct h100_spindle_config config;
    struct h100_spindle_context context;

    input = valid_input();
    input.i_forward_request = 0;
    input.i_reverse_request = 0;
    require_run_fault(input, valid_config(), new_context(), H100_BLOCK_DIRECTION);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_reverse_request = 1;
    require_run_fault(input, valid_config(), new_context(), H100_BLOCK_DIRECTION);

    input = valid_input();
    input.i_reverse_request = 1;
    context = new_context();
    context.x_state = H100_ARMING;
    context.x_held_frequency = 2000;
    context.x_held_target_hz = 200.0;
    require_run_fault(input, valid_config(), context, H100_BLOCK_DIRECTION_CHANGE);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_speed_command_rpm = NAN;
    config = valid_config();
    context = new_context();
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_INVALID);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_speed_command_rpm = INFINITY;
    config = valid_config();
    context = new_context();
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_INVALID);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_speed_command_rpm = -INFINITY;
    config = valid_config();
    context = new_context();
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_INVALID);

    input = valid_input();
    input.i_forward_request = 1;
    config = valid_config();
    context = new_context();
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_speed_command_rpm = 0.0;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_ZERO);

    context = new_context();
    input.i_speed_command_rpm = -1.0;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_ZERO);

    context = new_context();
    input.i_speed_command_rpm = 5999.0;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_LOW);

    context = new_context();
    input.i_speed_command_rpm = 24001.0;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_SPEED_HIGH);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_lower_limit_f011_centihz = 2001;
    require_run_fault(input, valid_config(), new_context(), H100_BLOCK_BELOW_F011);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_frequency_decimals_f169 = 1;
    input.i_reference_f004_centihz = 10000;
    input.i_maximum_f005_centihz = 10000;
    input.i_speed_command_rpm = 24000.0;
    config = valid_config();
    config.c_expected_reference_f004_centihz = 10000;
    config.c_expected_maximum_f005_centihz = 10000;
    config.c_minimum_rpm = 24000.0;
    require_run_fault(input, config, new_context(), H100_BLOCK_FREQUENCY_RANGE);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_reference_f004_centihz = 5000;
    input.i_speed_command_rpm = 24000.0;
    config = valid_config();
    config.c_expected_reference_f004_centihz = 5000;
    config.c_minimum_rpm = 24000.0;
    require_run_fault(input, config, new_context(), H100_BLOCK_FREQUENCY_RANGE);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_reference_f004_centihz = 1;
    input.i_maximum_f005_centihz = 1;
    config = valid_config();
    config.c_expected_reference_f004_centihz = 1;
    config.c_expected_maximum_f005_centihz = 1;
    config.c_minimum_rpm = DBL_TRUE_MIN;
    config.c_maximum_rpm = DBL_TRUE_MIN;
    config.c_rated_rpm = DBL_MAX;
    context = new_context();
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_speed_command_rpm = DBL_TRUE_MIN;
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_FREQUENCY_RANGE);

    input = valid_input();
    input.i_forward_request = 1;
    input.i_reference_f004_centihz = 1;
    input.i_maximum_f005_centihz = 1;
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_speed_command_rpm = 0.1;
    config = valid_config();
    config.c_expected_reference_f004_centihz = 1;
    config.c_expected_maximum_f005_centihz = 1;
    config.c_minimum_rpm = 0.1;
    context = new_context();
    h100_spindle_step(&context, &input, &config, &(struct h100_spindle_output){0});
    REQUIRE(context.x_fault_code == H100_BLOCK_FREQUENCY_RANGE);
}

static void test_configuration_faults_latch_during_each_active_state(void)
{
    int state;
    for (state = H100_ARMING; state <= H100_RUNNING; ++state) {
        struct h100_spindle_input input = valid_input();
        struct h100_spindle_config config = valid_config();
        struct h100_spindle_context context = new_context();
        struct h100_spindle_output output;
        context.x_state = state;
        context.x_held_frequency = 2000;
        context.x_held_target_hz = 200.0;
        input.i_machine_enabled = 1;
        input.i_run_request = 1;
        input.i_forward_request = 1;
        input.i_speed_command_rpm = 12000.0;
        input.i_link_fault = 1;
        h100_spindle_step(&context, &input, &config, &output);
        REQUIRE(output.o_fault_latched);
        REQUIRE(output.o_fault_code == H100_BLOCK_LINK);
        REQUIRE(output.o_main_control == H100_CONTROL_STOP);
    }
}

static void test_internal_state_and_first_fault_preservation(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    context.x_state = -1;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_INTERNAL_STATE);
    REQUIRE(output.o_state == H100_FAULT);

    context = new_context();
    context.x_state = H100_FAULT + 1;
    context.x_fault_latched = 1;
    context.x_fault_code = H100_BLOCK_LINK;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_LINK);
    REQUIRE(output.o_state == H100_FAULT);

    context = new_context();
    context.x_state = -1;
    context.x_fault_latched = 1;
    context.x_fault_code = H100_BLOCK_LINK;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_LINK);
    REQUIRE(output.o_state == H100_FAULT);

    context = new_context();
    context.x_state = H100_STOPPED;
    context.x_fault_latched = 1;
    context.x_fault_code = H100_BLOCK_NONE;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_INTERNAL_STATE);
    REQUIRE(output.o_state == H100_FAULT);

    context = new_context();
    context.x_state = H100_STOPPED;
    context.x_fault_latched = 0;
    context.x_fault_code = H100_BLOCK_LINK;
    h100_spindle_step(&context, &input, &config, &output);
    REQUIRE(output.o_fault_latched);
    REQUIRE(output.o_fault_code == H100_BLOCK_INTERNAL_STATE);
    REQUIRE(output.o_state == H100_FAULT);
}

void test_block_code_suite(void)
{
    test_every_configuration_block_code();
    test_every_run_refusal_code();
    test_configuration_faults_latch_during_each_active_state();
    test_internal_state_and_first_fault_preservation();
}
