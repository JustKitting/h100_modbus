use core::ffi::{c_long, c_void};
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::sequencer::{
    main_status_reserved_mask, BlockCode, BlockRecord, Input, MainStatusBit, State, VfdFaultCode,
    VfdFaultFamily, VfdFaultPhase,
};

use super::pins::Component;

unsafe fn read<T: Copy>(pointer: *mut T) -> T {
    unsafe { ptr::read_volatile(pointer) }
}

unsafe fn write<T: Copy>(pointer: *mut T, value: T) {
    unsafe { ptr::write_volatile(pointer, value) }
}

pub(super) unsafe extern "C" fn update_component(argument: *mut c_void, _period: c_long) {
    if argument.is_null() {
        return;
    }
    let component = unsafe { &mut *argument.cast::<Component>() };
    let generation = component.diagnostic_generation.wrapping_add(2);
    component.diagnostic_generation = generation;
    let pins = &component.pins;
    let input = Input {
        machine_enabled: unsafe { read(pins.machine_enabled) },
        run_request: unsafe { read(pins.run_request) },
        forward_request: unsafe { read(pins.forward_request) },
        reverse_request: unsafe { read(pins.reverse_request) },
        speed_command_rpm: unsafe { read(pins.speed_command_rpm) },
        reset: unsafe { read(pins.reset) },
        link_fault: unsafe { read(pins.link_fault) },
        any_command_disabled: pins
            .command_disabled
            .iter()
            .any(|pointer| unsafe { read(*pointer) }),
        control_mode_f001: unsafe { read(pins.control_mode_f001) },
        frequency_source_f002: unsafe { read(pins.frequency_source_f002) },
        reference_f004_centihz: unsafe { read(pins.reference_f004_centihz) },
        maximum_f005_centihz: unsafe { read(pins.maximum_f005_centihz) },
        lower_limit_f011_centihz: unsafe { read(pins.lower_limit_f011_centihz) },
        panel_stop_f024: unsafe { read(pins.panel_stop_f024) },
        slave_address_f163: unsafe { read(pins.slave_address_f163) },
        baud_selector_f164: unsafe { read(pins.baud_selector_f164) },
        data_mode_f165: unsafe { read(pins.data_mode_f165) },
        frequency_decimals_f169: unsafe { read(pins.frequency_decimals_f169) },
        output_frequency_decihz: unsafe { read(pins.output_frequency_decihz) },
        current_fault: unsafe { read(pins.current_fault) },
        main_status: unsafe { read(pins.main_status) },
        given_frequency_readback: unsafe { read(pins.given_frequency_readback) },
    };
    let config = unsafe { ptr::read_volatile(ptr::addr_of!(component.config)) };
    let output = component.context.step(input, config);
    let generation_pin = unsafe { &*(pins.diagnostic_snapshot_generation.cast::<AtomicU32>()) };
    unsafe {
        generation_pin.store(generation | 1, Ordering::SeqCst);
        write(pins.main_control, output.main_control);
        write(pins.given_frequency, output.given_frequency);
        write(pins.observed_current_fault, input.current_fault);
        write(pins.observed_main_status, input.main_status);
        write(pins.ready, output.ready);
        write(pins.running, output.running);
        write(pins.forward_running, output.forward_running);
        write(pins.reverse_running, output.reverse_running);
        write(pins.at_speed, output.at_speed);
        write(pins.fault_latched, output.fault_latched);
        write(pins.fault_code, output.fault_code);
        write(pins.block_code, output.block_code);
        write(pins.state, output.state);
        let fault_code =
            BlockCode::from_wire_code(output.fault_code).filter(|code| *code != BlockCode::None);
        for (index, known) in BlockCode::DIAGNOSTICS.iter().copied().enumerate() {
            write(pins.fault_kind[index], fault_code == Some(known));
        }
        write(
            pins.fault_kind_unknown,
            output.fault_code != 0 && fault_code.is_none(),
        );
        let block_code = BlockCode::from_wire_code(output.block_code);
        for (index, known) in BlockCode::ALL.iter().copied().enumerate() {
            write(pins.block_kind[index], block_code == Some(known));
        }
        write(pins.block_kind_unknown, block_code.is_none());
        let state = i32::try_from(output.state).ok().and_then(State::from_raw);
        for (index, known) in State::ALL.iter().copied().enumerate() {
            write(pins.state_kind[index], state == Some(known));
        }
        write(pins.state_kind_unknown, state.is_none());
        publish_main_status(pins, input.main_status);
        publish_vfd_fault(pins, input.current_fault);
        publish_fault_evidence(pins, output.fault_record.or(output.block_record));
        write(pins.speed_feedback_rpm, output.speed_feedback_rpm);
        write(pins.target_frequency_hz, output.target_frequency_hz);
        generation_pin.store(generation, Ordering::SeqCst);
    }
}

unsafe fn publish_main_status(pins: &super::pins::Pins, raw: u32) {
    let reserved = main_status_reserved_mask(raw);
    unsafe {
        write(pins.main_status_known, reserved == 0);
        write(pins.main_status_unknown, reserved != 0);
        write(pins.main_status_reserved_mask, reserved);
        for (index, status) in MainStatusBit::ALL.iter().copied().enumerate() {
            write(pins.main_status_bit[index], raw & status.wire_code() != 0);
        }
    }
}

unsafe fn publish_vfd_fault(pins: &super::pins::Pins, raw: u32) {
    let decoded = VfdFaultCode::decode(raw);
    unsafe {
        write(pins.vfd_fault_present, raw != 0);
        write(pins.vfd_fault_known, decoded.is_some());
        write(pins.vfd_fault_unknown, raw != 0 && decoded.is_none());
        for (index, known) in VfdFaultFamily::ALL.iter().copied().enumerate() {
            write(
                pins.vfd_fault_family[index],
                decoded.is_some_and(|value| value.family == known),
            );
        }
        for (index, known) in VfdFaultPhase::ALL.iter().copied().enumerate() {
            write(
                pins.vfd_fault_phase[index],
                decoded.is_some_and(|value| value.phase == known),
            );
        }
    }
}

unsafe fn publish_fault_evidence(pins: &super::pins::Pins, record: Option<BlockRecord>) {
    unsafe {
        write(pins.fault_evidence_valid, record.is_some());
        let Some(record) = record else {
            clear_fault_evidence(pins);
            return;
        };
        let evidence = record.evidence;
        let input = evidence.input;
        let config = evidence.config;
        write(pins.fault_evidence_state_before, evidence.state_before);
        write(
            pins.fault_evidence_context_fault_latched_before,
            evidence.context_fault_latched_before,
        );
        write(
            pins.fault_evidence_context_fault_record_present_before,
            evidence.context_fault_record_present_before,
        );
        write(
            pins.fault_evidence_context_fault_code_before,
            evidence.context_fault_code_before,
        );
        write(
            pins.fault_evidence_context_fault_record_code_before,
            evidence.context_fault_record_code_before,
        );
        write(pins.fault_evidence_machine_enabled, input.machine_enabled);
        write(pins.fault_evidence_run_request, input.run_request);
        write(pins.fault_evidence_forward_request, input.forward_request);
        write(pins.fault_evidence_reverse_request, input.reverse_request);
        write(pins.fault_evidence_reset, input.reset);
        write(pins.fault_evidence_link_fault, input.link_fault);
        write(
            pins.fault_evidence_command_disabled,
            input.any_command_disabled,
        );
        write(
            pins.fault_evidence_speed_command_rpm,
            input.speed_command_rpm,
        );
        write(
            pins.fault_evidence_control_mode_f001,
            input.control_mode_f001,
        );
        write(
            pins.fault_evidence_frequency_source_f002,
            input.frequency_source_f002,
        );
        write(
            pins.fault_evidence_reference_f004_centihz,
            input.reference_f004_centihz,
        );
        write(
            pins.fault_evidence_maximum_f005_centihz,
            input.maximum_f005_centihz,
        );
        write(
            pins.fault_evidence_lower_limit_f011_centihz,
            input.lower_limit_f011_centihz,
        );
        write(pins.fault_evidence_panel_stop_f024, input.panel_stop_f024);
        write(
            pins.fault_evidence_slave_address_f163,
            input.slave_address_f163,
        );
        write(
            pins.fault_evidence_baud_selector_f164,
            input.baud_selector_f164,
        );
        write(pins.fault_evidence_data_mode_f165, input.data_mode_f165);
        write(
            pins.fault_evidence_frequency_decimals_f169,
            input.frequency_decimals_f169,
        );
        write(
            pins.fault_evidence_output_frequency_decihz,
            input.output_frequency_decihz,
        );
        write(pins.fault_evidence_current_vfd_fault, input.current_fault);
        write(pins.fault_evidence_main_status, input.main_status);
        write(
            pins.fault_evidence_given_frequency_readback,
            input.given_frequency_readback,
        );
        write(
            pins.fault_evidence_expected_reference_f004_centihz,
            config.expected_reference_f004_centihz,
        );
        write(
            pins.fault_evidence_expected_maximum_f005_centihz,
            config.expected_maximum_f005_centihz,
        );
        write(
            pins.fault_evidence_calculated_frequency_register,
            evidence.calculated_frequency_register,
        );
        write(pins.fault_evidence_rated_rpm, config.rated_rpm);
        write(pins.fault_evidence_minimum_rpm, config.minimum_rpm);
        write(pins.fault_evidence_maximum_rpm, config.maximum_rpm);
        write(
            pins.fault_evidence_at_speed_tolerance_hz,
            config.at_speed_tolerance_hz,
        );
        write(
            pins.fault_evidence_calculated_frequency_hz,
            evidence.calculated_frequency_hz,
        );
    }
}

unsafe fn clear_fault_evidence(pins: &super::pins::Pins) {
    unsafe {
        for pointer in [
            pins.fault_evidence_machine_enabled,
            pins.fault_evidence_run_request,
            pins.fault_evidence_forward_request,
            pins.fault_evidence_reverse_request,
            pins.fault_evidence_reset,
            pins.fault_evidence_link_fault,
            pins.fault_evidence_command_disabled,
            pins.fault_evidence_context_fault_latched_before,
            pins.fault_evidence_context_fault_record_present_before,
        ] {
            write(pointer, false);
        }
        write(pins.fault_evidence_state_before, 0);
        for pointer in [
            pins.fault_evidence_control_mode_f001,
            pins.fault_evidence_context_fault_code_before,
            pins.fault_evidence_context_fault_record_code_before,
            pins.fault_evidence_frequency_source_f002,
            pins.fault_evidence_reference_f004_centihz,
            pins.fault_evidence_maximum_f005_centihz,
            pins.fault_evidence_lower_limit_f011_centihz,
            pins.fault_evidence_panel_stop_f024,
            pins.fault_evidence_slave_address_f163,
            pins.fault_evidence_baud_selector_f164,
            pins.fault_evidence_data_mode_f165,
            pins.fault_evidence_frequency_decimals_f169,
            pins.fault_evidence_output_frequency_decihz,
            pins.fault_evidence_current_vfd_fault,
            pins.fault_evidence_main_status,
            pins.fault_evidence_given_frequency_readback,
            pins.fault_evidence_expected_reference_f004_centihz,
            pins.fault_evidence_expected_maximum_f005_centihz,
            pins.fault_evidence_calculated_frequency_register,
        ] {
            write(pointer, 0);
        }
        for pointer in [
            pins.fault_evidence_speed_command_rpm,
            pins.fault_evidence_rated_rpm,
            pins.fault_evidence_minimum_rpm,
            pins.fault_evidence_maximum_rpm,
            pins.fault_evidence_at_speed_tolerance_hz,
            pins.fault_evidence_calculated_frequency_hz,
        ] {
            write(pointer, 0.0);
        }
    }
}
