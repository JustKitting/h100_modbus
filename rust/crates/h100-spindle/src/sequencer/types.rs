pub const CONTROL_FORWARD: u32 = 0x0001;
pub const CONTROL_REVERSE: u32 = 0x0004;
pub const CONTROL_STOP: u32 = 0x0008;
pub const STATUS_IN_OPERATION: u32 = 0x0008;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum State {
    Stopped = 0,
    Arming = 1,
    Starting = 2,
    Running = 3,
    Stopping = 4,
    Fault = 5,
}

impl State {
    pub(super) const fn from_raw(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Stopped),
            1 => Some(Self::Arming),
            2 => Some(Self::Starting),
            3 => Some(Self::Running),
            4 => Some(Self::Stopping),
            5 => Some(Self::Fault),
            _ => None,
        }
    }

    pub(super) const fn active(self) -> bool {
        matches!(self, Self::Arming | Self::Starting | Self::Running)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BlockCode {
    None = 0,
    Link = 1,
    F001 = 2,
    F002 = 3,
    F024 = 4,
    F163 = 5,
    F164 = 6,
    F165 = 7,
    F169 = 8,
    ExplicitFrequency = 9,
    F004 = 10,
    F005 = 11,
    RpmLimits = 12,
    VfdFault = 13,
    DirectionChange = 14,
    Direction = 15,
    SpeedZero = 16,
    SpeedLow = 17,
    SpeedHigh = 18,
    BelowF011 = 19,
    FrequencyRange = 20,
    SpeedInvalid = 21,
    CommandDisabled = 22,
    InternalState = 23,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub machine_enabled: bool,
    pub run_request: bool,
    pub forward_request: bool,
    pub reverse_request: bool,
    pub speed_command_rpm: f64,
    pub reset: bool,
    pub link_fault: bool,
    pub any_command_disabled: bool,
    pub control_mode_f001: u32,
    pub frequency_source_f002: u32,
    pub reference_f004_centihz: u32,
    pub maximum_f005_centihz: u32,
    pub lower_limit_f011_centihz: u32,
    pub panel_stop_f024: u32,
    pub slave_address_f163: u32,
    pub baud_selector_f164: u32,
    pub data_mode_f165: u32,
    pub frequency_decimals_f169: u32,
    pub output_frequency_decihz: u32,
    pub current_fault: u32,
    pub main_status: u32,
    pub given_frequency_readback: u32,
}

impl Input {
    pub const fn safe() -> Self {
        Self {
            machine_enabled: false,
            run_request: false,
            forward_request: false,
            reverse_request: false,
            speed_command_rpm: 0.0,
            reset: false,
            link_fault: true,
            any_command_disabled: false,
            control_mode_f001: 0,
            frequency_source_f002: 0,
            reference_f004_centihz: 0,
            maximum_f005_centihz: 0,
            lower_limit_f011_centihz: 0,
            panel_stop_f024: 0,
            slave_address_f163: 0,
            baud_selector_f164: 0,
            data_mode_f165: 0,
            frequency_decimals_f169: 0,
            output_frequency_decihz: 0,
            current_fault: 0,
            main_status: 0,
            given_frequency_readback: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub rated_rpm: f64,
    pub minimum_rpm: f64,
    pub maximum_rpm: f64,
    pub expected_reference_f004_centihz: u32,
    pub expected_maximum_f005_centihz: u32,
    pub at_speed_tolerance_hz: f64,
}

impl Config {
    pub const fn safe() -> Self {
        Self {
            rated_rpm: 24_000.0,
            minimum_rpm: 0.0,
            maximum_rpm: 24_000.0,
            expected_reference_f004_centihz: 0,
            expected_maximum_f005_centihz: 0,
            at_speed_tolerance_hz: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    pub(super) state: i32,
    pub(super) previous_reset: bool,
    pub(super) fault_latched: bool,
    pub(super) fault_code: u32,
    pub(super) held_frequency: u32,
    pub(super) held_target_hz: f64,
    pub(super) held_reverse: bool,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            state: State::Stopping as i32,
            previous_reset: false,
            fault_latched: false,
            fault_code: BlockCode::None as u32,
            held_frequency: 0,
            held_target_hz: 0.0,
            held_reverse: false,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Output {
    pub main_control: u32,
    pub given_frequency: u32,
    pub ready: bool,
    pub running: bool,
    pub forward_running: bool,
    pub reverse_running: bool,
    pub at_speed: bool,
    pub fault_latched: bool,
    pub fault_code: u32,
    pub block_code: u32,
    pub state: u32,
    pub speed_feedback_rpm: f64,
    pub target_frequency_hz: f64,
}
