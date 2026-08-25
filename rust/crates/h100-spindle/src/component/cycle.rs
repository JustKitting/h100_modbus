use core::ffi::{c_long, c_void};
use core::ptr;

use crate::sequencer::Input;

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
    unsafe {
        write(pins.main_control, output.main_control);
        write(pins.given_frequency, output.given_frequency);
        write(pins.ready, output.ready);
        write(pins.running, output.running);
        write(pins.forward_running, output.forward_running);
        write(pins.reverse_running, output.reverse_running);
        write(pins.at_speed, output.at_speed);
        write(pins.fault_latched, output.fault_latched);
        write(pins.fault_code, output.fault_code);
        write(pins.block_code, output.block_code);
        write(pins.state, output.state);
        write(pins.speed_feedback_rpm, output.speed_feedback_rpm);
        write(pins.target_frequency_hz, output.target_frequency_hz);
    }
}
