#include "h100_spindle_logic.h"

uint32_t h100_configuration_block(
    const struct h100_spindle_input *input,
    const struct h100_spindle_config *config)
{
    if (input->i_link_fault) return H100_BLOCK_LINK;
    if (input->i_any_command_disabled) return H100_BLOCK_COMMAND_DISABLED;
    if (input->i_control_mode_f001 != 2) return H100_BLOCK_F001;
    if (input->i_frequency_source_f002 != 2) return H100_BLOCK_F002;
    if (input->i_panel_stop_f024 != 1) return H100_BLOCK_F024;
    if (input->i_slave_address_f163 != 1) return H100_BLOCK_F163;
    if (input->i_baud_selector_f164 != 2) return H100_BLOCK_F164;
    if (input->i_data_mode_f165 != 3) return H100_BLOCK_F165;
    if (input->i_frequency_decimals_f169 > 1) return H100_BLOCK_F169;
    if (config->c_expected_reference_f004_centihz == 0 ||
        config->c_expected_maximum_f005_centihz == 0) {
        return H100_BLOCK_EXPLICIT_FREQUENCY;
    }
    if (input->i_reference_f004_centihz !=
        config->c_expected_reference_f004_centihz) {
        return H100_BLOCK_F004;
    }
    if (input->i_maximum_f005_centihz !=
        config->c_expected_maximum_f005_centihz) {
        return H100_BLOCK_F005;
    }
    if (!isfinite(config->c_rated_rpm) || !isfinite(config->c_minimum_rpm) ||
        !isfinite(config->c_maximum_rpm) ||
        !isfinite(config->c_at_speed_tolerance_hz) ||
        config->c_rated_rpm <= 0.0 || config->c_minimum_rpm <= 0.0 ||
        config->c_maximum_rpm < config->c_minimum_rpm ||
        config->c_maximum_rpm > config->c_rated_rpm ||
        config->c_at_speed_tolerance_hz < 0.0) {
        return H100_BLOCK_RPM_LIMITS;
    }
    if (input->i_current_fault != 0) return H100_BLOCK_VFD_FAULT;
    return H100_BLOCK_NONE;
}

void h100_spindle_step(
    struct h100_spindle_context *context,
    const struct h100_spindle_input *input,
    const struct h100_spindle_config *config,
    struct h100_spindle_output *output)
{
    uint32_t base_reason;
    uint32_t run_reason;
    uint32_t requested_raw = 0;
    double requested_hz = 0.0;
    double units_per_hz;
    double raw_float;
    double actual_hz;
    double frequency_error;
    int is_running;
    int stopped_feedback;
    int reset_rising;
    int requested_reverse;

    is_running = (input->i_main_status & H100_STATUS_IN_OPERATION) != 0;
    stopped_feedback = !is_running && input->i_output_frequency_decihz == 0;
    actual_hz = ((double)input->i_output_frequency_decihz) / 10.0;

    output->o_main_control = H100_CONTROL_STOP;
    output->o_given_frequency = context->x_held_frequency;
    output->o_running = is_running;
    output->o_forward_running = 0;
    output->o_reverse_running = 0;
    output->o_speed_feedback_rpm = 0.0;
    if (config->c_rated_rpm > 0.0 &&
        config->c_expected_reference_f004_centihz > 0) {
        output->o_speed_feedback_rpm =
            actual_hz * config->c_rated_rpm * H100_PARAMETER_UNITS_PER_HZ /
            (double)config->c_expected_reference_f004_centihz;
    }

    base_reason = h100_configuration_block(input, config);
    run_reason = base_reason;
    requested_reverse = input->i_reverse_request && !input->i_forward_request;

    reset_rising = input->i_reset && !context->x_previous_reset;
    context->x_previous_reset = input->i_reset;
    if (reset_rising && !input->i_run_request && stopped_feedback &&
        !input->i_link_fault && input->i_current_fault == 0) {
        context->x_fault_latched = 0;
        context->x_fault_code = H100_BLOCK_NONE;
        context->x_state = H100_STOPPING;
    }

    if (context->x_state < H100_STOPPED || context->x_state > H100_FAULT ||
        (!!context->x_fault_latched !=
         (context->x_fault_code != H100_BLOCK_NONE))) {
        context->x_fault_latched = 1;
        if (context->x_fault_code == H100_BLOCK_NONE ||
            (context->x_state >= H100_STOPPED &&
             context->x_state <= H100_FAULT)) {
            context->x_fault_code = H100_BLOCK_INTERNAL_STATE;
        }
        context->x_state = H100_FAULT;
    }

    if (context->x_fault_latched &&
        context->x_state != H100_STOPPING && context->x_state != H100_FAULT) {
        context->x_state = H100_STOPPING;
    }

    if (input->i_run_request && input->i_machine_enabled &&
        !context->x_fault_latched) {
        if (run_reason == H100_BLOCK_NONE &&
            input->i_forward_request == input->i_reverse_request) {
            run_reason = H100_BLOCK_DIRECTION;
        }
        if (run_reason == H100_BLOCK_NONE &&
            (context->x_state == H100_ARMING ||
             context->x_state == H100_STARTING ||
             context->x_state == H100_RUNNING) &&
            requested_reverse != context->x_held_reverse) {
            run_reason = H100_BLOCK_DIRECTION_CHANGE;
        }
        if (run_reason == H100_BLOCK_NONE &&
            !isfinite(input->i_speed_command_rpm)) {
            run_reason = H100_BLOCK_SPEED_INVALID;
        }
        if (run_reason == H100_BLOCK_NONE &&
            input->i_speed_command_rpm <= 0.0) {
            run_reason = H100_BLOCK_SPEED_ZERO;
        }
        if (run_reason == H100_BLOCK_NONE &&
            input->i_speed_command_rpm < config->c_minimum_rpm) {
            run_reason = H100_BLOCK_SPEED_LOW;
        }
        if (run_reason == H100_BLOCK_NONE &&
            input->i_speed_command_rpm > config->c_maximum_rpm) {
            run_reason = H100_BLOCK_SPEED_HIGH;
        }

        if (run_reason == H100_BLOCK_NONE) {
            requested_hz = input->i_speed_command_rpm *
                ((double)config->c_expected_reference_f004_centihz /
                    H100_PARAMETER_UNITS_PER_HZ) /
                config->c_rated_rpm;
            if (requested_hz * H100_PARAMETER_UNITS_PER_HZ <
                (double)input->i_lower_limit_f011_centihz) {
                run_reason = H100_BLOCK_BELOW_F011;
            }
        }

        if (run_reason == H100_BLOCK_NONE) {
            units_per_hz =
                input->i_frequency_decimals_f169 == 0 ? 10.0 : 100.0;
            raw_float = requested_hz * units_per_hz;
            if (raw_float <= 0.0 || raw_float > 65535.0 ||
                requested_hz * H100_PARAMETER_UNITS_PER_HZ >
                    (double)config->c_expected_maximum_f005_centihz) {
                run_reason = H100_BLOCK_FREQUENCY_RANGE;
            } else {
                requested_raw = (uint32_t)(raw_float + 0.5);
                if (requested_raw == 0) {
                    run_reason = H100_BLOCK_FREQUENCY_RANGE;
                }
            }
        }

        if (run_reason != H100_BLOCK_NONE) {
            context->x_fault_latched = 1;
            context->x_fault_code = run_reason;
            context->x_state = H100_STOPPING;
        }
    }

    if ((context->x_state == H100_ARMING ||
         context->x_state == H100_STARTING ||
         context->x_state == H100_RUNNING) &&
        base_reason != H100_BLOCK_NONE) {
        context->x_fault_latched = 1;
        context->x_fault_code = base_reason;
        context->x_state = H100_STOPPING;
    }

    if (!input->i_run_request || !input->i_machine_enabled) {
        if (context->x_state == H100_ARMING ||
            context->x_state == H100_STARTING ||
            context->x_state == H100_RUNNING) {
            context->x_state = H100_STOPPING;
        }
    }

    switch (context->x_state) {
    case H100_STOPPED:
        output->o_main_control = H100_CONTROL_STOP;
        output->o_given_frequency = 0;
        context->x_held_frequency = 0;
        context->x_held_target_hz = 0.0;
        context->x_held_reverse = 0;
        if (input->i_run_request && input->i_machine_enabled) {
            context->x_held_frequency = requested_raw;
            context->x_held_target_hz = requested_hz;
            context->x_held_reverse = requested_reverse;
            output->o_given_frequency = context->x_held_frequency;
            context->x_state = H100_ARMING;
        }
        break;

    case H100_ARMING:
        output->o_main_control = H100_CONTROL_STOP;
        context->x_held_frequency = requested_raw;
        context->x_held_target_hz = requested_hz;
        output->o_given_frequency = context->x_held_frequency;
        if (input->i_given_frequency_readback ==
                context->x_held_frequency) {
            output->o_main_control = context->x_held_reverse ?
                H100_CONTROL_FORWARD : H100_CONTROL_REVERSE;
            context->x_state = H100_STARTING;
        }
        break;

    case H100_STARTING:
    case H100_RUNNING:
        output->o_main_control = context->x_held_reverse ?
            H100_CONTROL_FORWARD : H100_CONTROL_REVERSE;
        context->x_held_frequency = requested_raw;
        context->x_held_target_hz = requested_hz;
        output->o_given_frequency = context->x_held_frequency;
        frequency_error = actual_hz - context->x_held_target_hz;
        if (frequency_error < 0.0) frequency_error = -frequency_error;
        if (is_running &&
            frequency_error <= config->c_at_speed_tolerance_hz) {
            context->x_state = H100_RUNNING;
        } else {
            context->x_state = H100_STARTING;
        }
        break;

    case H100_STOPPING:
        output->o_main_control = H100_CONTROL_STOP;
        output->o_given_frequency = context->x_held_frequency;
        if (stopped_feedback) {
            output->o_given_frequency = 0;
            context->x_held_frequency = 0;
            context->x_held_target_hz = 0.0;
            context->x_held_reverse = 0;
            context->x_state =
                context->x_fault_latched ? H100_FAULT : H100_STOPPED;
        }
        break;

    case H100_FAULT:
    default:
        output->o_main_control = H100_CONTROL_STOP;
        if (stopped_feedback) {
            output->o_given_frequency = 0;
            context->x_held_frequency = 0;
            context->x_held_target_hz = 0.0;
            context->x_held_reverse = 0;
        } else {
            output->o_given_frequency = context->x_held_frequency;
        }
        context->x_state = H100_FAULT;
        break;
    }

    /* The installed H100's 0210H readback proves in-operation but does not
       publish the manual's direction bit. Direction is nevertheless
       unambiguous because this sequencer owns 0200H, latches it before RUN,
       and refuses an in-operation direction change. */
    output->o_forward_running = is_running &&
        (context->x_state == H100_STARTING ||
         context->x_state == H100_RUNNING) &&
        !context->x_held_reverse;
    output->o_reverse_running = is_running &&
        (context->x_state == H100_STARTING ||
         context->x_state == H100_RUNNING) &&
        context->x_held_reverse;

    output->o_ready =
        base_reason == H100_BLOCK_NONE && !context->x_fault_latched;
    output->o_at_speed =
        !input->i_run_request || context->x_state == H100_RUNNING;
    output->o_fault_latched = context->x_fault_latched;
    output->o_fault_code = context->x_fault_code;
    output->o_block_code = input->i_run_request ? run_reason : base_reason;
    output->o_state = (uint32_t)context->x_state;
    output->o_target_frequency_hz = context->x_held_target_hz;
}
