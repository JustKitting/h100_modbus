use core::ffi::{c_char, c_int};
use core::ptr;

use dmc2_hal_sys as hal;

use crate::sequencer::{BlockCode, State, CONTROL_STOP};

use super::pins::{Component, Pins};

unsafe fn bit_pin(
    pointer: *mut *mut hal::hal_bit_t,
    name: &'static [u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), c_int> {
    let result = unsafe {
        hal::hal_pin_bit_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

unsafe fn u32_pin(
    pointer: *mut *mut hal::hal_u32_t,
    name: &'static [u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), c_int> {
    let result = unsafe {
        hal::hal_pin_u32_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

unsafe fn float_pin(
    pointer: *mut *mut hal::real_t,
    name: &'static [u8],
    direction: hal::hal_pin_dir_t,
    component_id: c_int,
) -> Result<(), c_int> {
    let result = unsafe {
        hal::hal_pin_float_new(
            name.as_ptr().cast::<c_char>(),
            direction,
            pointer,
            component_id,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

unsafe fn float_parameter(
    pointer: *mut hal::real_t,
    name: &'static [u8],
    component_id: c_int,
) -> Result<(), c_int> {
    let result = unsafe {
        hal::hal_param_float_new(
            name.as_ptr().cast::<c_char>(),
            hal::hal_param_dir_t_HAL_RW,
            pointer,
            component_id,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

unsafe fn u32_parameter(
    pointer: *mut hal::hal_u32_t,
    name: &'static [u8],
    component_id: c_int,
) -> Result<(), c_int> {
    let result = unsafe {
        hal::hal_param_u32_new(
            name.as_ptr().cast::<c_char>(),
            hal::hal_param_dir_t_HAL_RW,
            pointer,
            component_id,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

macro_rules! scalar_pins {
    ($function:ident, $pins:ident, $direction:ident, $component_id:ident; $($field:ident => $name:literal),+ $(,)?) => {
        $(unsafe {
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
        $(unsafe {
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
) -> Result<(), c_int> {
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
        main_control => "h100-spindle.main-control",
        given_frequency => "h100-spindle.given-frequency",
        fault_code => "h100-spindle.fault-code",
        block_code => "h100-spindle.block-code",
        state => "h100-spindle.state",
    );
    scalar_pins!(bit_pin, pins, output, component_id;
        ready => "h100-spindle.ready",
        running => "h100-spindle.running",
        forward_running => "h100-spindle.forward-running",
        reverse_running => "h100-spindle.reverse-running",
        at_speed => "h100-spindle.at-speed",
        fault_latched => "h100-spindle.fault-latched",
    );
    scalar_pins!(float_pin, pins, output, component_id;
        speed_feedback_rpm => "h100-spindle.speed-feedback-rpm",
        target_frequency_hz => "h100-spindle.target-frequency-hz",
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
        write(pins.speed_feedback_rpm, 0.0);
        write(pins.target_frequency_hz, 0.0);
    }
}
