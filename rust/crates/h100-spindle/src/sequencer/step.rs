use super::types::{
    BlockCode, BlockEvidence, BlockRecord, Config, Context, Input, MainStatusBit, Output, State,
    CONTROL_FORWARD, CONTROL_REVERSE, CONTROL_STOP, STATUS_IN_OPERATION,
};

const PARAMETER_UNITS_PER_HZ: f64 = 10.0;

pub fn configuration_block(input: Input, config: Config) -> BlockCode {
    if input.link_fault {
        return BlockCode::Link;
    }
    if input.any_command_disabled {
        return BlockCode::CommandDisabled;
    }
    if input.control_mode_f001 != 2 {
        return BlockCode::F001;
    }
    if input.frequency_source_f002 != 2 {
        return BlockCode::F002;
    }
    if input.panel_stop_f024 != 1 {
        return BlockCode::F024;
    }
    if input.slave_address_f163 != 1 {
        return BlockCode::F163;
    }
    if input.baud_selector_f164 != 2 {
        return BlockCode::F164;
    }
    if input.data_mode_f165 != 3 {
        return BlockCode::F165;
    }
    if input.frequency_decimals_f169 > 1 {
        return BlockCode::F169;
    }
    if config.expected_reference_f004_centihz == 0 || config.expected_maximum_f005_centihz == 0 {
        return BlockCode::ExplicitFrequency;
    }
    if input.reference_f004_centihz != config.expected_reference_f004_centihz {
        return BlockCode::F004;
    }
    if input.maximum_f005_centihz != config.expected_maximum_f005_centihz {
        return BlockCode::F005;
    }
    if !config.rated_rpm.is_finite()
        || !config.minimum_rpm.is_finite()
        || !config.maximum_rpm.is_finite()
        || !config.at_speed_tolerance_hz.is_finite()
        || config.rated_rpm <= 0.0
        || config.minimum_rpm <= 0.0
        || config.maximum_rpm < config.minimum_rpm
        || config.maximum_rpm > config.rated_rpm
        || config.at_speed_tolerance_hz < 0.0
    {
        return BlockCode::RpmLimits;
    }
    if input.current_fault != 0 {
        return BlockCode::VfdFault;
    }
    BlockCode::None
}

impl Context {
    fn set_state(&mut self, state: State) {
        self.state = state.wire_code();
    }

    fn state(&self) -> State {
        State::from_raw(self.state).expect("state is validated before use")
    }

    fn latch(&mut self, code: BlockCode, evidence: BlockEvidence) {
        if !self.fault_latched {
            self.fault_record = Some(BlockRecord { code, evidence });
            self.fault_code = code.wire_code();
        }
        self.fault_latched = true;
        self.set_state(State::Stopping);
    }

    fn validate_internal_state(
        &mut self,
        input: Input,
        config: Config,
        state_before: i32,
        fault_latched_before: bool,
        fault_code_before: u32,
        fault_record_code_before: Option<BlockCode>,
    ) -> bool {
        let state = State::from_raw(self.state);
        let decoded_fault = BlockCode::from_wire_code(self.fault_code);
        let fault_consistent = if self.fault_latched {
            decoded_fault.is_some_and(|code| code != BlockCode::None)
                && self
                    .fault_record
                    .is_some_and(|record| Some(record.code) == decoded_fault)
        } else {
            self.fault_code == BlockCode::None.wire_code() && self.fault_record.is_none()
        };
        if state.is_none() || !fault_consistent {
            self.fault_latched = true;
            self.fault_code = BlockCode::InternalState.wire_code();
            self.fault_record = Some(BlockRecord {
                code: BlockCode::InternalState,
                evidence: BlockEvidence::capture(
                    input,
                    config,
                    state_before,
                    fault_latched_before,
                    fault_code_before,
                    fault_record_code_before,
                ),
            });
            self.set_state(State::Fault);
            return false;
        }
        true
    }

    pub fn step(&mut self, input: Input, config: Config) -> Output {
        let state_before = self.state;
        let fault_latched_before = self.fault_latched;
        let fault_code_before = self.fault_code;
        let fault_record_code_before = self.fault_record.map(|record| record.code);
        let is_running = input.main_status & STATUS_IN_OPERATION != 0;
        let vfd_reverse_selected = input.main_status & MainStatusBit::Reverse.wire_code() != 0;
        let stopped_feedback = !is_running && input.output_frequency_decihz == 0;
        let actual_hz = input.output_frequency_decihz as f64 / 10.0;
        let mut output = Output {
            main_control: CONTROL_STOP,
            given_frequency: self.held_frequency,
            ready: false,
            running: is_running,
            forward_running: false,
            reverse_running: false,
            at_speed: false,
            fault_latched: false,
            fault_code: 0,
            block_code: 0,
            state: 0,
            fault_record: None,
            block_record: None,
            state_kind: State::Stopped,
            speed_feedback_rpm: 0.0,
            target_frequency_hz: 0.0,
        };
        if config.rated_rpm > 0.0 && config.expected_reference_f004_centihz > 0 {
            output.speed_feedback_rpm = actual_hz * config.rated_rpm * PARAMETER_UNITS_PER_HZ
                / config.expected_reference_f004_centihz as f64;
        }

        let base_reason = configuration_block(input, config);
        let mut run_reason = base_reason;
        let requested_reverse = input.reverse_request && !input.forward_request;
        let direction_feedback_matches = vfd_reverse_selected == !self.held_reverse;
        let reset_rising = input.reset && !self.previous_reset;
        self.previous_reset = input.reset;
        let internal_state_valid = self.validate_internal_state(
            input,
            config,
            state_before,
            fault_latched_before,
            fault_code_before,
            fault_record_code_before,
        );
        if internal_state_valid
            && reset_rising
            && !input.run_request
            && stopped_feedback
            && !input.link_fault
            && input.current_fault == 0
        {
            self.fault_latched = false;
            self.fault_code = BlockCode::None.wire_code();
            self.fault_record = None;
            self.set_state(State::Stopping);
        }
        if self.fault_latched && !matches!(self.state(), State::Stopping | State::Fault) {
            self.set_state(State::Stopping);
        }

        let mut requested_raw = 0_u32;
        let mut requested_hz = 0.0;
        if input.run_request && input.machine_enabled && !self.fault_latched {
            if run_reason == BlockCode::None && input.forward_request == input.reverse_request {
                run_reason = BlockCode::Direction;
            }
            if run_reason == BlockCode::None
                && self.state().active()
                && requested_reverse != self.held_reverse
            {
                run_reason = BlockCode::DirectionChange;
            }
            if run_reason == BlockCode::None && !input.speed_command_rpm.is_finite() {
                run_reason = BlockCode::SpeedInvalid;
            }
            if run_reason == BlockCode::None && input.speed_command_rpm <= 0.0 {
                run_reason = BlockCode::SpeedZero;
            }
            if run_reason == BlockCode::None && input.speed_command_rpm < config.minimum_rpm {
                run_reason = BlockCode::SpeedLow;
            }
            if run_reason == BlockCode::None && input.speed_command_rpm > config.maximum_rpm {
                run_reason = BlockCode::SpeedHigh;
            }
            if run_reason == BlockCode::None {
                requested_hz = input.speed_command_rpm
                    * (config.expected_reference_f004_centihz as f64 / PARAMETER_UNITS_PER_HZ)
                    / config.rated_rpm;
                if requested_hz * PARAMETER_UNITS_PER_HZ < input.lower_limit_f011_centihz as f64 {
                    run_reason = BlockCode::BelowF011;
                }
            }
            if run_reason == BlockCode::None {
                let units_per_hz = if input.frequency_decimals_f169 == 0 {
                    10.0
                } else {
                    100.0
                };
                let raw_float = requested_hz * units_per_hz;
                if raw_float <= 0.0
                    || raw_float > 65_535.0
                    || requested_hz * PARAMETER_UNITS_PER_HZ
                        > config.expected_maximum_f005_centihz as f64
                {
                    run_reason = BlockCode::FrequencyRange;
                } else {
                    requested_raw = (raw_float + 0.5) as u32;
                    if requested_raw == 0 {
                        run_reason = BlockCode::FrequencyRange;
                    }
                }
            }
            if run_reason != BlockCode::None {
                self.latch(
                    run_reason,
                    BlockEvidence {
                        calculated_frequency_hz: requested_hz,
                        calculated_frequency_register: requested_raw,
                        ..BlockEvidence::capture(
                            input,
                            config,
                            state_before,
                            fault_latched_before,
                            fault_code_before,
                            fault_record_code_before,
                        )
                    },
                );
            }
        }

        if self.state().active() && base_reason != BlockCode::None {
            self.latch(
                base_reason,
                BlockEvidence::capture(
                    input,
                    config,
                    state_before,
                    fault_latched_before,
                    fault_code_before,
                    fault_record_code_before,
                ),
            );
        }
        if self.state() == State::Running
            && input.run_request
            && input.machine_enabled
            && is_running
            && !direction_feedback_matches
        {
            self.latch(
                BlockCode::DirectionFeedback,
                BlockEvidence::capture(
                    input,
                    config,
                    state_before,
                    fault_latched_before,
                    fault_code_before,
                    fault_record_code_before,
                ),
            );
        }
        if (!input.run_request || !input.machine_enabled) && self.state().active() {
            self.set_state(State::Stopping);
        }

        match self.state() {
            State::Stopped => {
                output.main_control = CONTROL_STOP;
                output.given_frequency = 0;
                self.held_frequency = 0;
                self.held_target_hz = 0.0;
                self.held_reverse = false;
                if input.run_request && input.machine_enabled {
                    self.held_frequency = requested_raw;
                    self.held_target_hz = requested_hz;
                    self.held_reverse = requested_reverse;
                    output.given_frequency = self.held_frequency;
                    self.set_state(State::Arming);
                }
            }
            State::Arming => {
                output.main_control = CONTROL_STOP;
                self.held_frequency = requested_raw;
                self.held_target_hz = requested_hz;
                output.given_frequency = self.held_frequency;
                if input.given_frequency_readback == self.held_frequency {
                    output.main_control = if self.held_reverse {
                        CONTROL_FORWARD
                    } else {
                        CONTROL_REVERSE
                    };
                    self.set_state(State::Starting);
                }
            }
            State::Starting | State::Running => {
                output.main_control = if self.held_reverse {
                    CONTROL_FORWARD
                } else {
                    CONTROL_REVERSE
                };
                self.held_frequency = requested_raw;
                self.held_target_hz = requested_hz;
                output.given_frequency = self.held_frequency;
                let frequency_error = (actual_hz - self.held_target_hz).abs();
                // This installation's physically verified mapping is:
                // LinuxCNC M3/CW -> H100 Reverse and LinuxCNC M4/CCW ->
                // H100 Forward.  Do not report a requested direction ready
                // merely because the drive is rotating at the requested
                // frequency; 0210H must also report the selected H100
                // direction that corresponds to that request.
                if is_running
                    && direction_feedback_matches
                    && frequency_error <= config.at_speed_tolerance_hz
                {
                    self.set_state(State::Running);
                } else {
                    self.set_state(State::Starting);
                }
            }
            State::Stopping => {
                output.main_control = CONTROL_STOP;
                output.given_frequency = self.held_frequency;
                if stopped_feedback {
                    output.given_frequency = 0;
                    self.held_frequency = 0;
                    self.held_target_hz = 0.0;
                    self.held_reverse = false;
                    self.set_state(if self.fault_latched {
                        State::Fault
                    } else {
                        State::Stopped
                    });
                }
            }
            State::Fault => {
                output.main_control = CONTROL_STOP;
                if stopped_feedback {
                    output.given_frequency = 0;
                    self.held_frequency = 0;
                    self.held_target_hz = 0.0;
                    self.held_reverse = false;
                } else {
                    output.given_frequency = self.held_frequency;
                }
                self.set_state(State::Fault);
            }
        }

        output.forward_running = is_running
            && matches!(self.state(), State::Starting | State::Running)
            && direction_feedback_matches
            && !self.held_reverse;
        output.reverse_running = is_running
            && matches!(self.state(), State::Starting | State::Running)
            && direction_feedback_matches
            && self.held_reverse;
        output.ready = base_reason == BlockCode::None && !self.fault_latched;
        output.at_speed = !input.run_request || self.state() == State::Running;
        output.fault_latched = self.fault_latched;
        output.fault_code = self.fault_code;
        let block_code = if input.run_request {
            run_reason.wire_code()
        } else {
            base_reason.wire_code()
        };
        output.block_code = block_code;
        output.state = self.state as u32;
        output.fault_record = self.fault_record;
        output.block_record = BlockCode::from_wire_code(block_code)
            .filter(|code| *code != BlockCode::None)
            .map(|code| BlockRecord {
                code,
                evidence: BlockEvidence {
                    calculated_frequency_hz: requested_hz,
                    calculated_frequency_register: requested_raw,
                    ..BlockEvidence::capture(
                        input,
                        config,
                        state_before,
                        fault_latched_before,
                        fault_code_before,
                        fault_record_code_before,
                    )
                },
            });
        output.state_kind = self.state();
        output.target_frequency_hz = self.held_target_hz;
        output
    }
}
