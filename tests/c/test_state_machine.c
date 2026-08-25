#include "test_support.h"

#include <assert.h>
#include <math.h>

static void test_full_start_speed_change_and_stop(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_ready);
    assert(output.o_state == H100_STOPPED);
    assert(output.o_at_speed);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 0);

    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_forward_request = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_ARMING);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 2000);
    assert(fabs(output.o_target_frequency_hz - 200.0) < 0.0001);
    assert(!output.o_at_speed);

    input.i_given_frequency_readback = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STARTING);
    assert(output.o_main_control == H100_CONTROL_REVERSE);
    assert(output.o_given_frequency == 2000);
    assert(!output.o_at_speed);

    /* Live 0210H reports in-operation without a direction bit. */
    input.i_main_status = 0x0009;
    input.i_output_frequency_decihz = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_RUNNING);
    assert(output.o_at_speed);
    assert(output.o_forward_running);
    assert(!output.o_reverse_running);
    assert(fabs(output.o_speed_feedback_rpm - 12000.0) < 0.0001);

    input.i_speed_command_rpm = 18000.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STARTING);
    assert(output.o_main_control == H100_CONTROL_REVERSE);
    assert(output.o_given_frequency == 3000);
    assert(!output.o_at_speed);

    input.i_given_frequency_readback = 3000;
    input.i_output_frequency_decihz = 3000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_RUNNING);
    assert(output.o_at_speed);

    input.i_run_request = 0;
    input.i_forward_request = 0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STOPPING);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 3000);

    input.i_main_status = 0;
    input.i_output_frequency_decihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STOPPED);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 0);
}

static void test_reverse_start_and_stop(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    h100_spindle_step(&context, &input, &config, &output);
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_reverse_request = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(!output.o_fault_latched);
    assert(output.o_state == H100_ARMING);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 2000);

    input.i_given_frequency_readback = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STARTING);
    assert(output.o_main_control == H100_CONTROL_FORWARD);

    input.i_main_status = 0x0009;
    input.i_output_frequency_decihz = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_RUNNING);
    assert(output.o_at_speed);
    assert(!output.o_forward_running);
    assert(output.o_reverse_running);

    input.i_run_request = 0;
    input.i_reverse_request = 0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STOPPING);
    assert(output.o_main_control == H100_CONTROL_STOP);

    input.i_main_status = 0;
    input.i_output_frequency_decihz = 0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_STOPPED);
}

static void test_link_loss_during_run_stops_and_latches(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    h100_spindle_step(&context, &input, &config, &output);
    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_forward_request = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_given_frequency_readback = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    input.i_main_status = 0x0009;
    input.i_output_frequency_decihz = 2000;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_state == H100_RUNNING);

    input.i_link_fault = 1;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_fault_latched);
    assert(output.o_fault_code == H100_BLOCK_LINK);
    assert(output.o_main_control == H100_CONTROL_STOP);
    assert(output.o_given_frequency == 2000);
}

static void test_unresolved_limits_refuse_without_idle_fault(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    config.c_minimum_rpm = 0.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(!output.o_ready);
    assert(output.o_block_code == H100_BLOCK_RPM_LIMITS);
    assert(!output.o_fault_latched);

    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_forward_request = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_fault_latched);
    assert(output.o_fault_code == H100_BLOCK_RPM_LIMITS);
    assert(output.o_main_control == H100_CONTROL_STOP);
}

static void test_disabled_modbus_command_cannot_leave_stale_ready_state(void)
{
    struct h100_spindle_input input = valid_input();
    struct h100_spindle_config config = valid_config();
    struct h100_spindle_context context = new_context();
    struct h100_spindle_output output;

    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_ready);

    input.i_any_command_disabled = 1;
    h100_spindle_step(&context, &input, &config, &output);
    assert(!output.o_ready);
    assert(output.o_block_code == H100_BLOCK_COMMAND_DISABLED);
    assert(!output.o_fault_latched);

    input.i_machine_enabled = 1;
    input.i_run_request = 1;
    input.i_forward_request = 1;
    input.i_speed_command_rpm = 12000.0;
    h100_spindle_step(&context, &input, &config, &output);
    assert(output.o_fault_latched);
    assert(output.o_fault_code == H100_BLOCK_COMMAND_DISABLED);
    assert(output.o_main_control == H100_CONTROL_STOP);
}

void test_state_machine_suite(void)
{
    test_full_start_speed_change_and_stop();
    test_reverse_start_and_stop();
    test_link_loss_during_run_stops_and_latches();
    test_unresolved_limits_refuse_without_idle_fault();
    test_disabled_modbus_command_cannot_leave_stale_ready_state();
}
