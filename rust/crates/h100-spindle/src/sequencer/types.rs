use dmc2_diagnostics::{DiagnosticMetadata, SelfDescribingDiagnostic};

// H100 0200H maps bit 0 to the generic Operation command and bit 1 to the
// explicit Forward command.  Operation alone does not clear a previously
// latched Reverse selection, so direction-aware control must use bit 1.
pub const CONTROL_FORWARD: u32 = 0x0002;
pub const CONTROL_REVERSE: u32 = 0x0004;
pub const CONTROL_STOP: u32 = 0x0008;
pub const STATUS_IN_OPERATION: u32 = 0x0008;

/// Exact H100 holding-register 0210H bits, sourced from manual V1.8 printed
/// pages 82 and 85. Bits 8..15 map reserved parameter addresses 0008H..000FH
/// and are therefore never assigned guessed meanings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MainStatusBit {
    Operation = 0x0001,
    Jog = 0x0002,
    Reverse = 0x0004,
    InOperation = 0x0008,
    InJogging = 0x0010,
    InReverseRotation = 0x0020,
    InBraking = 0x0040,
    FrequencyTracking = 0x0080,
}

impl MainStatusBit {
    pub const COUNT: usize = 8;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Operation,
        Self::Jog,
        Self::Reverse,
        Self::InOperation,
        Self::InJogging,
        Self::InReverseRotation,
        Self::InBraking,
        Self::FrequencyTracking,
    ];
    pub const KNOWN_MASK: u32 = 0x00ff;

    pub const fn wire_code(self) -> u32 {
        self as u32
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Operation => "H100_STATUS_OPERATION",
            Self::Jog => "H100_STATUS_JOG",
            Self::Reverse => "H100_STATUS_REVERSE",
            Self::InOperation => "H100_STATUS_IN_OPERATION",
            Self::InJogging => "H100_STATUS_IN_JOGGING",
            Self::InReverseRotation => "H100_STATUS_IN_REVERSE_ROTATION",
            Self::InBraking => "H100_STATUS_IN_BRAKING",
            Self::FrequencyTracking => "H100_STATUS_FREQUENCY_TRACKING",
        }
    }

    pub const fn hal_slug(self) -> &'static str {
        match self {
            Self::Operation => "operation",
            Self::Jog => "jog",
            Self::Reverse => "reverse",
            Self::InOperation => "in-operation",
            Self::InJogging => "in-jogging",
            Self::InReverseRotation => "in-reverse-rotation",
            Self::InBraking => "in-braking",
            Self::FrequencyTracking => "frequency-tracking",
        }
    }

    pub const fn summary(self) -> &'static str {
        match self {
            Self::Operation => "H100 status reports the operation command active",
            Self::Jog => "H100 status reports the jog command active",
            Self::Reverse => "H100 status reports the reverse direction selected",
            Self::InOperation => "H100 status reports the inverter in operation",
            Self::InJogging => "H100 status reports the inverter in jogging",
            Self::InReverseRotation => "H100 status reports reverse physical rotation",
            Self::InBraking => "H100 status reports active braking",
            Self::FrequencyTracking => "H100 status reports active frequency tracking",
        }
    }

    pub const fn action(self) -> &'static str {
        match self {
            Self::Operation | Self::InOperation => {
                "compare this status with the requested run state and sequencer state"
            }
            Self::Jog | Self::InJogging => {
                "verify that no unintended H100 jog command source is active"
            }
            Self::Reverse | Self::InReverseRotation => {
                "compare this direction status with the requested spindle direction"
            }
            Self::InBraking => "wait for braking to clear before expecting stopped feedback",
            Self::FrequencyTracking => {
                "compare frequency-tracking status with the intended H100 operating mode"
            }
        }
    }
}

impl SelfDescribingDiagnostic for MainStatusBit {
    fn metadata(self) -> DiagnosticMetadata {
        DiagnosticMetadata::new(
            self.wire_code() as i64,
            self.name(),
            self.hal_slug(),
            self.summary(),
            self.action(),
        )
    }
}

pub const fn main_status_reserved_mask(raw: u32) -> u32 {
    raw & !MainStatusBit::KNOWN_MASK
}

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
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Stopped,
        Self::Arming,
        Self::Starting,
        Self::Running,
        Self::Stopping,
        Self::Fault,
    ];

    pub const fn from_raw(value: i32) -> Option<Self> {
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

    pub const fn wire_code(self) -> i32 {
        self as i32
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Stopped => "STOPPED",
            Self::Arming => "ARMING",
            Self::Starting => "STARTING",
            Self::Running => "RUNNING",
            Self::Stopping => "STOPPING",
            Self::Fault => "FAULT",
        }
    }

    pub const fn hal_slug(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Arming => "arming",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Fault => "fault",
        }
    }

    pub const fn summary(self) -> &'static str {
        match self {
            Self::Stopped => "the spindle sequencer has verified stopped feedback",
            Self::Arming => "the commanded frequency is being written and verified before rotation",
            Self::Starting => {
                "rotation is commanded and verified speed feedback has not yet reached tolerance"
            }
            Self::Running => "rotation and speed feedback are both verified within tolerance",
            Self::Stopping => "the stop command is asserted while zero-speed feedback is awaited",
            Self::Fault => "the spindle sequencer is latched faulted and continues to command stop",
        }
    }

    pub const fn action(self) -> &'static str {
        match self {
            Self::Stopped | Self::Running => "no state-specific operator action is required",
            Self::Arming => "wait for the frequency-command readback to match before expecting rotation",
            Self::Starting => "wait for verified run and speed feedback or inspect the accompanying block diagnostic",
            Self::Stopping => "wait for both the run indication and output-frequency feedback to clear",
            Self::Fault => "inspect the named latched fault and retained evidence before requesting reset",
        }
    }

    pub(super) const fn active(self) -> bool {
        matches!(self, Self::Arming | Self::Starting | Self::Running)
    }
}

impl SelfDescribingDiagnostic for State {
    fn metadata(self) -> DiagnosticMetadata {
        DiagnosticMetadata::new(
            self.wire_code() as i64,
            self.name(),
            self.hal_slug(),
            self.summary(),
            self.action(),
        )
    }
}

macro_rules! block_catalog {
    ($(
        $variant:ident = $code:literal,
        $name:literal,
        $slug:literal,
        $summary:literal,
        $action:literal;
    )+) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(u32)]
        pub enum BlockCode {
            $($variant = $code,)+
        }

        impl BlockCode {
            pub const COUNT: usize = block_catalog!(@count $($variant)+);
            pub const ALL: [Self; Self::COUNT] = [$(Self::$variant,)+];
            pub const fn wire_code(self) -> u32 {
                self as u32
            }

            pub const fn from_wire_code(code: u32) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub const fn hal_slug(self) -> &'static str {
                match self {
                    $(Self::$variant => $slug,)+
                }
            }

            pub const fn summary(self) -> &'static str {
                match self {
                    $(Self::$variant => $summary,)+
                }
            }

            pub const fn action(self) -> &'static str {
                match self {
                    $(Self::$variant => $action,)+
                }
            }
        }

        impl SelfDescribingDiagnostic for BlockCode {
            fn metadata(self) -> DiagnosticMetadata {
                DiagnosticMetadata::new(
                    self.wire_code() as i64,
                    self.name(),
                    self.hal_slug(),
                    self.summary(),
                    self.action(),
                )
            }
        }
    };
    (@count $head:ident $($tail:ident)*) => { 1usize + block_catalog!(@count $($tail)*) };
    (@count) => { 0usize };
}

block_catalog! {
    None = 0,
    "NONE",
    "none",
    "no spindle fault or command block is present",
    "no operator action is required";
    Link = 1,
    "MODBUS_LINK_FAILURE",
    "modbus-link-failure",
    "the H100 Modbus link reports a communication failure",
    "inspect the linked Modbus diagnostic and restore verified communication";
    F001 = 2,
    "F001_CONTROL_MODE_MISMATCH",
    "f001-control-mode-mismatch",
    "observed F001 is not the required communication control mode value 2",
    "set and verify H100 parameter F001=2 before requesting spindle motion";
    F002 = 3,
    "F002_FREQUENCY_SOURCE_MISMATCH",
    "f002-frequency-source-mismatch",
    "observed F002 is not the required communication frequency-source value 2",
    "set and verify H100 parameter F002=2 before requesting spindle motion";
    F024 = 4,
    "F024_PANEL_STOP_MISMATCH",
    "f024-panel-stop-mismatch",
    "observed F024 is not the required panel-stop-enabled value 1",
    "set and verify H100 parameter F024=1";
    F163 = 5,
    "F163_SLAVE_ADDRESS_MISMATCH",
    "f163-slave-address-mismatch",
    "observed F163 is not the configured Modbus slave address 1",
    "set and verify H100 parameter F163=1 and the matching host address";
    F164 = 6,
    "F164_BAUD_MISMATCH",
    "f164-baud-mismatch",
    "observed F164 is not the required baud selector value 2",
    "set and verify H100 parameter F164=2 and matching host baud";
    F165 = 7,
    "F165_DATA_MODE_MISMATCH",
    "f165-data-mode-mismatch",
    "observed F165 is not the required serial data-mode value 3",
    "set and verify H100 parameter F165=3 and matching host framing";
    F169 = 8,
    "F169_DECIMAL_MODE_INVALID",
    "f169-decimal-mode-invalid",
    "observed F169 is outside the supported frequency decimal modes 0 and 1",
    "set and verify H100 parameter F169 as 0 or 1";
    ExplicitFrequency = 9,
    "EXPLICIT_FREQUENCY_LIMIT_MISSING",
    "explicit-frequency-limit-missing",
    "the configured expected F004 or F005 value is zero and therefore unresolved",
    "configure both expected-reference-f004-centihz and expected-maximum-f005-centihz";
    F004 = 10,
    "F004_REFERENCE_FREQUENCY_MISMATCH",
    "f004-reference-frequency-mismatch",
    "observed F004 differs from the explicitly configured reference frequency",
    "compare the retained observed and expected F004 values and correct the mismatch";
    F005 = 11,
    "F005_MAXIMUM_FREQUENCY_MISMATCH",
    "f005-maximum-frequency-mismatch",
    "observed F005 differs from the explicitly configured maximum frequency",
    "compare the retained observed and expected F005 values and correct the mismatch";
    RpmLimits = 12,
    "RPM_CONFIGURATION_INVALID",
    "rpm-configuration-invalid",
    "rated, minimum, maximum, or at-speed configuration violates its finite ordered bounds",
    "inspect the retained RPM configuration and correct every invalid bound";
    VfdFault = 13,
    "H100_VFD_FAULT",
    "h100-vfd-fault",
    "the H100 current-fault input register reports a nonzero drive fault",
    "inspect the source-backed H100 fault family/phase and clear its physical cause";
    DirectionChange = 14,
    "DIRECTION_CHANGE_WHILE_ACTIVE",
    "direction-change-while-active",
    "the requested spindle direction changed while the sequencer was active",
    "stop fully before issuing the opposite direction";
    Direction = 15,
    "DIRECTION_REQUEST_INVALID",
    "direction-request-invalid",
    "forward and reverse requests were both asserted or both clear for a run request",
    "assert exactly one direction request";
    SpeedZero = 16,
    "SPEED_NOT_POSITIVE",
    "speed-not-positive",
    "the requested run speed is zero or negative",
    "request a positive speed within the configured limits";
    SpeedLow = 17,
    "SPEED_BELOW_MINIMUM",
    "speed-below-minimum",
    "the requested RPM is below the configured minimum RPM",
    "raise the request or deliberately lower the verified minimum configuration";
    SpeedHigh = 18,
    "SPEED_ABOVE_MAXIMUM",
    "speed-above-maximum",
    "the requested RPM exceeds the configured maximum RPM",
    "lower the request to the verified spindle and tool limit";
    BelowF011 = 19,
    "FREQUENCY_BELOW_F011",
    "frequency-below-f011",
    "the requested frequency is below the observed H100 F011 lower limit",
    "raise the request or deliberately reconfigure and verify F011";
    FrequencyRange = 20,
    "FREQUENCY_ENCODING_OUT_OF_RANGE",
    "frequency-encoding-out-of-range",
    "the requested frequency cannot fit the H100 register or configured F005 range",
    "inspect the retained RPM, calculated hertz, F169, and F005 evidence";
    SpeedInvalid = 21,
    "SPEED_NOT_FINITE",
    "speed-not-finite",
    "the requested RPM is NaN or infinite",
    "correct the upstream speed command before resetting";
    CommandDisabled = 22,
    "MODBUS_COMMAND_DISABLED",
    "modbus-command-disabled",
    "at least one required Modbus command instance is disabled",
    "identify and enable every required command instance";
    InternalState = 23,
    "INTERNAL_STATE_INCONSISTENT",
    "internal-state-inconsistent",
    "the sequencer state or latched-fault fields violated an internal invariant",
    "retain the diagnostic evidence and restart only after correcting the software/state fault";
    DirectionFeedback = 24,
    "DIRECTION_FEEDBACK_MISMATCH",
    "direction-feedback-mismatch",
    "H100 direction status changed away from the already verified requested spindle direction while running",
    "keep the spindle stopped and inspect the retained command and H100 0210H status evidence before resetting";
}

impl BlockCode {
    pub const DIAGNOSTICS: [Self; Self::COUNT - 1] = [
        Self::Link,
        Self::F001,
        Self::F002,
        Self::F024,
        Self::F163,
        Self::F164,
        Self::F165,
        Self::F169,
        Self::ExplicitFrequency,
        Self::F004,
        Self::F005,
        Self::RpmLimits,
        Self::VfdFault,
        Self::DirectionChange,
        Self::Direction,
        Self::SpeedZero,
        Self::SpeedLow,
        Self::SpeedHigh,
        Self::BelowF011,
        Self::FrequencyRange,
        Self::SpeedInvalid,
        Self::CommandDisabled,
        Self::InternalState,
        Self::DirectionFeedback,
    ];
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
pub struct BlockEvidence {
    pub input: Input,
    pub config: Config,
    pub state_before: i32,
    pub context_fault_latched_before: bool,
    pub context_fault_code_before: u32,
    pub context_fault_record_present_before: bool,
    pub context_fault_record_code_before: u32,
    pub calculated_frequency_hz: f64,
    pub calculated_frequency_register: u32,
}

impl BlockEvidence {
    pub const fn capture(
        input: Input,
        config: Config,
        state_before: i32,
        context_fault_latched_before: bool,
        context_fault_code_before: u32,
        context_fault_record_code_before: Option<BlockCode>,
    ) -> Self {
        Self {
            input,
            config,
            state_before,
            context_fault_latched_before,
            context_fault_code_before,
            context_fault_record_present_before: context_fault_record_code_before.is_some(),
            context_fault_record_code_before: match context_fault_record_code_before {
                Some(code) => code.wire_code(),
                None => BlockCode::None.wire_code(),
            },
            calculated_frequency_hz: 0.0,
            calculated_frequency_register: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockRecord {
    pub code: BlockCode,
    pub evidence: BlockEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    pub(super) state: i32,
    pub(super) previous_reset: bool,
    pub(super) fault_latched: bool,
    pub(super) fault_code: u32,
    pub(super) fault_record: Option<BlockRecord>,
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
            fault_record: None,
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
    pub fault_record: Option<BlockRecord>,
    pub block_record: Option<BlockRecord>,
    pub state_kind: State,
    pub speed_feedback_rpm: f64,
    pub target_frequency_hz: f64,
}
