use super::step::configuration_block;
use super::*;
use dmc2_diagnostics::SelfDescribingDiagnostic;
use std::collections::BTreeSet;

fn valid_input() -> Input {
    Input {
        link_fault: false,
        control_mode_f001: 2,
        frequency_source_f002: 2,
        reference_f004_centihz: 4_000,
        maximum_f005_centihz: 4_000,
        panel_stop_f024: 1,
        slave_address_f163: 1,
        baud_selector_f164: 2,
        data_mode_f165: 3,
        frequency_decimals_f169: 0,
        ..Input::safe()
    }
}

fn valid_config() -> Config {
    Config {
        rated_rpm: 24_000.0,
        minimum_rpm: 6_000.0,
        maximum_rpm: 24_000.0,
        expected_reference_f004_centihz: 4_000,
        expected_maximum_f005_centihz: 4_000,
        at_speed_tolerance_hz: 1.0,
    }
}

fn running_status(vfd_reverse_selected: bool) -> u32 {
    MainStatusBit::Operation.wire_code()
        | MainStatusBit::InOperation.wire_code()
        | if vfd_reverse_selected {
            MainStatusBit::Reverse.wire_code()
        } else {
            0
        }
}

fn running_fixture() -> (Context, Input, Config, Output) {
    let mut context = Context::new();
    let mut input = valid_input();
    let config = valid_config();
    context.step(input, config);
    input.machine_enabled = true;
    input.run_request = true;
    input.forward_request = true;
    input.speed_command_rpm = 12_000.0;
    context.step(input, config);
    input.given_frequency_readback = 2_000;
    context.step(input, config);
    input.main_status = running_status(true);
    input.output_frequency_decihz = 2_000;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Running as u32);
    (context, input, config, output)
}

fn assert_run_fault(mut input: Input, config: Config, mut context: Context, expected: BlockCode) {
    input.machine_enabled = true;
    input.run_request = true;
    if input.speed_command_rpm == 0.0 {
        input.speed_command_rpm = 12_000.0;
    }
    let output = context.step(input, config);
    assert!(output.fault_latched);
    assert_eq!(output.fault_code, expected as u32);
    assert_eq!(output.block_code, expected as u32);
    assert_eq!(output.main_control, CONTROL_STOP);
    assert_eq!(output.state, State::Fault as u32);
    assert_eq!(
        output.fault_record.map(|record| record.code),
        Some(expected)
    );
}

fn record(code: BlockCode) -> BlockRecord {
    BlockRecord {
        code,
        evidence: BlockEvidence::capture(
            valid_input(),
            valid_config(),
            State::Stopped.wire_code(),
            false,
            BlockCode::None.wire_code(),
            None,
        ),
    }
}

#[test]
fn every_nonzero_code_is_stable_unique_and_self_describing() {
    assert_eq!(BlockCode::ALL.len(), 25);
    assert_eq!(BlockCode::DIAGNOSTICS.len(), 24);
    let mut names = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    for (index, code) in BlockCode::ALL.iter().copied().enumerate() {
        assert_eq!(code.wire_code(), index as u32);
        assert_eq!(BlockCode::from_wire_code(index as u32), Some(code));
        assert!(!code.name().is_empty());
        assert!(!code.hal_slug().is_empty());
        assert!(names.insert(code.name()));
        assert!(slugs.insert(code.hal_slug()));
        if code != BlockCode::None {
            assert!(code.metadata().complete(), "{}", code.name());
        }
    }
    for unknown in [25, u32::MAX] {
        assert_eq!(BlockCode::from_wire_code(unknown), None);
    }
}

#[test]
fn every_manual_main_status_bit_has_stable_identity_cause_action_and_mask() {
    let mut names = BTreeSet::new();
    let mut slugs = BTreeSet::new();
    let mut mask = 0_u32;
    for bit in MainStatusBit::ALL {
        assert!(bit.metadata().complete());
        assert!(names.insert(bit.name()));
        assert!(slugs.insert(bit.hal_slug()));
        assert_eq!(mask & bit.wire_code(), 0);
        mask |= bit.wire_code();
    }
    assert_eq!(mask, MainStatusBit::KNOWN_MASK);
    assert_eq!(main_status_reserved_mask(0x8108), 0x8100);
}

#[test]
fn every_configuration_block_code_and_priority_is_exact() {
    let input = valid_input();
    let config = valid_config();
    assert_eq!(configuration_block(input, config), BlockCode::None);

    let cases: &[(fn(&mut Input, &mut Config), BlockCode)] = &[
        (|input, _| input.link_fault = true, BlockCode::Link),
        (
            |input, _| input.any_command_disabled = true,
            BlockCode::CommandDisabled,
        ),
        (|input, _| input.control_mode_f001 = 1, BlockCode::F001),
        (|input, _| input.frequency_source_f002 = 1, BlockCode::F002),
        (|input, _| input.panel_stop_f024 = 0, BlockCode::F024),
        (|input, _| input.slave_address_f163 = 2, BlockCode::F163),
        (|input, _| input.baud_selector_f164 = 1, BlockCode::F164),
        (|input, _| input.data_mode_f165 = 2, BlockCode::F165),
        (
            |input, _| input.frequency_decimals_f169 = 2,
            BlockCode::F169,
        ),
        (
            |_, config| config.expected_reference_f004_centihz = 0,
            BlockCode::ExplicitFrequency,
        ),
        (
            |_, config| config.expected_maximum_f005_centihz = 0,
            BlockCode::ExplicitFrequency,
        ),
        (
            |input, _| input.reference_f004_centihz = 3_999,
            BlockCode::F004,
        ),
        (
            |input, _| input.maximum_f005_centihz = 3_999,
            BlockCode::F005,
        ),
        (
            |_, config| config.rated_rpm = f64::NAN,
            BlockCode::RpmLimits,
        ),
        (
            |_, config| config.minimum_rpm = f64::NAN,
            BlockCode::RpmLimits,
        ),
        (
            |_, config| config.maximum_rpm = f64::NAN,
            BlockCode::RpmLimits,
        ),
        (
            |_, config| config.at_speed_tolerance_hz = f64::NAN,
            BlockCode::RpmLimits,
        ),
        (|_, config| config.rated_rpm = 0.0, BlockCode::RpmLimits),
        (|_, config| config.minimum_rpm = 0.0, BlockCode::RpmLimits),
        (
            |_, config| config.maximum_rpm = 5_999.0,
            BlockCode::RpmLimits,
        ),
        (
            |_, config| config.maximum_rpm = 24_001.0,
            BlockCode::RpmLimits,
        ),
        (
            |_, config| config.at_speed_tolerance_hz = -0.001,
            BlockCode::RpmLimits,
        ),
        (|input, _| input.current_fault = 1, BlockCode::VfdFault),
    ];
    for (mutate, expected) in cases {
        let mut input = valid_input();
        let mut config = valid_config();
        mutate(&mut input, &mut config);
        assert_eq!(configuration_block(input, config), *expected);
    }

    let mut every_fault = valid_input();
    let mut every_bad_config = valid_config();
    every_fault.link_fault = true;
    every_fault.any_command_disabled = true;
    every_fault.control_mode_f001 = 0;
    every_bad_config.minimum_rpm = 0.0;
    assert_eq!(
        configuration_block(every_fault, every_bad_config),
        BlockCode::Link
    );
}

#[test]
fn full_forward_start_speed_change_and_stop_is_exact() {
    assert_eq!(CONTROL_FORWARD, 0x0002);
    assert_eq!(CONTROL_REVERSE, 0x0004);
    assert_eq!(CONTROL_STOP, 0x0008);

    let mut context = Context::new();
    let mut input = valid_input();
    let config = valid_config();
    let output = context.step(input, config);
    assert!(output.ready);
    assert_eq!(output.state, State::Stopped as u32);
    assert!(output.at_speed);
    assert_eq!(output.main_control, CONTROL_STOP);
    assert_eq!(output.given_frequency, 0);

    input.machine_enabled = true;
    input.run_request = true;
    input.forward_request = true;
    input.speed_command_rpm = 12_000.0;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Arming as u32);
    assert_eq!(output.main_control, CONTROL_STOP);
    assert_eq!(output.given_frequency, 2_000);
    assert_eq!(output.target_frequency_hz, 200.0);
    assert!(!output.at_speed);

    input.given_frequency_readback = 2_000;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Starting as u32);
    assert_eq!(output.main_control, CONTROL_REVERSE);

    input.main_status = running_status(true);
    input.output_frequency_decihz = 2_000;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Running as u32);
    assert!(output.forward_running);
    assert!(!output.reverse_running);
    assert!(output.at_speed);
    assert_eq!(output.speed_feedback_rpm, 12_000.0);

    input.speed_command_rpm = 18_000.0;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Starting as u32);
    assert_eq!(output.given_frequency, 3_000);
    input.given_frequency_readback = 3_000;
    input.output_frequency_decihz = 3_000;
    assert_eq!(context.step(input, config).state, State::Running as u32);

    input.run_request = false;
    input.forward_request = false;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Stopping as u32);
    assert_eq!(output.main_control, CONTROL_STOP);
    assert_eq!(output.given_frequency, 3_000);
    input.main_status = 0;
    input.output_frequency_decihz = 0;
    let output = context.step(input, config);
    assert_eq!(output.state, State::Stopped as u32);
    assert_eq!(output.given_frequency, 0);
}

#[test]
fn reverse_mapping_and_direction_change_refusal_are_exact() {
    let mut context = Context::new();
    let mut input = valid_input();
    let config = valid_config();
    context.step(input, config);
    input.machine_enabled = true;
    input.run_request = true;
    input.reverse_request = true;
    input.speed_command_rpm = 12_000.0;
    assert_eq!(context.step(input, config).state, State::Arming as u32);
    input.given_frequency_readback = 2_000;
    assert_eq!(context.step(input, config).main_control, CONTROL_FORWARD);
    input.main_status = running_status(false);
    input.output_frequency_decihz = 2_000;
    let output = context.step(input, config);
    assert!(output.reverse_running);
    assert!(!output.forward_running);

    input.reverse_request = false;
    input.forward_request = true;
    let output = context.step(input, config);
    assert!(output.fault_latched);
    assert_eq!(output.fault_code, BlockCode::DirectionChange as u32);
    assert_eq!(output.main_control, CONTROL_STOP);
}

#[test]
fn run_and_at_speed_feedback_require_the_selected_physical_direction() {
    let config = valid_config();

    let mut clockwise_context = Context::new();
    let mut clockwise_input = valid_input();
    clockwise_context.step(clockwise_input, config);
    clockwise_input.machine_enabled = true;
    clockwise_input.run_request = true;
    clockwise_input.forward_request = true;
    clockwise_input.speed_command_rpm = 12_000.0;
    clockwise_context.step(clockwise_input, config);
    clockwise_input.given_frequency_readback = 2_000;
    assert_eq!(
        clockwise_context.step(clockwise_input, config).main_control,
        CONTROL_REVERSE
    );
    clockwise_input.output_frequency_decihz = 2_000;
    clockwise_input.main_status = running_status(false);
    let wrong_direction = clockwise_context.step(clockwise_input, config);
    assert!(wrong_direction.running);
    assert_eq!(wrong_direction.state, State::Starting.wire_code() as u32);
    assert!(!wrong_direction.forward_running);
    assert!(!wrong_direction.reverse_running);
    assert!(!wrong_direction.at_speed);

    clockwise_input.main_status = running_status(true);
    let confirmed_clockwise = clockwise_context.step(clockwise_input, config);
    assert_eq!(confirmed_clockwise.state, State::Running.wire_code() as u32);
    assert!(confirmed_clockwise.forward_running);
    assert!(!confirmed_clockwise.reverse_running);
    assert!(confirmed_clockwise.at_speed);

    let mut counterclockwise_context = Context::new();
    let mut counterclockwise_input = valid_input();
    counterclockwise_context.step(counterclockwise_input, config);
    counterclockwise_input.machine_enabled = true;
    counterclockwise_input.run_request = true;
    counterclockwise_input.reverse_request = true;
    counterclockwise_input.speed_command_rpm = 12_000.0;
    counterclockwise_context.step(counterclockwise_input, config);
    counterclockwise_input.given_frequency_readback = 2_000;
    assert_eq!(
        counterclockwise_context
            .step(counterclockwise_input, config)
            .main_control,
        CONTROL_FORWARD
    );
    counterclockwise_input.output_frequency_decihz = 2_000;
    counterclockwise_input.main_status = running_status(true);
    let wrong_direction = counterclockwise_context.step(counterclockwise_input, config);
    assert!(wrong_direction.running);
    assert_eq!(wrong_direction.state, State::Starting.wire_code() as u32);
    assert!(!wrong_direction.forward_running);
    assert!(!wrong_direction.reverse_running);
    assert!(!wrong_direction.at_speed);

    counterclockwise_input.main_status = running_status(false);
    let confirmed_counterclockwise = counterclockwise_context.step(counterclockwise_input, config);
    assert_eq!(
        confirmed_counterclockwise.state,
        State::Running.wire_code() as u32
    );
    assert!(!confirmed_counterclockwise.forward_running);
    assert!(confirmed_counterclockwise.reverse_running);
    assert!(confirmed_counterclockwise.at_speed);
}

#[test]
fn direction_feedback_loss_after_confirmation_latches_stop_with_exact_evidence() {
    let (mut context, mut input, config, _) = running_fixture();

    input.main_status = running_status(false);
    let output = context.step(input, config);
    assert!(output.fault_latched);
    assert_eq!(output.fault_code, BlockCode::DirectionFeedback.wire_code());
    assert_eq!(output.main_control, CONTROL_STOP);
    assert_eq!(output.state, State::Stopping.wire_code() as u32);
    assert!(!output.forward_running);
    assert!(!output.reverse_running);
    assert!(!output.at_speed);
    let evidence = output
        .fault_record
        .expect("direction fault evidence")
        .evidence;
    assert_eq!(evidence.input.main_status, running_status(false));
    assert!(evidence.input.run_request);
    assert!(evidence.input.forward_request);
    assert!(!evidence.input.reverse_request);
}

#[test]
fn every_run_refusal_code_has_an_exact_trigger() {
    let config = valid_config();

    let input = valid_input();
    assert_run_fault(input, config, Context::new(), BlockCode::Direction);
    let mut input = valid_input();
    input.forward_request = true;
    input.reverse_request = true;
    assert_run_fault(input, config, Context::new(), BlockCode::Direction);

    let mut input = valid_input();
    input.reverse_request = true;
    let mut context = Context::new();
    context.state = State::Arming as i32;
    context.held_frequency = 2_000;
    context.held_target_hz = 200.0;
    assert_run_fault(input, config, context, BlockCode::DirectionChange);

    for speed in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut input = valid_input();
        input.forward_request = true;
        input.speed_command_rpm = speed;
        assert_run_fault(input, config, Context::new(), BlockCode::SpeedInvalid);
    }
    for speed in [0.0, -1.0] {
        let mut input = valid_input();
        input.forward_request = true;
        input.speed_command_rpm = speed;
        input.machine_enabled = true;
        input.run_request = true;
        let output = Context::new().step(input, config);
        assert_eq!(output.fault_code, BlockCode::SpeedZero as u32);
    }
    for (speed, code) in [
        (5_999.0, BlockCode::SpeedLow),
        (24_001.0, BlockCode::SpeedHigh),
    ] {
        let mut input = valid_input();
        input.forward_request = true;
        input.speed_command_rpm = speed;
        assert_run_fault(input, config, Context::new(), code);
    }

    let mut input = valid_input();
    input.forward_request = true;
    input.lower_limit_f011_centihz = 2_001;
    assert_run_fault(input, config, Context::new(), BlockCode::BelowF011);

    let mut input = valid_input();
    input.forward_request = true;
    input.frequency_decimals_f169 = 1;
    input.reference_f004_centihz = 10_000;
    input.maximum_f005_centihz = 10_000;
    input.speed_command_rpm = 24_000.0;
    let mut range_config = config;
    range_config.expected_reference_f004_centihz = 10_000;
    range_config.expected_maximum_f005_centihz = 10_000;
    range_config.minimum_rpm = 24_000.0;
    assert_run_fault(
        input,
        range_config,
        Context::new(),
        BlockCode::FrequencyRange,
    );

    let mut input = valid_input();
    input.forward_request = true;
    input.reference_f004_centihz = 5_000;
    input.speed_command_rpm = 24_000.0;
    let mut range_config = config;
    range_config.expected_reference_f004_centihz = 5_000;
    range_config.minimum_rpm = 24_000.0;
    assert_run_fault(
        input,
        range_config,
        Context::new(),
        BlockCode::FrequencyRange,
    );

    let mut input = valid_input();
    input.forward_request = true;
    input.reference_f004_centihz = 1;
    input.maximum_f005_centihz = 1;
    input.speed_command_rpm = f64::from_bits(1);
    let range_config = Config {
        rated_rpm: f64::MAX,
        minimum_rpm: f64::from_bits(1),
        maximum_rpm: f64::from_bits(1),
        expected_reference_f004_centihz: 1,
        expected_maximum_f005_centihz: 1,
        at_speed_tolerance_hz: 1.0,
    };
    assert_run_fault(
        input,
        range_config,
        Context::new(),
        BlockCode::FrequencyRange,
    );

    let mut input = valid_input();
    input.forward_request = true;
    input.reference_f004_centihz = 1;
    input.maximum_f005_centihz = 1;
    input.speed_command_rpm = 0.1;
    let mut range_config = config;
    range_config.minimum_rpm = 0.1;
    range_config.expected_reference_f004_centihz = 1;
    range_config.expected_maximum_f005_centihz = 1;
    assert_run_fault(
        input,
        range_config,
        Context::new(),
        BlockCode::FrequencyRange,
    );
}

#[test]
fn configuration_faults_latch_from_every_active_state_and_preserve_stop() {
    for state in [State::Arming, State::Starting, State::Running] {
        let mut input = valid_input();
        input.machine_enabled = true;
        input.run_request = true;
        input.forward_request = true;
        input.speed_command_rpm = 12_000.0;
        input.link_fault = true;
        let mut context = Context::new();
        context.state = state as i32;
        context.held_frequency = 2_000;
        context.held_target_hz = 200.0;
        let output = context.step(input, valid_config());
        assert!(output.fault_latched);
        assert_eq!(output.fault_code, BlockCode::Link as u32);
        assert_eq!(output.main_control, CONTROL_STOP);
    }
}

#[test]
fn frequency_resolution_limits_feedback_and_stop_feedback_are_exact() {
    for (rpm, decimals, raw) in [
        (6_000.0, 0, 1_000),
        (24_000.0, 0, 4_000),
        (12_000.0, 1, 20_000),
    ] {
        let mut context = Context::new();
        context.state = State::Stopped as i32;
        let mut input = valid_input();
        input.machine_enabled = true;
        input.run_request = true;
        input.forward_request = true;
        input.speed_command_rpm = rpm;
        input.frequency_decimals_f169 = decimals;
        assert_eq!(context.step(input, valid_config()).given_frequency, raw);
    }

    let (mut context, mut input, config, _) = running_fixture();
    for (frequency, expected) in [
        (1_990, State::Running),
        (2_010, State::Running),
        (1_989, State::Starting),
        (2_011, State::Starting),
    ] {
        input.output_frequency_decihz = frequency;
        assert_eq!(context.step(input, config).state, expected as u32);
    }

    input.run_request = false;
    input.forward_request = false;
    assert_eq!(context.step(input, config).state, State::Stopping as u32);
    input.main_status = STATUS_IN_OPERATION;
    input.output_frequency_decihz = 0;
    assert_eq!(context.step(input, config).state, State::Stopping as u32);
    input.main_status = 0;
    input.output_frequency_decihz = 1;
    assert_eq!(context.step(input, config).state, State::Stopping as u32);
    input.output_frequency_decihz = 0;
    assert_eq!(context.step(input, config).state, State::Stopped as u32);
}

#[test]
fn reset_and_internal_state_contracts_are_fail_closed() {
    let mut context = Context::new();
    context.state = State::Fault as i32;
    context.fault_latched = true;
    context.fault_code = BlockCode::Direction as u32;
    context.fault_record = Some(record(BlockCode::Direction));
    let mut input = valid_input();
    let config = valid_config();

    input.reset = true;
    input.run_request = true;
    assert!(context.step(input, config).fault_latched);
    input.run_request = false;
    assert!(context.step(input, config).fault_latched);
    input.reset = false;
    context.step(input, config);
    input.reset = true;
    let output = context.step(input, config);
    assert!(!output.fault_latched);
    assert_eq!(output.state, State::Stopped as u32);

    for (state, latched, code, record_code) in [
        (-1, false, 0, None),
        (6, true, BlockCode::Link as u32, Some(BlockCode::Link)),
        (-1, true, BlockCode::Link as u32, Some(BlockCode::Link)),
        (State::Stopped as i32, true, 0, None),
        (State::Stopped as i32, false, BlockCode::Link as u32, None),
        (State::Stopped as i32, true, BlockCode::Link as u32, None),
        (
            State::Stopped as i32,
            true,
            BlockCode::Link as u32,
            Some(BlockCode::Direction),
        ),
        (
            State::Stopped as i32,
            false,
            BlockCode::None as u32,
            Some(BlockCode::Link),
        ),
        (State::Stopped as i32, true, u32::MAX, None),
    ] {
        let mut context = Context::new();
        context.state = state;
        context.fault_latched = latched;
        context.fault_code = code;
        context.fault_record = record_code.map(record);
        let output = context.step(valid_input(), config);
        assert!(output.fault_latched);
        assert_eq!(output.fault_code, BlockCode::InternalState.wire_code());
        assert_eq!(
            output.fault_record.map(|record| record.code),
            Some(BlockCode::InternalState)
        );
        let evidence = output.fault_record.expect("internal evidence").evidence;
        assert_eq!(evidence.state_before, state);
        assert_eq!(evidence.context_fault_latched_before, latched);
        assert_eq!(evidence.context_fault_code_before, code);
        assert_eq!(
            evidence.context_fault_record_present_before,
            record_code.is_some()
        );
        assert_eq!(
            evidence.context_fault_record_code_before,
            record_code.unwrap_or(BlockCode::None).wire_code()
        );
        assert_eq!(output.state, State::Fault as u32);
    }
}

#[test]
fn exhaustive_boolean_state_matrix_preserves_all_output_invariants() {
    let speeds = [0.0, 5_999.0, 12_000.0, 24_001.0, f64::NAN];
    let frequencies = [0, 1_990, 2_000, 2_010];
    let mut cases = 0_u32;
    for state in 0..=State::Fault as i32 {
        for flags in 0_u32..256 {
            for previous_reset in [false, true] {
                for held_reverse in [false, true] {
                    for speed in speeds {
                        for frequency in frequencies {
                            let mut input = valid_input();
                            let config = valid_config();
                            let mut context = Context::new();
                            context.state = state;
                            context.previous_reset = previous_reset;
                            context.fault_latched = flags & 0x80 != 0;
                            context.fault_code = if context.fault_latched {
                                BlockCode::Link as u32
                            } else {
                                BlockCode::None as u32
                            };
                            context.fault_record =
                                context.fault_latched.then(|| record(BlockCode::Link));
                            context.held_frequency = 2_000;
                            context.held_target_hz = 200.0;
                            context.held_reverse = held_reverse;
                            input.machine_enabled = flags & 1 != 0;
                            input.run_request = flags & 2 != 0;
                            input.forward_request = flags & 4 != 0;
                            input.reverse_request = flags & 8 != 0;
                            input.reset = flags & 16 != 0;
                            input.link_fault = flags & 32 != 0;
                            input.any_command_disabled = flags & 64 != 0;
                            input.main_status = if flags & 128 != 0 {
                                STATUS_IN_OPERATION
                            } else {
                                0
                            };
                            input.speed_command_rpm = speed;
                            input.output_frequency_decihz = frequency;
                            let base = configuration_block(input, config);
                            let output = context.step(input, config);
                            cases += 1;
                            assert!(output.state <= State::Fault as u32);
                            assert!([CONTROL_STOP, CONTROL_FORWARD, CONTROL_REVERSE]
                                .contains(&output.main_control));
                            assert!(!(output.forward_running && output.reverse_running));
                            assert!(!output.forward_running || output.running);
                            assert!(!output.reverse_running || output.running);
                            assert!(!output.fault_latched || output.main_control == CONTROL_STOP);
                            assert_eq!(
                                output.ready,
                                base == BlockCode::None && !output.fault_latched
                            );
                            assert_eq!(
                                output.at_speed,
                                !input.run_request
                                    || (output.state == State::Running as u32 && output.running)
                            );
                            assert!(output.block_code <= BlockCode::DirectionFeedback as u32);
                            assert!(output.fault_code <= BlockCode::DirectionFeedback as u32);
                            assert_eq!(output.fault_latched, output.fault_record.is_some());
                            if let Some(record) = output.fault_record {
                                assert_eq!(record.code.wire_code(), output.fault_code);
                            }
                            if output.main_control != CONTROL_STOP {
                                assert!(matches!(
                                    output.state,
                                    value if value == State::Starting as u32
                                        || value == State::Running as u32
                                ));
                            }
                            if output.state == State::Stopped as u32 {
                                assert_eq!(output.given_frequency, 0);
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 122_880);
}
