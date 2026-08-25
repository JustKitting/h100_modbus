"""Pure H100 Modbus RTU framing and command validation.

This module performs no I/O.  It exists so register values and complete RTU
frames can be verified without opening HAL, the Mesa card, or the VFD link.
"""

from __future__ import annotations

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
        raise H100ProtocolError(f"H100 returned Modbus exception {frame[2]}")
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
            CONTROL_STOP, 0, False, f"H100 fault code {vfd_fault_code}"
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
    if requested > configured_vfd_maximum:
        return SpindleDecision(
            CONTROL_STOP, 0, False, "requested frequency exceeds the H100 F005 limit"
        )

    encoded = encode_frequency_hz(requested, f169=f169)
    return SpindleDecision(CONTROL_RUN, encoded, True, "forward run permitted")
