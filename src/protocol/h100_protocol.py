"""Pure H100 Modbus RTU framing and command validation.

This module performs no I/O.  It exists so register values and complete RTU
frames can be verified without opening HAL, the Mesa card, or the VFD link.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation


DEFAULT_SLAVE_ADDRESS = 1

# H100 holding-register addresses (manual, printed page 85).
REGISTER_MAIN_CONTROL = 0x0200
REGISTER_GIVEN_FREQUENCY = 0x0201
REGISTER_MAIN_STATUS = 0x0210

# H100 parameter addresses are their decimal F-number expressed in hex.
PARAMETER_CONTROL_MODE = 0x0001  # F001
PARAMETER_FREQUENCY_SOURCE = 0x0002  # F002
PARAMETER_REFERENCE_FREQUENCY = 0x0004  # F004, 0.1 Hz units on this H100
PARAMETER_MAXIMUM_FREQUENCY = 0x0005  # F005, 0.1 Hz units on this H100
PARAMETER_ACCELERATION_TIME = 0x000E  # F014, 0.1 second units
PARAMETER_DECELERATION_TIME = 0x000F  # F015, 0.1 second units
PARAMETER_PANEL_STOP_ENABLE = 0x0018  # F024
PARAMETER_COMMUNICATION_ADDRESS = 0x00A3  # F163
PARAMETER_COMMUNICATION_BAUD = 0x00A4  # F164
PARAMETER_COMMUNICATION_MODE = 0x00A5  # F165
PARAMETER_FREQUENCY_DECIMALS = 0x00A9  # F169

# H100 input-register addresses (manual, printed page 84).
INPUT_OUTPUT_FREQUENCY = 0x0000
INPUT_SET_FREQUENCY = 0x0001
INPUT_OUTPUT_CURRENT = 0x0002
INPUT_OUTPUT_SPEED = 0x0003
INPUT_DC_VOLTAGE = 0x0004
INPUT_AC_VOLTAGE = 0x0005
INPUT_TEMPERATURE = 0x0006
INPUT_CURRENT_FAULT = 0x000A
INPUT_OUTPUT_POWER = 0x000C

# 0200H maps bits 0..7 to parameter-address commands 0048H..004FH.
CONTROL_RUN = 0x0001
CONTROL_FORWARD = 0x0002
CONTROL_REVERSE = 0x0004
CONTROL_STOP = 0x0008
CONTROL_FORWARD_REVERSE_SWITCH = 0x0010
CONTROL_JOG = 0x0020
CONTROL_JOG_FORWARD = 0x0040
CONTROL_JOG_REVERSE = 0x0080

# Required selector values for control through the communication interface.
CONTROL_MODE_COMMUNICATION = 2  # F001=2
FREQUENCY_SOURCE_COMMUNICATION = 2  # F002=2

FUNCTION_READ_HOLDING_REGISTERS = 0x03
FUNCTION_READ_INPUT_REGISTERS = 0x04
FUNCTION_WRITE_SINGLE_REGISTER = 0x06


class H100ProtocolError(ValueError):
    """Raised when a value or RTU frame is invalid for this controller."""


@dataclass(frozen=True)
class H100FaultDiagnostic:
    raw: int
    name: str
    drive_display: str | None
    summary: str
    action: str
    source_known: bool

    def __post_init__(self) -> None:
        if not isinstance(self.raw, int) or isinstance(self.raw, bool):
            raise TypeError("diagnostic raw value must be an integer")
        known_identity = re.fullmatch(r"[A-Z][A-Z0-9_]*", self.name)
        unknown_identity = re.fullmatch(
            r"UNKNOWN_[A-Z][A-Z0-9_]*\(raw=(-?\d+)\)", self.name
        )
        if self.source_known:
            if known_identity is None:
                raise ValueError("known diagnostic identity is not canonical")
        elif unknown_identity is None or int(unknown_identity.group(1)) != self.raw:
            raise ValueError(
                "unknown diagnostic identity must retain its exact raw value"
            )
        if not self.summary:
            raise ValueError("diagnostic cause must be present")
        if not self.action:
            raise ValueError("diagnostic action must be present")
        if self.drive_display == "":
            raise ValueError("drive display must be absent or nonempty")

    def operator_text(self) -> str:
        display = (
            "" if self.drive_display is None else f", drive_display={self.drive_display}"
        )
        raw = "" if not self.source_known else f" (raw={self.raw}{display})"
        return (
            f"{self.name}{raw}: {self.summary}; "
            f"action: {self.action}"
        )


# Exact input-register 000A table from H100 manual V1.8, printed page 84.
# Each listed base is followed by the four source-defined suffixes S/A/d/n.
_H100_FAULT_FAMILIES = (
    (64, "E.OC"),
    (80, "E.oU"),
    (88, "E.Lu"),
    (92, "E.oH"),
    (96, "E.oL"),
    (100, "E.oA"),
    (104, "E.oT"),
)
_H100_FAULT_SUFFIXES = ("S", "A", "d", "n")
_H100_FAULT_PHASE_CONTEXT = {
    "S": "at stop",
    "A": "during acceleration",
    "d": "during deceleration",
    "n": "at constant speed",
}
_H100_FAULT_CAUSE = {
    "E.OC": "detected over-current",
    "E.oU": "detected over-voltage",
    "E.Lu": "detected low input voltage",
    "E.oH": "inverter overheated",
    "E.oL": "inverter overload protection tripped",
    "E.oA": "motor-overload protection tripped",
    "E.oT": "detected motor over-torque",
}
_H100_FAULT_ACTION = {
    "E.oU": (
        "keep the spindle stopped; check input voltage for abnormal changes and "
        "lengthen deceleration or verify the specified braking provision before reset"
    ),
    "E.Lu": (
        "keep the spindle stopped; verify input voltage, supply continuity, and "
        "any sudden load change before reset"
    ),
    "E.oH": (
        "keep the spindle stopped; clear blocked cooling airflow or fins, verify fan "
        "operation, ambient temperature, and ventilation, and allow the drive to "
        "cool before reset"
    ),
    "E.oL": (
        "keep the spindle stopped; check for a jammed mechanical load, verify drive "
        "capacity, and correct the V/F configuration before reset"
    ),
    "E.oA": (
        "keep the spindle stopped; check for sudden or excessive mechanical load, "
        "verify motor sizing and condition, and inspect supply-voltage stability "
        "before reset"
    ),
    "E.oT": (
        "keep the spindle stopped; inspect the mechanical load for a jam or sudden "
        "torque change and verify that the motor is correctly sized before reset"
    ),
}


def _over_current_action(suffix: str) -> str:
    if suffix == "A":
        return (
            "keep the spindle stopped; check motor/output wiring for shorts and "
            "insulation failure, check load and drive sizing, and lengthen "
            "acceleration before reset"
        )
    if suffix == "n":
        return (
            "keep the spindle stopped; check motor/output wiring, a blocked spindle "
            "or sudden load change, drive sizing, and supply-voltage changes before reset"
        )
    return (
        "keep the spindle stopped; check motor/output wiring for shorts and insulation "
        "failure, lengthen deceleration, and check drive sizing and DC-braking "
        "settings before reset"
    )


def decode_h100_fault(raw: int) -> H100FaultDiagnostic:
    """Describe a current-fault register without inventing unknown meanings."""

    if not isinstance(raw, int) or isinstance(raw, bool) or not 0 <= raw <= 0xFFFF:
        raise H100ProtocolError("H100 current-fault value must fit in 16 bits")
    for base, family in _H100_FAULT_FAMILIES:
        offset = raw - base
        if 0 <= offset < len(_H100_FAULT_SUFFIXES):
            display = f"{family}.{_H100_FAULT_SUFFIXES[offset]}"
            family_identity = family.replace(".", "_").upper()
            phase_identity = _H100_FAULT_SUFFIXES[offset].upper()
            return H100FaultDiagnostic(
                raw=raw,
                name=f"H100_{family_identity}_{phase_identity}",
                drive_display=display,
                summary=(
                    f"the H100 {_H100_FAULT_CAUSE[family]} "
                    f"{_H100_FAULT_PHASE_CONTEXT[_H100_FAULT_SUFFIXES[offset]]}"
                ),
                action=(
                    _over_current_action(_H100_FAULT_SUFFIXES[offset])
                    if family == "E.OC"
                    else _H100_FAULT_ACTION[family]
                ),
                source_known=True,
            )
    return H100FaultDiagnostic(
        raw=raw,
        name=f"UNKNOWN_H100_VFD_CURRENT_FAULT(raw={raw})",
        drive_display=None,
        summary="value is absent from the verified H100 manual V1.8 fault-code table",
        action="retain the raw value and consult the drive manufacturer before reset",
        source_known=False,
    )


# Exact H100 V1.8 abnormal-response codes from printed page 90.
_H100_MODBUS_EXCEPTIONS = {
    0x01: (
        "H100_MODBUS_FUNCTION_UNSUPPORTED",
        "the H100 cannot process the requested Modbus function",
        "verify that the request uses a function supported by H100 V1.8 and the correct frame format",
    ),
    0x02: (
        "H100_MODBUS_DATA_ADDRESS_INVALID",
        "the H100 rejected the requested Modbus data address",
        "verify the exact H100 register address and requested span before retrying",
    ),
    0x03: (
        "H100_MODBUS_DATA_VALUE_OUT_OF_RANGE",
        "the H100 rejected one or more Modbus data values as out of range",
        "verify every transmitted value against the target H100 register range before retrying",
    ),
    0x04: (
        "H100_MODBUS_OPERATION_FAILED",
        "the H100 could not perform the operation, such as writing a read-only or run-locked parameter",
        "keep the spindle stopped and verify register writability and required drive state before retrying",
    ),
}


def decode_h100_modbus_exception(raw: int) -> H100FaultDiagnostic:
    """Describe an H100 abnormal-response byte without a separate code lookup."""

    if not isinstance(raw, int) or isinstance(raw, bool) or not 0 <= raw <= 0xFF:
        raise H100ProtocolError("H100 Modbus exception value must fit in 8 bits")
    known = _H100_MODBUS_EXCEPTIONS.get(raw)
    if known is None:
        return H100FaultDiagnostic(
            raw=raw,
            name=f"UNKNOWN_H100_MODBUS_EXCEPTION(raw={raw})",
            drive_display=None,
            summary="value is absent from the verified H100 V1.8 abnormal-response table",
            action="retain the raw response frame and verify the exact drive manual and firmware before retrying",
            source_known=False,
        )
    name, summary, action = known
    return H100FaultDiagnostic(
        raw=raw,
        name=name,
        drive_display=None,
        summary=summary,
        action=action,
        source_known=True,
    )


def crc16_modbus(data: bytes) -> int:
    """Return standard Modbus CRC-16 as a host integer."""

    crc = 0xFFFF
    for value in data:
        crc ^= value
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc


def append_crc(payload: bytes) -> bytes:
    crc = crc16_modbus(payload)
    return payload + bytes((crc & 0xFF, (crc >> 8) & 0xFF))


def validate_slave_address(slave_address: int) -> None:
    if not 1 <= slave_address <= 247:
        raise H100ProtocolError("Modbus slave address must be in 1..247")


def write_single_register_request(
    address: int,
    value: int,
    *,
    slave_address: int = DEFAULT_SLAVE_ADDRESS,
) -> bytes:
    validate_slave_address(slave_address)
    if not 0 <= address <= 0xFFFF:
        raise H100ProtocolError("register address must fit in 16 bits")
    if not 0 <= value <= 0xFFFF:
        raise H100ProtocolError("register value must fit in 16 bits")
    payload = bytes(
        (
            slave_address,
            FUNCTION_WRITE_SINGLE_REGISTER,
            address >> 8,
            address & 0xFF,
            value >> 8,
            value & 0xFF,
        )
    )
    return append_crc(payload)


def read_registers_request(
    function: int,
    address: int,
    count: int,
    *,
    slave_address: int = DEFAULT_SLAVE_ADDRESS,
) -> bytes:
    validate_slave_address(slave_address)
    if function not in {
        FUNCTION_READ_HOLDING_REGISTERS,
        FUNCTION_READ_INPUT_REGISTERS,
    }:
        raise H100ProtocolError("function must be 03H or 04H")
    if not 0 <= address <= 0xFFFF:
        raise H100ProtocolError("register address must fit in 16 bits")
    if not 1 <= count <= 20:
        raise H100ProtocolError("H100 manual limits a read to 1..20 registers")
    payload = bytes(
        (
            slave_address,
            function,
            address >> 8,
            address & 0xFF,
            count >> 8,
            count & 0xFF,
        )
    )
    return append_crc(payload)


def verify_crc(frame: bytes) -> bool:
    if len(frame) < 4:
        return False
    expected = crc16_modbus(frame[:-2])
    received = frame[-2] | (frame[-1] << 8)
    return received == expected


def parse_register_response(
    frame: bytes,
    *,
    function: int,
    slave_address: int = DEFAULT_SLAVE_ADDRESS,
) -> tuple[int, ...]:
    """Validate and decode a function-03 or function-04 response."""

    validate_slave_address(slave_address)
    if function not in {
        FUNCTION_READ_HOLDING_REGISTERS,
        FUNCTION_READ_INPUT_REGISTERS,
    }:
        raise H100ProtocolError("function must be 03H or 04H")
    if len(frame) < 5 or not verify_crc(frame):
        raise H100ProtocolError("invalid or missing Modbus CRC")
    if frame[0] != slave_address:
        raise H100ProtocolError("response slave address does not match request")
    if frame[1] == (function | 0x80):
        if len(frame) != 5:
            raise H100ProtocolError("malformed Modbus exception response")
        raise H100ProtocolError(decode_h100_modbus_exception(frame[2]).operator_text())
    if frame[1] != function:
        raise H100ProtocolError("response function does not match request")
    byte_count = frame[2]
    if byte_count == 0 or byte_count % 2 or len(frame) != byte_count + 5:
        raise H100ProtocolError("invalid register-response byte count")
    return tuple(
        (frame[index] << 8) | frame[index + 1]
        for index in range(3, 3 + byte_count, 2)
    )


def encode_frequency_hz(frequency_hz: object, *, f169: int) -> int:
    """Encode hertz exactly according to the VFD's observed F169 value."""

    if f169 not in (0, 1):
        raise H100ProtocolError("F169 must be observed as 0 or 1 before encoding")
    try:
        frequency = Decimal(str(frequency_hz))
    except (InvalidOperation, ValueError) as error:
        raise H100ProtocolError("frequency must be a decimal number") from error
    if not frequency.is_finite() or frequency < 0:
        raise H100ProtocolError("frequency must be finite and non-negative")
    units_per_hz = Decimal(10 if f169 == 0 else 100)
    raw = frequency * units_per_hz
    if raw != raw.to_integral_value():
        decimals = 1 if f169 == 0 else 2
        raise H100ProtocolError(
            f"frequency must be exactly representable with {decimals} decimal place(s)"
        )
    value = int(raw)
    if value > 0xFFFF:
        raise H100ProtocolError("encoded frequency exceeds the 16-bit register")
    return value


@dataclass(frozen=True)
class SpindleDecision:
    control_word: int
    frequency_register: int
    run_permitted: bool
    reason: str


def decide_forward_command(
    *,
    run_requested: bool,
    requested_frequency_hz: object,
    maximum_frequency_hz: object | None,
    f001: int,
    f002: int,
    f005_hundredths_hz: int,
    f024: int,
    f169: int,
    link_fault: bool,
    vfd_fault_code: int,
) -> SpindleDecision:
    """Return a forward-only command; every unresolved state returns STOP."""

    stop = SpindleDecision(CONTROL_STOP, 0, False, "stop requested")
    if not run_requested:
        return stop
    if link_fault:
        return SpindleDecision(CONTROL_STOP, 0, False, "Modbus link fault")
    if vfd_fault_code != 0:
        return SpindleDecision(
            CONTROL_STOP,
            0,
            False,
            decode_h100_fault(vfd_fault_code).operator_text(),
        )
    if f001 != CONTROL_MODE_COMMUNICATION:
        return SpindleDecision(CONTROL_STOP, 0, False, "F001 is not 2")
    if f002 != FREQUENCY_SOURCE_COMMUNICATION:
        return SpindleDecision(CONTROL_STOP, 0, False, "F002 is not 2")
    if f024 != 1:
        return SpindleDecision(CONTROL_STOP, 0, False, "F024 is not 1")
    if f005_hundredths_hz <= 0:
        return SpindleDecision(CONTROL_STOP, 0, False, "F005 is not a valid limit")
    if maximum_frequency_hz is None:
        return SpindleDecision(
            CONTROL_STOP, 0, False, "maximum spindle frequency is not configured"
        )

    try:
        requested = Decimal(str(requested_frequency_hz))
        maximum = Decimal(str(maximum_frequency_hz))
    except (InvalidOperation, ValueError) as error:
        raise H100ProtocolError(
            "requested and maximum frequencies must be decimal numbers"
        ) from error
    if not requested.is_finite() or not maximum.is_finite():
        raise H100ProtocolError("requested and maximum frequencies must be finite")
    if maximum <= 0:
        raise H100ProtocolError("maximum spindle frequency must be positive")
    # The installed four-digit H100 transfers F004/F005 in 0.1 Hz units.
    # The legacy argument name is retained because the live HAL pin name is
    # already part of the installed configuration.
    configured_vfd_maximum = Decimal(f005_hundredths_hz) / Decimal(10)
    if maximum > configured_vfd_maximum:
        return SpindleDecision(
            CONTROL_STOP,
            0,
            False,
            "explicit maximum exceeds the H100 F005 limit",
        )
    if requested <= 0:
        return SpindleDecision(
            CONTROL_STOP, 0, False, "run frequency must be greater than zero"
        )
    if requested > maximum:
        return SpindleDecision(
            CONTROL_STOP, 0, False, "requested frequency exceeds configured maximum"
        )

    encoded = encode_frequency_hz(requested, f169=f169)
    # Use the explicit Forward command. CONTROL_RUN leaves the H100's previous
    # direction latched, so it cannot reliably reverse a prior reverse run.
    return SpindleDecision(CONTROL_FORWARD, encoded, True, "forward run permitted")
