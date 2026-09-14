use core::ffi::{c_char, c_int};
use core::ptr;

use dmc2_hal_sys as hal;

use crate::sequencer::{
    BlockCode, MainStatusBit, State, VfdFaultFamily, VfdFaultPhase, CONTROL_STOP,
};

use super::pins::{Component, Pins};

unsafe fn bit_pin(
    pointer: *mut *mut hal::hal_bit_t,
    name: &[u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_pin_bit_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    hal::HalCall::PinBitNew.classify(result).map(|_| ())
}

unsafe fn u32_pin(
    pointer: *mut *mut hal::hal_u32_t,
    name: &[u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_pin_u32_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    hal::HalCall::PinU32New.classify(result).map(|_| ())
}

unsafe fn float_pin(
    pointer: *mut *mut hal::real_t,
    name: &[u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_pin_float_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    hal::HalCall::PinFloatNew.classify(result).map(|_| ())
}

unsafe fn s32_pin(
    pointer: *mut *mut hal::hal_s32_t,
    name: &[u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_pin_s32_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    hal::HalCall::PinS32New.classify(result).map(|_| ())
}

unsafe fn numbered_bit_pin(
    pointer: *mut *mut hal::hal_bit_t,
    prefix: &[u8],
    value: u32,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let mut name = [0_u8; 128];
    let mut reversed = [0_u8; 10];
    let mut remaining = value;
    let mut digits = 0;
    loop {
        reversed[digits] = b'0' + (remaining % 10) as u8;
        digits += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let end = prefix.len() + digits;
    if end > hal::HAL_NAME_LEN as usize {
        return hal::HalCall::PinBitNew
            .classify(hal::HalKnownErrno::InvalidArgument.raw())
            .map(|_| ());
    }
    name[..prefix.len()].copy_from_slice(prefix);
    for index in 0..digits {
        name[prefix.len() + index] = reversed[digits - index - 1];
    }
    name[end] = 0;
    unsafe {
        bit_pin(
            pointer,
            &name[..=end],
            hal::hal_pin_dir_t_HAL_OUT,
            component_id,
        )
    }
}

unsafe fn float_parameter(
    pointer: *mut hal::real_t,
    name: &'static [u8],
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_param_float_new(
            name.as_ptr().cast::<c_char>(),
            hal::hal_param_dir_t_HAL_RW,
            pointer,
            component_id,
        )
    };
    hal::HalCall::ParamFloatNew.classify(result).map(|_| ())
}

unsafe fn u32_parameter(
    pointer: *mut hal::hal_u32_t,
    name: &'static [u8],
    component_id: c_int,
) -> Result<(), hal::HalError> {
    let result = unsafe {
        hal::hal_param_u32_new(
            name.as_ptr().cast::<c_char>(),
            hal::hal_param_dir_t_HAL_RW,
            pointer,
            component_id,
        )
    };
    hal::HalCall::ParamU32New.classify(result).map(|_| ())
}

macro_rules! scalar_pins {
    ($function:ident, $pins:ident, $direction:ident, $component_id:ident; $($field:ident => $name:literal),+ $(,)?) => {
        $(const _: () = assert!($name.len() <= hal::HAL_NAME_LEN as usize);
        unsafe {
            $function(
                ptr::addr_of_mut!((*$pins).$field),
                concat!($name, "\0").as_bytes(),
                $direction,
                $component_id,
            )?;
        })+
    };
}

macro_rules! indexed_pins {
    ($function:ident, $pins:ident, $direction:ident, $component_id:ident, $field:ident; $($index:literal => $name:literal),+ $(,)?) => {
        $(const _: () = assert!($name.len() <= hal::HAL_NAME_LEN as usize);
        unsafe {
            $function(
                ptr::addr_of_mut!((*$pins).$field[$index]),
                concat!($name, "\0").as_bytes(),
                $direction,
                $component_id,
            )?;
        })+
    };
}

pub(super) unsafe fn register_interface(
    component: *mut Component,
    component_id: c_int,
) -> Result<(), hal::HalError> {
    const _: () =
        assert!("h100-spindle.expected-reference-f004-centihz".len() <= hal::HAL_NAME_LEN as usize);
    const _: () =
        assert!("h100-spindle.expected-maximum-f005-centihz".len() <= hal::HAL_NAME_LEN as usize);
    const _: () = assert!("h100-spindle.at-speed-tolerance-hz".len() <= hal::HAL_NAME_LEN as usize);
    const _: () = assert!("h100-spindle.rated-rpm".len() <= hal::HAL_NAME_LEN as usize);
    const _: () = assert!("h100-spindle.minimum-rpm".len() <= hal::HAL_NAME_LEN as usize);
    const _: () = assert!("h100-spindle.maximum-rpm".len() <= hal::HAL_NAME_LEN as usize);

    let pins = unsafe { ptr::addr_of_mut!((*component).pins) };
    let input = hal::hal_pin_dir_t_HAL_IN;
    let output = hal::hal_pin_dir_t_HAL_OUT;

    scalar_pins!(bit_pin, pins, input, component_id;
        machine_enabled => "h100-spindle.machine-enabled",
        run_request => "h100-spindle.run-request",
        forward_request => "h100-spindle.forward-request",
        reverse_request => "h100-spindle.reverse-request",
        reset => "h100-spindle.reset",
        link_fault => "h100-spindle.link-fault",
    );
    scalar_pins!(float_pin, pins, input, component_id;
        speed_command_rpm => "h100-spindle.speed-command-rpm",
    );
    indexed_pins!(bit_pin, pins, input, component_id, command_disabled;
        0 => "h100-spindle.command-disabled0",
        1 => "h100-spindle.command-disabled1",
        2 => "h100-spindle.command-disabled2",
        3 => "h100-spindle.command-disabled3",
        4 => "h100-spindle.command-disabled4",
        5 => "h100-spindle.command-disabled5",
        6 => "h100-spindle.command-disabled6",
        7 => "h100-spindle.command-disabled7",
        8 => "h100-spindle.command-disabled8",
        9 => "h100-spindle.command-disabled9",
        10 => "h100-spindle.command-disabled10",
        11 => "h100-spindle.command-disabled11",
        12 => "h100-spindle.command-disabled12",
    );
    scalar_pins!(u32_pin, pins, input, component_id;
        control_mode_f001 => "h100-spindle.control-mode-f001",
        frequency_source_f002 => "h100-spindle.frequency-source-f002",
        reference_f004_centihz => "h100-spindle.reference-f004-centihz",
        maximum_f005_centihz => "h100-spindle.maximum-f005-centihz",
        lower_limit_f011_centihz => "h100-spindle.lower-limit-f011-centihz",
        panel_stop_f024 => "h100-spindle.panel-stop-f024",
        slave_address_f163 => "h100-spindle.slave-address-f163",
        baud_selector_f164 => "h100-spindle.baud-selector-f164",
        data_mode_f165 => "h100-spindle.data-mode-f165",
        frequency_decimals_f169 => "h100-spindle.frequency-decimals-f169",
        output_frequency_decihz => "h100-spindle.output-frequency-decihz",
        current_fault => "h100-spindle.current-fault",
        main_status => "h100-spindle.main-status",
        given_frequency_readback => "h100-spindle.given-frequency-readback",
    );
    scalar_pins!(u32_pin, pins, output, component_id;
        diagnostic_snapshot_generation => "h100-spindle.diagnostic-snapshot-generation",
        main_control => "h100-spindle.main-control",
        given_frequency => "h100-spindle.given-frequency",
        observed_current_fault => "h100-spindle.observed-current-fault",
        observed_main_status => "h100-spindle.observed-main-status",
        fault_code => "h100-spindle.fault-code",
        block_code => "h100-spindle.block-code",
        state => "h100-spindle.state",
    );
    for (index, code) in BlockCode::DIAGNOSTICS.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).fault_kind[index]),
                b"h100-spindle.fault-kind-",
                code.wire_code(),
                component_id,
            )?;
        }
    }
    for (index, code) in BlockCode::ALL.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).block_kind[index]),
                b"h100-spindle.block-kind-",
                code.wire_code(),
                component_id,
            )?;
        }
    }
    for (index, state) in State::ALL.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).state_kind[index]),
                b"h100-spindle.state-kind-",
                state.wire_code() as u32,
                component_id,
            )?;
        }
    }
    for (index, family) in VfdFaultFamily::ALL.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).vfd_fault_family[index]),
                b"h100-spindle.vfd-fault-family-",
                family.base_code(),
                component_id,
            )?;
        }
    }
    for (index, phase) in VfdFaultPhase::ALL.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).vfd_fault_phase[index]),
                b"h100-spindle.vfd-fault-phase-",
                phase.offset(),
                component_id,
            )?;
        }
    }
    for (index, status) in MainStatusBit::ALL.iter().copied().enumerate() {
        unsafe {
            numbered_bit_pin(
                ptr::addr_of_mut!((*pins).main_status_bit[index]),
                b"h100-spindle.main-status-bit-",
                status.wire_code(),
                component_id,
            )?;
        }
    }
    scalar_pins!(bit_pin, pins, output, component_id;
        ready => "h100-spindle.ready",
        running => "h100-spindle.running",
        forward_running => "h100-spindle.forward-running",
        reverse_running => "h100-spindle.reverse-running",
        at_speed => "h100-spindle.at-speed",
        fault_latched => "h100-spindle.fault-latched",
        fault_kind_unknown => "h100-spindle.fault-kind-unknown",
        block_kind_unknown => "h100-spindle.block-kind-unknown",
        state_kind_unknown => "h100-spindle.state-kind-unknown",
        vfd_fault_present => "h100-spindle.vfd-fault-present",
        vfd_fault_known => "h100-spindle.vfd-fault-known",
        vfd_fault_unknown => "h100-spindle.vfd-fault-unknown",
        main_status_known => "h100-spindle.main-status-known",
        main_status_unknown => "h100-spindle.main-status-unknown",
        fault_evidence_valid => "h100-spindle.fault-data-b00",
        fault_evidence_context_fault_latched_before => "h100-spindle.fault-data-b01",
        fault_evidence_context_fault_record_present_before => "h100-spindle.fault-data-b02",
        fault_evidence_machine_enabled => "h100-spindle.fault-data-b03",
        fault_evidence_run_request => "h100-spindle.fault-data-b04",
        fault_evidence_forward_request => "h100-spindle.fault-data-b05",
        fault_evidence_reverse_request => "h100-spindle.fault-data-b06",
        fault_evidence_reset => "h100-spindle.fault-data-b07",
        fault_evidence_link_fault => "h100-spindle.fault-data-b08",
        fault_evidence_command_disabled => "h100-spindle.fault-data-b09",
    );
    scalar_pins!(s32_pin, pins, output, component_id;
        fault_evidence_state_before => "h100-spindle.fault-data-s00",
    );
    scalar_pins!(u32_pin, pins, output, component_id;
        main_status_reserved_mask => "h100-spindle.main-status-reserved-mask",
        fault_evidence_context_fault_code_before => "h100-spindle.fault-data-u00",
        fault_evidence_context_fault_record_code_before => "h100-spindle.fault-data-u01",
        fault_evidence_control_mode_f001 => "h100-spindle.fault-data-u02",
        fault_evidence_frequency_source_f002 => "h100-spindle.fault-data-u03",
        fault_evidence_reference_f004_centihz => "h100-spindle.fault-data-u04",
        fault_evidence_maximum_f005_centihz => "h100-spindle.fault-data-u05",
        fault_evidence_lower_limit_f011_centihz => "h100-spindle.fault-data-u06",
        fault_evidence_panel_stop_f024 => "h100-spindle.fault-data-u07",
        fault_evidence_slave_address_f163 => "h100-spindle.fault-data-u08",
        fault_evidence_baud_selector_f164 => "h100-spindle.fault-data-u09",
        fault_evidence_data_mode_f165 => "h100-spindle.fault-data-u10",
        fault_evidence_frequency_decimals_f169 => "h100-spindle.fault-data-u11",
        fault_evidence_output_frequency_decihz => "h100-spindle.fault-data-u12",
        fault_evidence_current_vfd_fault => "h100-spindle.fault-data-u13",
        fault_evidence_main_status => "h100-spindle.fault-data-u14",
        fault_evidence_given_frequency_readback => "h100-spindle.fault-data-u15",
        fault_evidence_expected_reference_f004_centihz => "h100-spindle.fault-data-u16",
        fault_evidence_expected_maximum_f005_centihz => "h100-spindle.fault-data-u17",
        fault_evidence_calculated_frequency_register => "h100-spindle.fault-data-u18",
    );
    scalar_pins!(float_pin, pins, output, component_id;
        speed_feedback_rpm => "h100-spindle.speed-feedback-rpm",
        target_frequency_hz => "h100-spindle.target-frequency-hz",
        fault_evidence_speed_command_rpm => "h100-spindle.fault-data-f00",
        fault_evidence_rated_rpm => "h100-spindle.fault-data-f01",
        fault_evidence_minimum_rpm => "h100-spindle.fault-data-f02",
        fault_evidence_maximum_rpm => "h100-spindle.fault-data-f03",
        fault_evidence_at_speed_tolerance_hz => "h100-spindle.fault-data-f04",
        fault_evidence_calculated_frequency_hz => "h100-spindle.fault-data-f05",
    );

    unsafe {
        float_parameter(
            ptr::addr_of_mut!((*component).config.rated_rpm),
            b"h100-spindle.rated-rpm\0",
            component_id,
        )?;
        float_parameter(
            ptr::addr_of_mut!((*component).config.minimum_rpm),
            b"h100-spindle.minimum-rpm\0",
            component_id,
        )?;
        float_parameter(
            ptr::addr_of_mut!((*component).config.maximum_rpm),
            b"h100-spindle.maximum-rpm\0",
            component_id,
        )?;
        u32_parameter(
            ptr::addr_of_mut!((*component).config.expected_reference_f004_centihz),
            b"h100-spindle.expected-reference-f004-centihz\0",
            component_id,
        )?;
        u32_parameter(
            ptr::addr_of_mut!((*component).config.expected_maximum_f005_centihz),
            b"h100-spindle.expected-maximum-f005-centihz\0",
            component_id,
        )?;
        float_parameter(
            ptr::addr_of_mut!((*component).config.at_speed_tolerance_hz),
            b"h100-spindle.at-speed-tolerance-hz\0",
            component_id,
        )?;
    }
    Ok(())
}

unsafe fn write<T: Copy>(pointer: *mut T, value: T) {
    unsafe { ptr::write_volatile(pointer, value) }
}

pub(super) unsafe fn publish_initial_values(component: &mut Component) {
    let pins: &Pins = &component.pins;
    unsafe {
        write(pins.machine_enabled, false);
        write(pins.run_request, false);
        write(pins.forward_request, false);
        write(pins.reverse_request, false);
        write(pins.speed_command_rpm, 0.0);
        write(pins.reset, false);
        write(pins.link_fault, true);
        for pointer in pins.command_disabled {
            write(pointer, false);
        }
        for pointer in [
            pins.control_mode_f001,
            pins.frequency_source_f002,
            pins.reference_f004_centihz,
            pins.maximum_f005_centihz,
            pins.lower_limit_f011_centihz,
            pins.panel_stop_f024,
            pins.slave_address_f163,
            pins.baud_selector_f164,
            pins.data_mode_f165,
            pins.frequency_decimals_f169,
            pins.output_frequency_decihz,
            pins.current_fault,
            pins.main_status,
            pins.given_frequency_readback,
        ] {
            write(pointer, 0);
        }
        write(pins.main_control, CONTROL_STOP);
        write(pins.given_frequency, 0);
        write(pins.ready, false);
        write(pins.running, false);
        write(pins.forward_running, false);
        write(pins.reverse_running, false);
        write(pins.at_speed, true);
        write(pins.fault_latched, false);
        write(pins.fault_code, BlockCode::None as u32);
        write(pins.block_code, BlockCode::None as u32);
        write(pins.state, State::Stopped as u32);
        for pointer in pins.fault_kind {
            write(pointer, false);
        }
        for (index, pointer) in pins.block_kind.iter().copied().enumerate() {
            write(pointer, BlockCode::ALL[index] == BlockCode::None);
        }
        for (index, pointer) in pins.state_kind.iter().copied().enumerate() {
            write(pointer, State::ALL[index] == State::Stopped);
        }
        for pointer in [
            pins.vfd_fault_present,
            pins.fault_kind_unknown,
            pins.block_kind_unknown,
            pins.state_kind_unknown,
            pins.vfd_fault_known,
            pins.vfd_fault_unknown,
            pins.main_status_unknown,
            pins.fault_evidence_valid,
            pins.fault_evidence_context_fault_latched_before,
            pins.fault_evidence_context_fault_record_present_before,
            pins.fault_evidence_machine_enabled,
            pins.fault_evidence_run_request,
            pins.fault_evidence_forward_request,
            pins.fault_evidence_reverse_request,
            pins.fault_evidence_reset,
            pins.fault_evidence_link_fault,
            pins.fault_evidence_command_disabled,
        ] {
            write(pointer, false);
        }
        write(pins.main_status_known, true);
        for pointer in pins.main_status_bit {
            write(pointer, false);
        }
        write(pins.main_status_reserved_mask, 0);
        for pointer in pins.vfd_fault_family {
            write(pointer, false);
        }
        for pointer in pins.vfd_fault_phase {
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
        write(pins.speed_feedback_rpm, 0.0);
        write(pins.target_frequency_hz, 0.0);
    }
}
