#ifndef H100_SPINDLE_LOGIC_H
#define H100_SPINDLE_LOGIC_H

#include <math.h>
#include <stdint.h>

#define H100_CONTROL_FORWARD 0x0001u /* Physically CCW, verified top-down. */
#define H100_CONTROL_REVERSE 0x0004u /* Physically CW, verified top-down. */
#define H100_CONTROL_STOP    0x0008u

#define H100_STATUS_IN_OPERATION 0x0008u

/*
 * The H100 parameter-register values for F004/F005/F011 are transferred in
 * 0.1 Hz units.  This is independently established by the live F005 value:
 * raw 500 is the documented 50.0 Hz factory value, while 5.0 Hz would be
 * outside F005's documented 10.0 Hz minimum.  F169 controls the separate
 * 0201H command-register resolution and is handled below.
 */
#define H100_PARAMETER_UNITS_PER_HZ 10.0

enum h100_spindle_state {
    H100_STOPPED = 0,
    H100_ARMING = 1,
    H100_STARTING = 2,
    H100_RUNNING = 3,
    H100_STOPPING = 4,
    H100_FAULT = 5
};

enum h100_spindle_block_code {
    H100_BLOCK_NONE = 0,
    H100_BLOCK_LINK = 1,
    H100_BLOCK_F001 = 2,
    H100_BLOCK_F002 = 3,
    H100_BLOCK_F024 = 4,
    H100_BLOCK_F163 = 5,
    H100_BLOCK_F164 = 6,
    H100_BLOCK_F165 = 7,
    H100_BLOCK_F169 = 8,
    H100_BLOCK_EXPLICIT_FREQUENCY = 9,
    H100_BLOCK_F004 = 10,
    H100_BLOCK_F005 = 11,
    H100_BLOCK_RPM_LIMITS = 12,
    H100_BLOCK_VFD_FAULT = 13,
    H100_BLOCK_DIRECTION_CHANGE = 14,
    H100_BLOCK_DIRECTION = 15,
    H100_BLOCK_SPEED_ZERO = 16,
    H100_BLOCK_SPEED_LOW = 17,
    H100_BLOCK_SPEED_HIGH = 18,
    H100_BLOCK_BELOW_F011 = 19,
    H100_BLOCK_FREQUENCY_RANGE = 20,
    H100_BLOCK_SPEED_INVALID = 21,
    H100_BLOCK_COMMAND_DISABLED = 22,
    H100_BLOCK_INTERNAL_STATE = 23
};

struct h100_spindle_input {
    int i_machine_enabled;
    int i_run_request;
    int i_forward_request;
    int i_reverse_request;
    double i_speed_command_rpm;
    int i_reset;
    int i_link_fault;
    int i_any_command_disabled;
    uint32_t i_control_mode_f001;
    uint32_t i_frequency_source_f002;
    uint32_t i_reference_f004_centihz;
    uint32_t i_maximum_f005_centihz;
    uint32_t i_lower_limit_f011_centihz;
    uint32_t i_panel_stop_f024;
    uint32_t i_slave_address_f163;
    uint32_t i_baud_selector_f164;
    uint32_t i_data_mode_f165;
    uint32_t i_frequency_decimals_f169;
    uint32_t i_output_frequency_decihz;
    uint32_t i_current_fault;
    uint32_t i_main_status;
    uint32_t i_given_frequency_readback;
};

struct h100_spindle_config {
    double c_rated_rpm;
    double c_minimum_rpm;
    double c_maximum_rpm;
    uint32_t c_expected_reference_f004_centihz;
    uint32_t c_expected_maximum_f005_centihz;
    double c_at_speed_tolerance_hz;
};

struct h100_spindle_context {
    int x_state;
    int x_previous_reset;
    int x_fault_latched;
    uint32_t x_fault_code;
    uint32_t x_held_frequency;
    double x_held_target_hz;
    int x_held_reverse;
};

struct h100_spindle_output {
    uint32_t o_main_control;
    uint32_t o_given_frequency;
    int o_ready;
    int o_running;
    int o_forward_running;
    int o_reverse_running;
    int o_at_speed;
    int o_fault_latched;
    uint32_t o_fault_code;
    uint32_t o_block_code;
    uint32_t o_state;
    double o_speed_feedback_rpm;
    double o_target_frequency_hz;
};

uint32_t h100_configuration_block(
    const struct h100_spindle_input *input,
    const struct h100_spindle_config *config);

void h100_spindle_step(
    struct h100_spindle_context *context,
    const struct h100_spindle_input *input,
    const struct h100_spindle_config *config,
    struct h100_spindle_output *output);

#endif
