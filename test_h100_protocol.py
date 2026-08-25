from __future__ import annotations

import unittest

from h100_protocol import (
    CONTROL_RUN,
    CONTROL_STOP,
    FUNCTION_READ_INPUT_REGISTERS,
    H100ProtocolError,
    REGISTER_GIVEN_FREQUENCY,
    decide_forward_command,
    encode_frequency_hz,
    parse_register_response,
    read_registers_request,
    verify_crc,
    write_single_register_request,
)


class H100ProtocolTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
