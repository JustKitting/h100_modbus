use super::*;

use std::collections::BTreeSet;
use std::ffi::{c_char, c_int, c_long, c_void, CStr};
use std::string::{String, ToString};
use std::sync::Mutex;
use std::vec::Vec;

use dmc2_hal_sys as hal;

use crate::sequencer::{
    BlockCode, MainStatusBit, State, VfdFaultFamily, VfdFaultPhase, CONTROL_REVERSE, CONTROL_STOP,
    STATUS_IN_OPERATION,
};

const COMPONENT_ID: c_int = 41;
const PIN_COUNT: usize = 169;
const PARAMETER_COUNT: usize = 6;
const INTERFACE_COUNT: usize = PIN_COUNT + PARAMETER_COUNT;

#[repr(align(16))]
struct Arena([u8; 32_768]);

static mut ARENA: Arena = Arena([0; 32_768]);
static mut ARENA_OFFSET: usize = 0;
static TEST_LOCK: Mutex<()> = Mutex::new(());
static RECORDS: Mutex<Vec<Record>> = Mutex::new(Vec::new());
static FUNCTION: Mutex<Option<(usize, usize)>> = Mutex::new(None);
static PLAN: Mutex<Plan> = Mutex::new(Plan::success());
static CALLS: Mutex<Calls> = Mutex::new(Calls::new());
static MESSAGES: Mutex<Vec<(hal::msg_level_t, String)>> = Mutex::new(Vec::new());

#[derive(Clone, Copy)]
struct Plan {
    init_result: c_int,
    malloc_fails: bool,
    registration_failure: Option<(usize, c_int)>,
    export_result: c_int,
    ready_result: c_int,
    exit_result: c_int,
}

impl Plan {
    const fn success() -> Self {
        Self {
            init_result: COMPONENT_ID,
            malloc_fails: false,
            registration_failure: None,
            export_result: 0,
            ready_result: 0,
            exit_result: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Calls {
    init: usize,
    malloc: usize,
    registration: usize,
    export: usize,
    ready: usize,
    exit: usize,
}

impl Calls {
    const fn new() -> Self {
        Self {
            init: 0,
            malloc: 0,
            registration: 0,
            export: 0,
            ready: 0,
            exit: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Kind {
    BitPin,
    FloatPin,
    S32Pin,
    U32Pin,
    FloatParameter,
    U32Parameter,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Record {
    name: String,
    address: usize,
    kind: Kind,
    direction: u32,
}

unsafe fn allocate(size: usize) -> *mut c_void {
    let aligned = unsafe { (ARENA_OFFSET + 15) & !15 };
    let end = aligned.saturating_add(size);
    if end > 32_768 {
        return ptr::null_mut();
    }
    unsafe {
        ARENA_OFFSET = end;
        ptr::addr_of_mut!(ARENA.0)
            .cast::<u8>()
            .add(aligned)
            .cast::<c_void>()
    }
}

fn registration_result() -> Option<c_int> {
    let call = {
        let mut calls = CALLS.lock().expect("call lock");
        let call = calls.registration;
        calls.registration += 1;
        call
    };
    PLAN.lock()
        .expect("plan lock")
        .registration_failure
        .filter(|(failure_call, _)| *failure_call == call)
        .map(|(_, error)| error)
}

unsafe fn register_pin<T>(
    name: *const c_char,
    direction: hal::hal_pin_dir_t,
    pointer: *mut *mut T,
    component_id: c_int,
    kind: Kind,
) -> c_int {
    assert_eq!(component_id, COMPONENT_ID);
    if let Some(error) = registration_result() {
        return error;
    }
    let data = unsafe { allocate(core::mem::size_of::<T>()) }.cast::<T>();
    if data.is_null() {
        return ENOMEM;
    }
    unsafe {
        ptr::write_bytes(data, 0, 1);
        ptr::write(pointer, data);
    }
    record(name, data.cast::<c_void>(), kind, direction as u32);
    0
}

fn record(name: *const c_char, address: *mut c_void, kind: Kind, direction: u32) {
    let name = unsafe { CStr::from_ptr(name) }
        .to_str()
        .expect("HAL name was UTF-8")
        .to_string();
    RECORDS.lock().expect("record lock").push(Record {
        name,
        address: address as usize,
        kind,
        direction,
    });
}

unsafe fn register_parameter<T>(
    name: *const c_char,
    direction: hal::hal_param_dir_t,
    pointer: *mut T,
    component_id: c_int,
    kind: Kind,
) -> c_int {
    assert_eq!(component_id, COMPONENT_ID);
    if let Some(error) = registration_result() {
        return error;
    }
    assert!(!pointer.is_null());
    record(name, pointer.cast::<c_void>(), kind, direction as u32);
    0
}

#[no_mangle]
extern "C" fn hal_init(name: *const c_char) -> c_int {
    assert_eq!(unsafe { CStr::from_ptr(name) }.to_bytes(), b"h100_spindle");
    CALLS.lock().expect("call lock").init += 1;
    PLAN.lock().expect("plan lock").init_result
}

#[no_mangle]
extern "C" fn hal_exit(component_id: c_int) -> c_int {
    assert_eq!(component_id, COMPONENT_ID);
    CALLS.lock().expect("call lock").exit += 1;
    PLAN.lock().expect("plan lock").exit_result
}

#[no_mangle]
extern "C" fn hal_ready(component_id: c_int) -> c_int {
    assert_eq!(component_id, COMPONENT_ID);
    CALLS.lock().expect("call lock").ready += 1;
    PLAN.lock().expect("plan lock").ready_result
}

#[no_mangle]
extern "C" fn hal_malloc(size: c_long) -> *mut c_void {
    CALLS.lock().expect("call lock").malloc += 1;
    if size <= 0 || PLAN.lock().expect("plan lock").malloc_fails {
        ptr::null_mut()
    } else {
        unsafe { allocate(size as usize) }
    }
}

#[no_mangle]
extern "C" fn hal_pin_bit_new(
    name: *const c_char,
    direction: hal::hal_pin_dir_t,
    pointer: *mut *mut bool,
    component_id: c_int,
) -> c_int {
    unsafe { register_pin(name, direction, pointer, component_id, Kind::BitPin) }
}

#[no_mangle]
extern "C" fn hal_pin_float_new(
    name: *const c_char,
    direction: hal::hal_pin_dir_t,
    pointer: *mut *mut f64,
    component_id: c_int,
) -> c_int {
    unsafe { register_pin(name, direction, pointer, component_id, Kind::FloatPin) }
}

#[no_mangle]
extern "C" fn hal_pin_s32_new(
    name: *const c_char,
    direction: hal::hal_pin_dir_t,
    pointer: *mut *mut i32,
    component_id: c_int,
) -> c_int {
    unsafe { register_pin(name, direction, pointer, component_id, Kind::S32Pin) }
}

#[no_mangle]
extern "C" fn hal_pin_u32_new(
    name: *const c_char,
    direction: hal::hal_pin_dir_t,
    pointer: *mut *mut u32,
    component_id: c_int,
) -> c_int {
    unsafe { register_pin(name, direction, pointer, component_id, Kind::U32Pin) }
}

#[no_mangle]
extern "C" fn hal_param_float_new(
    name: *const c_char,
    direction: hal::hal_param_dir_t,
    pointer: *mut f64,
    component_id: c_int,
) -> c_int {
    unsafe { register_parameter(name, direction, pointer, component_id, Kind::FloatParameter) }
}

#[no_mangle]
extern "C" fn hal_param_u32_new(
    name: *const c_char,
    direction: hal::hal_param_dir_t,
    pointer: *mut u32,
    component_id: c_int,
) -> c_int {
    unsafe { register_parameter(name, direction, pointer, component_id, Kind::U32Parameter) }
}

#[no_mangle]
extern "C" fn hal_export_funct(
    name: *const c_char,
    function: Option<unsafe extern "C" fn(*mut c_void, c_long)>,
    argument: *mut c_void,
    uses_fp: c_int,
    reentrant: c_int,
    component_id: c_int,
) -> c_int {
    assert_eq!(unsafe { CStr::from_ptr(name) }.to_bytes(), b"h100-spindle");
    assert_eq!(uses_fp, 1);
    assert_eq!(reentrant, 0);
    assert_eq!(component_id, COMPONENT_ID);
    CALLS.lock().expect("call lock").export += 1;
    let result = PLAN.lock().expect("plan lock").export_result;
    if result == 0 {
        *FUNCTION.lock().expect("function lock") = Some((
            function.expect("realtime function was provided") as usize,
            argument as usize,
        ));
    }
    result
}

#[no_mangle]
extern "C" fn rtapi_print_msg(level: hal::msg_level_t, message: *const c_char) {
    let message = unsafe { CStr::from_ptr(message) }
        .to_str()
        .expect("RTAPI message was UTF-8")
        .to_string();
    MESSAGES
        .lock()
        .expect("message lock")
        .push((level, message));
}

fn reset_mock() {
    rtapi_app_exit();
    unsafe {
        ARENA_OFFSET = 0;
        ptr::write_bytes(ptr::addr_of_mut!(ARENA.0).cast::<u8>(), 0, 32_768);
    }
    RECORDS.lock().expect("record lock").clear();
    *FUNCTION.lock().expect("function lock") = None;
    *PLAN.lock().expect("plan lock") = Plan::success();
    *CALLS.lock().expect("call lock") = Calls::new();
    MESSAGES.lock().expect("message lock").clear();
}

fn start_component() {
    reset_mock();
    assert_eq!(rtapi_app_main(), 0);
}

fn address(name: &str) -> usize {
    RECORDS
        .lock()
        .expect("record lock")
        .iter()
        .find(|record| record.name == name)
        .unwrap_or_else(|| panic!("missing HAL interface: {name}"))
        .address
}

fn set<T: Copy>(name: &str, value: T) {
    unsafe { ptr::write_volatile(address(name) as *mut T, value) }
}

fn get<T: Copy>(name: &str) -> T {
    unsafe { ptr::read_volatile(address(name) as *const T) }
}

fn callback() {
    let (function, argument) = FUNCTION
        .lock()
        .expect("function lock")
        .expect("realtime function was exported");
    let function: unsafe extern "C" fn(*mut c_void, c_long) = unsafe { mem::transmute(function) };
    unsafe { function(argument as *mut c_void, 1_000_000) };
}

fn set_valid_idle_interface() {
    set("h100-spindle.link-fault", false);
    set("h100-spindle.control-mode-f001", 2_u32);
    set("h100-spindle.frequency-source-f002", 2_u32);
    set("h100-spindle.reference-f004-centihz", 4_000_u32);
    set("h100-spindle.maximum-f005-centihz", 4_000_u32);
    set("h100-spindle.panel-stop-f024", 1_u32);
    set("h100-spindle.slave-address-f163", 1_u32);
    set("h100-spindle.baud-selector-f164", 2_u32);
    set("h100-spindle.data-mode-f165", 3_u32);
    set("h100-spindle.frequency-decimals-f169", 0_u32);
    set("h100-spindle.rated-rpm", 24_000.0_f64);
    set("h100-spindle.minimum-rpm", 6_000.0_f64);
    set("h100-spindle.maximum-rpm", 24_000.0_f64);
    set("h100-spindle.expected-reference-f004-centihz", 4_000_u32);
    set("h100-spindle.expected-maximum-f005-centihz", 4_000_u32);
    set("h100-spindle.at-speed-tolerance-hz", 1.0_f64);
}

fn add(schema: &mut BTreeSet<(String, Kind, u32)>, kind: Kind, direction: u32, names: &[&str]) {
    for name in names {
        assert!(schema.insert(((*name).to_string(), kind, direction)));
    }
}

fn expected_schema() -> BTreeSet<(String, Kind, u32)> {
    let input = hal::hal_pin_dir_t_HAL_IN as u32;
    let output = hal::hal_pin_dir_t_HAL_OUT as u32;
    let parameter = hal::hal_param_dir_t_HAL_RW as u32;
    let mut schema = BTreeSet::new();
    add(
        &mut schema,
        Kind::BitPin,
        input,
        &[
            "h100-spindle.machine-enabled",
            "h100-spindle.run-request",
            "h100-spindle.forward-request",
            "h100-spindle.reverse-request",
            "h100-spindle.reset",
            "h100-spindle.link-fault",
        ],
    );
    for index in 0..13 {
        add(
            &mut schema,
            Kind::BitPin,
            input,
            &[&format!("h100-spindle.command-disabled{index}")],
        );
    }
    add(
        &mut schema,
        Kind::FloatPin,
        input,
        &["h100-spindle.speed-command-rpm"],
    );
    add(
        &mut schema,
        Kind::U32Pin,
        input,
        &[
            "h100-spindle.control-mode-f001",
            "h100-spindle.frequency-source-f002",
            "h100-spindle.reference-f004-centihz",
            "h100-spindle.maximum-f005-centihz",
            "h100-spindle.lower-limit-f011-centihz",
            "h100-spindle.panel-stop-f024",
            "h100-spindle.slave-address-f163",
            "h100-spindle.baud-selector-f164",
            "h100-spindle.data-mode-f165",
            "h100-spindle.frequency-decimals-f169",
            "h100-spindle.output-frequency-decihz",
            "h100-spindle.current-fault",
            "h100-spindle.main-status",
            "h100-spindle.given-frequency-readback",
        ],
    );
    add(
        &mut schema,
        Kind::U32Pin,
        output,
        &[
            "h100-spindle.diagnostic-snapshot-generation",
            "h100-spindle.main-control",
            "h100-spindle.given-frequency",
            "h100-spindle.observed-current-fault",
            "h100-spindle.observed-main-status",
            "h100-spindle.fault-code",
            "h100-spindle.block-code",
            "h100-spindle.state",
        ],
    );
    add(
        &mut schema,
        Kind::BitPin,
        output,
        &[
            "h100-spindle.ready",
            "h100-spindle.running",
            "h100-spindle.forward-running",
            "h100-spindle.reverse-running",
            "h100-spindle.at-speed",
            "h100-spindle.fault-latched",
            "h100-spindle.fault-kind-unknown",
            "h100-spindle.block-kind-unknown",
            "h100-spindle.state-kind-unknown",
            "h100-spindle.vfd-fault-present",
            "h100-spindle.vfd-fault-known",
            "h100-spindle.vfd-fault-unknown",
            "h100-spindle.main-status-known",
            "h100-spindle.main-status-unknown",
            "h100-spindle.fault-data-b00",
            "h100-spindle.fault-data-b01",
            "h100-spindle.fault-data-b02",
            "h100-spindle.fault-data-b03",
            "h100-spindle.fault-data-b04",
            "h100-spindle.fault-data-b05",
            "h100-spindle.fault-data-b06",
            "h100-spindle.fault-data-b07",
            "h100-spindle.fault-data-b08",
            "h100-spindle.fault-data-b09",
        ],
    );
    for code in BlockCode::DIAGNOSTICS {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!("h100-spindle.fault-kind-{}", code.wire_code())],
        );
    }
    for code in BlockCode::ALL {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!("h100-spindle.block-kind-{}", code.wire_code())],
        );
    }
    for state in State::ALL {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!("h100-spindle.state-kind-{}", state.wire_code())],
        );
    }
    for family in VfdFaultFamily::ALL {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!(
                "h100-spindle.vfd-fault-family-{}",
                family.base_code()
            )],
        );
    }
    for phase in VfdFaultPhase::ALL {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!("h100-spindle.vfd-fault-phase-{}", phase.offset())],
        );
    }
    for status in MainStatusBit::ALL {
        add(
            &mut schema,
            Kind::BitPin,
            output,
            &[&format!(
                "h100-spindle.main-status-bit-{}",
                status.wire_code()
            )],
        );
    }
    add(
        &mut schema,
        Kind::S32Pin,
        output,
        &["h100-spindle.fault-data-s00"],
    );
    add(
        &mut schema,
        Kind::U32Pin,
        output,
        &[
            "h100-spindle.main-status-reserved-mask",
            "h100-spindle.fault-data-u00",
            "h100-spindle.fault-data-u01",
            "h100-spindle.fault-data-u02",
            "h100-spindle.fault-data-u03",
            "h100-spindle.fault-data-u04",
            "h100-spindle.fault-data-u05",
            "h100-spindle.fault-data-u06",
            "h100-spindle.fault-data-u07",
            "h100-spindle.fault-data-u08",
            "h100-spindle.fault-data-u09",
            "h100-spindle.fault-data-u10",
            "h100-spindle.fault-data-u11",
            "h100-spindle.fault-data-u12",
            "h100-spindle.fault-data-u13",
            "h100-spindle.fault-data-u14",
            "h100-spindle.fault-data-u15",
            "h100-spindle.fault-data-u16",
            "h100-spindle.fault-data-u17",
            "h100-spindle.fault-data-u18",
        ],
    );
    add(
        &mut schema,
        Kind::FloatPin,
        output,
        &[
            "h100-spindle.speed-feedback-rpm",
            "h100-spindle.target-frequency-hz",
            "h100-spindle.fault-data-f00",
            "h100-spindle.fault-data-f01",
            "h100-spindle.fault-data-f02",
            "h100-spindle.fault-data-f03",
            "h100-spindle.fault-data-f04",
            "h100-spindle.fault-data-f05",
        ],
    );
    add(
        &mut schema,
        Kind::FloatParameter,
        parameter,
        &[
            "h100-spindle.rated-rpm",
            "h100-spindle.minimum-rpm",
            "h100-spindle.maximum-rpm",
            "h100-spindle.at-speed-tolerance-hz",
        ],
    );
    add(
        &mut schema,
        Kind::U32Parameter,
        parameter,
        &[
            "h100-spindle.expected-reference-f004-centihz",
            "h100-spindle.expected-maximum-f005-centihz",
        ],
    );
    schema
}

#[test]
fn exact_hal_schema_and_initial_values_are_preserved() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    start_component();
    let records = RECORDS.lock().expect("record lock").clone();
    let actual = records
        .iter()
        .map(|record| (record.name.clone(), record.kind, record.direction))
        .collect::<BTreeSet<_>>();
    let addresses = records
        .iter()
        .map(|record| record.address)
        .collect::<BTreeSet<_>>();
    assert_eq!(records.len(), INTERFACE_COUNT);
    assert_eq!(actual, expected_schema());
    assert_eq!(addresses.len(), records.len());
    assert!(records
        .iter()
        .all(|record| record.name.len() <= hal::HAL_NAME_LEN as usize));

    assert!(get::<bool>("h100-spindle.link-fault"));
    assert_eq!(get::<u32>("h100-spindle.main-control"), CONTROL_STOP);
    assert_eq!(get::<u32>("h100-spindle.given-frequency"), 0);
    assert_eq!(get::<u32>("h100-spindle.diagnostic-snapshot-generation"), 0);
    assert_eq!(get::<u32>("h100-spindle.observed-current-fault"), 0);
    assert_eq!(get::<u32>("h100-spindle.observed-main-status"), 0);
    assert!(get::<bool>("h100-spindle.at-speed"));
    assert!(!get::<bool>("h100-spindle.ready"));
    assert!(!get::<bool>("h100-spindle.fault-latched"));
    assert_eq!(get::<u32>("h100-spindle.state"), State::Stopped as u32);
    for code in BlockCode::DIAGNOSTICS {
        assert!(!get::<bool>(&format!(
            "h100-spindle.fault-kind-{}",
            code.wire_code()
        )));
    }
    for code in BlockCode::ALL {
        assert_eq!(
            get::<bool>(&format!("h100-spindle.block-kind-{}", code.wire_code())),
            code == BlockCode::None
        );
    }
    for state in State::ALL {
        assert_eq!(
            get::<bool>(&format!("h100-spindle.state-kind-{}", state.wire_code())),
            state == State::Stopped
        );
    }
    assert!(!get::<bool>("h100-spindle.fault-kind-unknown"));
    assert!(!get::<bool>("h100-spindle.block-kind-unknown"));
    assert!(!get::<bool>("h100-spindle.state-kind-unknown"));
    assert!(!get::<bool>("h100-spindle.vfd-fault-present"));
    assert!(get::<bool>("h100-spindle.main-status-known"));
    assert!(!get::<bool>("h100-spindle.main-status-unknown"));
    assert_eq!(get::<u32>("h100-spindle.main-status-reserved-mask"), 0);
    for status in MainStatusBit::ALL {
        assert!(!get::<bool>(&format!(
            "h100-spindle.main-status-bit-{}",
            status.wire_code()
        )));
    }
    assert!(!get::<bool>("h100-spindle.fault-data-b00"));
    assert_eq!(get::<f64>("h100-spindle.rated-rpm"), 24_000.0);
    assert_eq!(get::<f64>("h100-spindle.minimum-rpm"), 0.0);
    assert_eq!(get::<f64>("h100-spindle.maximum-rpm"), 24_000.0);
    assert_eq!(
        get::<u32>("h100-spindle.expected-reference-f004-centihz"),
        0
    );
    assert_eq!(get::<f64>("h100-spindle.at-speed-tolerance-hz"), 1.0);
    rtapi_app_exit();
}

#[test]
fn hal_inputs_drive_the_exact_start_run_speed_change_and_stop_sequence() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    start_component();
    set_valid_idle_interface();
    callback();
    assert!(get::<bool>("h100-spindle.ready"));
    assert_eq!(get::<u32>("h100-spindle.state"), State::Stopped as u32);

    set("h100-spindle.machine-enabled", true);
    set("h100-spindle.run-request", true);
    set("h100-spindle.forward-request", true);
    set("h100-spindle.speed-command-rpm", 12_000.0_f64);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Arming as u32);
    assert_eq!(get::<u32>("h100-spindle.main-control"), CONTROL_STOP);
    assert_eq!(get::<u32>("h100-spindle.given-frequency"), 2_000);
    assert!(!get::<bool>("h100-spindle.at-speed"));

    set("h100-spindle.given-frequency-readback", 2_000_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Starting as u32);
    assert_eq!(get::<u32>("h100-spindle.main-control"), CONTROL_REVERSE);
    let clockwise_status = STATUS_IN_OPERATION
        | MainStatusBit::Operation.wire_code()
        | MainStatusBit::Reverse.wire_code();
    set("h100-spindle.main-status", clockwise_status);
    set("h100-spindle.output-frequency-decihz", 2_000_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Running as u32);
    assert!(get::<bool>("h100-spindle.main-status-known"));
    assert_eq!(
        get::<u32>("h100-spindle.observed-main-status"),
        clockwise_status
    );
    assert!(get::<bool>("h100-spindle.main-status-bit-4"));
    assert!(get::<bool>("h100-spindle.main-status-bit-8"));
    assert!(get::<bool>("h100-spindle.running"));
    assert!(get::<bool>("h100-spindle.forward-running"));
    assert!(get::<bool>("h100-spindle.at-speed"));
    assert_eq!(get::<f64>("h100-spindle.speed-feedback-rpm"), 12_000.0);

    set("h100-spindle.speed-command-rpm", 18_000.0_f64);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Starting as u32);
    assert_eq!(get::<u32>("h100-spindle.given-frequency"), 3_000);
    set("h100-spindle.given-frequency-readback", 3_000_u32);
    set("h100-spindle.output-frequency-decihz", 3_000_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Running as u32);

    set("h100-spindle.run-request", false);
    set("h100-spindle.forward-request", false);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Stopping as u32);
    assert_eq!(get::<u32>("h100-spindle.main-control"), CONTROL_STOP);
    assert_eq!(get::<u32>("h100-spindle.given-frequency"), 3_000);
    set("h100-spindle.main-status", 0_u32);
    set("h100-spindle.output-frequency-decihz", 0_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.state"), State::Stopped as u32);
    assert_eq!(get::<u32>("h100-spindle.given-frequency"), 0);
    rtapi_app_exit();
}

#[test]
fn idle_block_publishes_the_evidence_that_identifies_it() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    start_component();
    set_valid_idle_interface();
    set("h100-spindle.link-fault", true);
    callback();

    assert!(!get::<bool>("h100-spindle.fault-latched"));
    assert_eq!(
        get::<u32>("h100-spindle.block-code"),
        BlockCode::Link.wire_code()
    );
    assert!(get::<bool>("h100-spindle.fault-data-b00"));
    assert!(get::<bool>("h100-spindle.fault-data-b08"));
    assert_eq!(
        get::<u32>("h100-spindle.fault-data-u02"),
        2,
        "the block evidence must be the current input snapshot"
    );
    rtapi_app_exit();
}

#[test]
fn every_h100_main_status_bit_is_named_and_reserved_bits_remain_explicitly_unknown() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    start_component();
    set_valid_idle_interface();

    let all_known = MainStatusBit::KNOWN_MASK;
    set("h100-spindle.main-status", all_known);
    callback();
    assert!(get::<bool>("h100-spindle.main-status-known"));
    assert!(!get::<bool>("h100-spindle.main-status-unknown"));
    assert_eq!(get::<u32>("h100-spindle.main-status-reserved-mask"), 0);
    for status in MainStatusBit::ALL {
        assert!(get::<bool>(&format!(
            "h100-spindle.main-status-bit-{}",
            status.wire_code()
        )));
    }

    set("h100-spindle.main-status", 0x8108_u32);
    callback();
    assert!(!get::<bool>("h100-spindle.main-status-known"));
    assert!(get::<bool>("h100-spindle.main-status-unknown"));
    assert_eq!(get::<u32>("h100-spindle.main-status-reserved-mask"), 0x8100);
    assert!(get::<bool>("h100-spindle.main-status-bit-8"));
    for status in MainStatusBit::ALL {
        assert_eq!(
            get::<bool>(&format!(
                "h100-spindle.main-status-bit-{}",
                status.wire_code()
            )),
            0x0008 & status.wire_code() != 0
        );
    }
    rtapi_app_exit();
}

#[test]
fn every_modbus_disabled_input_blocks_and_faults_the_run_request() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    for index in 0..13 {
        start_component();
        set_valid_idle_interface();
        callback();
        let name = format!("h100-spindle.command-disabled{index}");
        set(&name, true);
        callback();
        assert!(!get::<bool>("h100-spindle.ready"));
        assert_eq!(
            get::<u32>("h100-spindle.block-code"),
            BlockCode::CommandDisabled as u32
        );
        set("h100-spindle.machine-enabled", true);
        set("h100-spindle.run-request", true);
        set("h100-spindle.forward-request", true);
        set("h100-spindle.speed-command-rpm", 12_000.0_f64);
        callback();
        assert!(get::<bool>("h100-spindle.fault-latched"));
        assert_eq!(
            get::<u32>("h100-spindle.fault-code"),
            BlockCode::CommandDisabled as u32
        );
        assert!(get::<bool>(&format!(
            "h100-spindle.fault-kind-{}",
            BlockCode::CommandDisabled.wire_code()
        )));
        assert!(get::<bool>(&format!(
            "h100-spindle.block-kind-{}",
            BlockCode::CommandDisabled.wire_code()
        )));
        assert!(get::<bool>("h100-spindle.fault-data-b00"));
        assert!(get::<bool>("h100-spindle.fault-data-b09"));
        assert!(get::<bool>("h100-spindle.fault-data-b03"));
        assert!(get::<bool>("h100-spindle.fault-data-b04"));
        assert_eq!(get::<f64>("h100-spindle.fault-data-f00"), 12_000.0);
        assert_eq!(get::<u32>("h100-spindle.main-control"), CONTROL_STOP);
        rtapi_app_exit();
    }
}

#[test]
fn h100_current_fault_is_exposed_as_source_named_family_phase_or_explicit_unknown() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    start_component();
    set_valid_idle_interface();

    set("h100-spindle.current-fault", 65_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.diagnostic-snapshot-generation"), 2);
    assert_eq!(get::<u32>("h100-spindle.observed-current-fault"), 65);
    assert!(get::<bool>("h100-spindle.vfd-fault-present"));
    assert!(get::<bool>("h100-spindle.vfd-fault-known"));
    assert!(!get::<bool>("h100-spindle.vfd-fault-unknown"));
    assert!(get::<bool>("h100-spindle.vfd-fault-family-64"));
    assert!(get::<bool>("h100-spindle.vfd-fault-phase-1"));
    assert!(get::<bool>(&format!(
        "h100-spindle.block-kind-{}",
        BlockCode::VfdFault.wire_code()
    )));

    set("h100-spindle.current-fault", 7_u32);
    callback();
    assert_eq!(get::<u32>("h100-spindle.diagnostic-snapshot-generation"), 4);
    assert_eq!(get::<u32>("h100-spindle.observed-current-fault"), 7);
    assert!(get::<bool>("h100-spindle.vfd-fault-present"));
    assert!(!get::<bool>("h100-spindle.vfd-fault-known"));
    assert!(get::<bool>("h100-spindle.vfd-fault-unknown"));
    for family in VfdFaultFamily::ALL {
        assert!(!get::<bool>(&format!(
            "h100-spindle.vfd-fault-family-{}",
            family.base_code()
        )));
    }
    for phase in VfdFaultPhase::ALL {
        assert!(!get::<bool>(&format!(
            "h100-spindle.vfd-fault-phase-{}",
            phase.offset()
        )));
    }
    rtapi_app_exit();
}

#[test]
fn every_lifecycle_failure_is_returned_and_cleaned_up() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    for (init_result, expected) in [(-19, -19), (0, EINVAL)] {
        reset_mock();
        PLAN.lock().expect("plan lock").init_result = init_result;
        assert_eq!(rtapi_app_main(), expected);
        assert_eq!(
            *CALLS.lock().expect("call lock"),
            Calls {
                init: 1,
                ..Calls::new()
            }
        );
    }

    reset_mock();
    PLAN.lock().expect("plan lock").malloc_fails = true;
    assert_eq!(rtapi_app_main(), ENOMEM);
    assert_eq!(CALLS.lock().expect("call lock").exit, 1);

    for failure_call in 0..INTERFACE_COUNT {
        reset_mock();
        let error = -1_000 - failure_call as c_int;
        PLAN.lock().expect("plan lock").registration_failure = Some((failure_call, error));
        assert_eq!(rtapi_app_main(), error);
        let calls = *CALLS.lock().expect("call lock");
        assert_eq!(calls.registration, failure_call + 1);
        assert_eq!(calls.export, 0);
        assert_eq!(calls.ready, 0);
        assert_eq!(calls.exit, 1);
    }

    reset_mock();
    PLAN.lock().expect("plan lock").registration_failure = Some((0, 7));
    assert_eq!(rtapi_app_main(), EINVAL);
    assert_eq!(CALLS.lock().expect("call lock").registration, 1);
    assert_eq!(CALLS.lock().expect("call lock").exit, 1);

    for (export_result, ready_result, expected) in [(-70, 0, -70), (0, -71, -71)] {
        reset_mock();
        PLAN.lock().expect("plan lock").export_result = export_result;
        PLAN.lock().expect("plan lock").ready_result = ready_result;
        assert_eq!(rtapi_app_main(), expected);
        assert_eq!(CALLS.lock().expect("call lock").exit, 1);
    }

    for (export_result, ready_result) in [(7, 0), (0, 7)] {
        reset_mock();
        PLAN.lock().expect("plan lock").export_result = export_result;
        PLAN.lock().expect("plan lock").ready_result = ready_result;
        assert_eq!(rtapi_app_main(), EINVAL);
        assert_eq!(CALLS.lock().expect("call lock").exit, 1);
    }

    start_component();
    PLAN.lock().expect("plan lock").exit_result = -72;
    rtapi_app_exit();
    assert_eq!(
        MESSAGES.lock().expect("message lock").as_slice(),
        &[(
            hal::msg_level_t_RTAPI_MSG_ERR,
            "h100_spindle: %s (raw=%d, call=%s): %s; action: %s\n".to_string()
        )]
    );
}
