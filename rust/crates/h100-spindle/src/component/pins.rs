use core::ptr;

use dmc2_hal_sys as hal;

use crate::sequencer::{Config, Context};

pub(super) struct Pins {
    pub(super) machine_enabled: *mut hal::hal_bit_t,
    pub(super) run_request: *mut hal::hal_bit_t,
    pub(super) forward_request: *mut hal::hal_bit_t,
    pub(super) reverse_request: *mut hal::hal_bit_t,
    pub(super) speed_command_rpm: *mut hal::real_t,
    pub(super) reset: *mut hal::hal_bit_t,
    pub(super) link_fault: *mut hal::hal_bit_t,
    pub(super) command_disabled: [*mut hal::hal_bit_t; 13],
    pub(super) control_mode_f001: *mut hal::hal_u32_t,
    pub(super) frequency_source_f002: *mut hal::hal_u32_t,
    pub(super) reference_f004_centihz: *mut hal::hal_u32_t,
    pub(super) maximum_f005_centihz: *mut hal::hal_u32_t,
    pub(super) lower_limit_f011_centihz: *mut hal::hal_u32_t,
    pub(super) panel_stop_f024: *mut hal::hal_u32_t,
    pub(super) slave_address_f163: *mut hal::hal_u32_t,
    pub(super) baud_selector_f164: *mut hal::hal_u32_t,
    pub(super) data_mode_f165: *mut hal::hal_u32_t,
    pub(super) frequency_decimals_f169: *mut hal::hal_u32_t,
    pub(super) output_frequency_decihz: *mut hal::hal_u32_t,
    pub(super) current_fault: *mut hal::hal_u32_t,
    pub(super) main_status: *mut hal::hal_u32_t,
    pub(super) given_frequency_readback: *mut hal::hal_u32_t,
    pub(super) main_control: *mut hal::hal_u32_t,
    pub(super) given_frequency: *mut hal::hal_u32_t,
    pub(super) ready: *mut hal::hal_bit_t,
    pub(super) running: *mut hal::hal_bit_t,
    pub(super) forward_running: *mut hal::hal_bit_t,
    pub(super) reverse_running: *mut hal::hal_bit_t,
    pub(super) at_speed: *mut hal::hal_bit_t,
    pub(super) fault_latched: *mut hal::hal_bit_t,
    pub(super) fault_code: *mut hal::hal_u32_t,
    pub(super) block_code: *mut hal::hal_u32_t,
    pub(super) state: *mut hal::hal_u32_t,
    pub(super) speed_feedback_rpm: *mut hal::real_t,
    pub(super) target_frequency_hz: *mut hal::real_t,
}

impl Pins {
    const fn new() -> Self {
        Self {
            machine_enabled: ptr::null_mut(),
            run_request: ptr::null_mut(),
            forward_request: ptr::null_mut(),
            reverse_request: ptr::null_mut(),
            speed_command_rpm: ptr::null_mut(),
            reset: ptr::null_mut(),
            link_fault: ptr::null_mut(),
            command_disabled: [ptr::null_mut(); 13],
            control_mode_f001: ptr::null_mut(),
            frequency_source_f002: ptr::null_mut(),
            reference_f004_centihz: ptr::null_mut(),
            maximum_f005_centihz: ptr::null_mut(),
            lower_limit_f011_centihz: ptr::null_mut(),
            panel_stop_f024: ptr::null_mut(),
            slave_address_f163: ptr::null_mut(),
            baud_selector_f164: ptr::null_mut(),
            data_mode_f165: ptr::null_mut(),
            frequency_decimals_f169: ptr::null_mut(),
            output_frequency_decihz: ptr::null_mut(),
            current_fault: ptr::null_mut(),
            main_status: ptr::null_mut(),
            given_frequency_readback: ptr::null_mut(),
            main_control: ptr::null_mut(),
            given_frequency: ptr::null_mut(),
            ready: ptr::null_mut(),
            running: ptr::null_mut(),
            forward_running: ptr::null_mut(),
            reverse_running: ptr::null_mut(),
            at_speed: ptr::null_mut(),
            fault_latched: ptr::null_mut(),
            fault_code: ptr::null_mut(),
            block_code: ptr::null_mut(),
            state: ptr::null_mut(),
            speed_feedback_rpm: ptr::null_mut(),
            target_frequency_hz: ptr::null_mut(),
        }
    }
}

pub(super) struct Component {
    pub(super) pins: Pins,
    pub(super) config: Config,
    pub(super) context: Context,
}

impl Component {
    pub(super) const fn new() -> Self {
        Self {
            pins: Pins::new(),
            config: Config::safe(),
            context: Context::new(),
        }
    }
}
