from __future__ import annotations

import sys
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PROTOCOL_DIR = PROJECT_ROOT / "src" / "protocol"
if str(PROTOCOL_DIR) not in sys.path:
    sys.path.insert(0, str(PROTOCOL_DIR))

from h100_protocol import (
    CONTROL_RUN,
    CONTROL_STOP,
    FUNCTION_READ_HOLDING_REGISTERS,
    FUNCTION_READ_INPUT_REGISTERS,
    H100ProtocolError,
    REGISTER_GIVEN_FREQUENCY,
    append_crc,
    crc16_modbus,
    decide_forward_command,
    encode_frequency_hz,
    parse_register_response,
    read_registers_request,
    validate_slave_address,
    verify_crc,
    write_single_register_request,
)


class H100ProtocolTests(unittest.TestCase):
    @staticmethod
    def decision_arguments(**changes):
        arguments = {
            "run_requested": True,
            "requested_frequency_hz": "30.0",
            "maximum_frequency_hz": "400.0",
            "f001": 2,
            "f002": 2,
            "f005_hundredths_hz": 4000,
            "f024": 1,
            "f169": 0,
            "link_fault": False,
            "vfd_fault_code": 0,
        }
        arguments.update(changes)
        return arguments

    def test_known_read_request_matches_successful_capture(self) -> None:
        request = read_registers_request(
            FUNCTION_READ_INPUT_REGISTERS, 0x0000, 1
        )
        self.assertEqual(request.hex(" ").upper(), "01 04 00 00 00 01 31 CA")

    def test_manual_300_hz_write_example_matches_exact_frame(self) -> None:
        request = write_single_register_request(REGISTER_GIVEN_FREQUENCY, 3000)
        self.assertEqual(request.hex(" ").upper(), "01 06 02 01 0B B8 DE F0")

    def test_successful_zero_frequency_capture_decodes(self) -> None:
        response = bytes.fromhex("01 04 02 00 00 B9 30")
        self.assertTrue(verify_crc(response))
        self.assertEqual(
            parse_register_response(
                response, function=FUNCTION_READ_INPUT_REGISTERS
            ),
            (0,),
        )

    def test_frequency_units_are_controlled_by_f169(self) -> None:
        self.assertEqual(encode_frequency_hz("30.0", f169=0), 300)
        self.assertEqual(encode_frequency_hz("30.00", f169=1), 3000)
        with self.assertRaises(H100ProtocolError):
            encode_frequency_hz("30.01", f169=0)

    def test_run_is_refused_until_configuration_is_known(self) -> None:
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz="30.0",
            maximum_frequency_hz=None,
            f001=2,
            f002=2,
            f005_hundredths_hz=4000,
            f024=1,
            f169=0,
            link_fault=False,
            vfd_fault_code=0,
        )
        self.assertEqual(decision.control_word, CONTROL_STOP)
        self.assertFalse(decision.run_permitted)

    def test_verified_configuration_produces_forward_run(self) -> None:
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz="30.0",
            maximum_frequency_hz="400.0",
            f001=2,
            f002=2,
            f005_hundredths_hz=4000,
            f024=1,
            f169=0,
            link_fault=False,
            vfd_fault_code=0,
        )
        self.assertEqual(decision.control_word, CONTROL_RUN)
        self.assertEqual(decision.frequency_register, 300)
        self.assertTrue(decision.run_permitted)

    def test_fault_forces_stop_and_zero_frequency(self) -> None:
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz="30.0",
            maximum_frequency_hz="400.0",
            f001=2,
            f002=2,
            f005_hundredths_hz=4000,
            f024=1,
            f169=0,
            link_fault=False,
            vfd_fault_code=64,
        )
        self.assertEqual(decision.control_word, CONTROL_STOP)
        self.assertEqual(decision.frequency_register, 0)
        self.assertFalse(decision.run_permitted)

    def test_nonfinite_frequency_is_rejected(self) -> None:
        with self.assertRaises(H100ProtocolError):
            decide_forward_command(
                run_requested=True,
                requested_frequency_hz="NaN",
                maximum_frequency_hz="400.0",
                f001=2,
                f002=2,
                f005_hundredths_hz=4000,
                f024=1,
                f169=0,
                link_fault=False,
                vfd_fault_code=0,
            )

    def test_vfd_f005_limit_is_enforced(self) -> None:
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz="60.0",
            maximum_frequency_hz="400.0",
            f001=2,
            f002=2,
            f005_hundredths_hz=500,
            f024=1,
            f169=0,
            link_fault=False,
            vfd_fault_code=0,
        )
        self.assertEqual(decision.control_word, CONTROL_STOP)
        self.assertFalse(decision.run_permitted)

    def test_disabled_panel_stop_refuses_run(self) -> None:
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz="30.0",
            maximum_frequency_hz="400.0",
            f001=2,
            f002=2,
            f005_hundredths_hz=4000,
            f024=0,
            f169=0,
            link_fault=False,
            vfd_fault_code=0,
        )
        self.assertEqual(decision.control_word, CONTROL_STOP)
        self.assertFalse(decision.run_permitted)

    def test_crc_primitives_cover_empty_and_corrupted_frames(self) -> None:
        self.assertEqual(crc16_modbus(b"123456789"), 0x4B37)
        frame = append_crc(b"\x01\x04\x00")
        self.assertTrue(verify_crc(frame))
        self.assertFalse(verify_crc(b""))
        self.assertFalse(verify_crc(b"\x01\x02\x03"))
        self.assertFalse(verify_crc(frame[:-1] + bytes((frame[-1] ^ 1,))))

    def test_slave_address_range_is_exact(self) -> None:
        validate_slave_address(1)
        validate_slave_address(247)
        for address in (0, 248, -1, 65535):
            with self.subTest(address=address):
                with self.assertRaisesRegex(H100ProtocolError, "1..247"):
                    validate_slave_address(address)

    def test_write_request_rejects_every_out_of_range_field(self) -> None:
        for address in (-1, 0x10000):
            with self.subTest(address=address):
                with self.assertRaisesRegex(H100ProtocolError, "address"):
                    write_single_register_request(address, 0)
        for value in (-1, 0x10000):
            with self.subTest(value=value):
                with self.assertRaisesRegex(H100ProtocolError, "value"):
                    write_single_register_request(0, value)
        request = write_single_register_request(0xFFFF, 0xFFFF, slave_address=247)
        self.assertTrue(verify_crc(request))

    def test_read_request_rejects_function_address_and_count_errors(self) -> None:
        with self.assertRaisesRegex(H100ProtocolError, "03H or 04H"):
            read_registers_request(0x06, 0, 1)
        for address in (-1, 0x10000):
            with self.subTest(address=address):
                with self.assertRaisesRegex(H100ProtocolError, "address"):
                    read_registers_request(
                        FUNCTION_READ_HOLDING_REGISTERS, address, 1
                    )
        for count in (0, 21):
            with self.subTest(count=count):
                with self.assertRaisesRegex(H100ProtocolError, "1..20"):
                    read_registers_request(
                        FUNCTION_READ_HOLDING_REGISTERS, 0, count
                    )
        for function in (
            FUNCTION_READ_HOLDING_REGISTERS,
            FUNCTION_READ_INPUT_REGISTERS,
        ):
            request = read_registers_request(function, 0xFFFF, 20, slave_address=247)
            self.assertTrue(verify_crc(request))

    def test_response_parser_rejects_every_frame_contract_violation(self) -> None:
        with self.assertRaisesRegex(H100ProtocolError, "03H or 04H"):
            parse_register_response(b"", function=0x06)
        for frame in (b"", b"\x01\x04\x00\x00", b"\x01\x04\x00\x00\x00"):
            with self.subTest(frame=frame):
                with self.assertRaisesRegex(H100ProtocolError, "CRC"):
                    parse_register_response(
                        frame, function=FUNCTION_READ_INPUT_REGISTERS
                    )

        wrong_slave = append_crc(b"\x02\x04\x02\x00\x00")
        with self.assertRaisesRegex(H100ProtocolError, "slave address"):
            parse_register_response(
                wrong_slave, function=FUNCTION_READ_INPUT_REGISTERS
            )

        malformed_exception = append_crc(b"\x01\x84\x02\x00")
        with self.assertRaisesRegex(H100ProtocolError, "malformed"):
            parse_register_response(
                malformed_exception, function=FUNCTION_READ_INPUT_REGISTERS
            )
        exception = append_crc(b"\x01\x84\x02")
        with self.assertRaisesRegex(H100ProtocolError, "exception 2"):
            parse_register_response(
                exception, function=FUNCTION_READ_INPUT_REGISTERS
            )

        wrong_function = append_crc(b"\x01\x03\x02\x00\x00")
        with self.assertRaisesRegex(H100ProtocolError, "function"):
            parse_register_response(
                wrong_function, function=FUNCTION_READ_INPUT_REGISTERS
            )

        invalid_payloads = (
            b"\x01\x04\x00",
            b"\x01\x04\x01\x00",
            b"\x01\x04\x02\x00",
            b"\x01\x04\x02\x00\x00\x00",
        )
        for payload in invalid_payloads:
            with self.subTest(payload=payload):
                with self.assertRaisesRegex(H100ProtocolError, "byte count"):
                    parse_register_response(
                        append_crc(payload),
                        function=FUNCTION_READ_INPUT_REGISTERS,
                    )

        holding = append_crc(b"\x01\x03\x04\x12\x34\xAB\xCD")
        self.assertEqual(
            parse_register_response(
                holding, function=FUNCTION_READ_HOLDING_REGISTERS
            ),
            (0x1234, 0xABCD),
        )

    def test_frequency_encoder_rejects_every_invalid_domain(self) -> None:
        for f169 in (-1, 2, 0xFFFF):
            with self.subTest(f169=f169):
                with self.assertRaisesRegex(H100ProtocolError, "F169"):
                    encode_frequency_hz("1", f169=f169)
        for value in (None, "not-a-number", float("nan"), float("inf"), -1):
            with self.subTest(value=value):
                with self.assertRaises(H100ProtocolError):
                    encode_frequency_hz(value, f169=0)
        with self.assertRaisesRegex(H100ProtocolError, "exactly representable"):
            encode_frequency_hz("1.01", f169=0)
        self.assertEqual(encode_frequency_hz("6553.5", f169=0), 0xFFFF)
        with self.assertRaisesRegex(H100ProtocolError, "16-bit"):
            encode_frequency_hz("6553.6", f169=0)

        class BadString:
            def __str__(self):
                raise ValueError("unprintable")

        with self.assertRaisesRegex(H100ProtocolError, "decimal number"):
            encode_frequency_hz(BadString(), f169=0)

    def test_decision_gate_exercises_every_refusal(self) -> None:
        cases = (
            ({"run_requested": False}, "stop requested"),
            ({"link_fault": True}, "link fault"),
            ({"vfd_fault_code": 7}, "fault code 7"),
            ({"f001": 1}, "F001"),
            ({"f002": 1}, "F002"),
            ({"f024": 0}, "F024"),
            ({"f005_hundredths_hz": 0}, "F005"),
            ({"maximum_frequency_hz": None}, "not configured"),
            ({"maximum_frequency_hz": "401.0"}, "exceeds the H100 F005"),
            ({"requested_frequency_hz": "0"}, "greater than zero"),
            ({"requested_frequency_hz": "-1"}, "greater than zero"),
            (
                {
                    "requested_frequency_hz": "301.0",
                    "maximum_frequency_hz": "300.0",
                },
                "exceeds configured maximum",
            ),
        )
        for changes, reason in cases:
            with self.subTest(changes=changes):
                decision = decide_forward_command(
                    **self.decision_arguments(**changes)
                )
                self.assertEqual(decision.control_word, CONTROL_STOP)
                self.assertEqual(decision.frequency_register, 0)
                self.assertFalse(decision.run_permitted)
                self.assertIn(reason, decision.reason)

    def test_decision_gate_rejects_invalid_decimal_configuration(self) -> None:
        for changes in (
            {"requested_frequency_hz": "invalid"},
            {"maximum_frequency_hz": "invalid"},
            {"requested_frequency_hz": "NaN"},
            {"maximum_frequency_hz": "Infinity"},
            {"maximum_frequency_hz": "0"},
            {"maximum_frequency_hz": "-1"},
            {"f169": 2},
            {"requested_frequency_hz": "30.01"},
        ):
            with self.subTest(changes=changes):
                with self.assertRaises(H100ProtocolError):
                    decide_forward_command(**self.decision_arguments(**changes))


if __name__ == "__main__":
    unittest.main()
