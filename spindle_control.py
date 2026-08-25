#!/usr/bin/env python3
"""Prepared H100 status and finite forward-run test controller.

Without --live this program performs validation only and opens no hardware.
The live test path requires an explicit frequency, spindle maximum, and test
duration. The separate configure-400hz path performs only a stopped, one-time
parameter write and exact readback.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from decimal import Decimal, InvalidOperation
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
PENDANT_CNC_DIR = PROJECT_ROOT / "pendant_cnc"
if str(PENDANT_CNC_DIR) not in sys.path:
    sys.path.insert(0, str(PENDANT_CNC_DIR))

import hal

from hal_session import HalSession, HalSessionError
from h100_protocol import (
    CONTROL_STOP,
    H100ProtocolError,
    SpindleDecision,
    decide_forward_command,
)


BOARD_HAL_NAME = "hm2_7i95.0"
PKTUART_NAME = f"{BOARD_HAL_NAME}.pktuart.0"
MODBUS_PREFIX = "hm2_modbus.0"
DEVICE_PREFIX = f"{MODBUS_PREFIX}.h100"
SERVO_PERIOD_NS = 1_000_000
READ_ONLY_MAP = Path(__file__).with_name("h100-readonly.mbccb")
CONTROL_MAP = Path(__file__).with_name("h100-spindle.mbccb")
CONFIGURE_400HZ_MAP = Path(__file__).with_name("h100-configure-400hz.mbccb")


class SpindleControlError(RuntimeError):
    pass


def process_is_running(name: str) -> bool:
    completed = subprocess.run(
        ["pgrep", "-x", name],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return completed.returncode == 0


def parse_positive_decimal(value: str, *, label: str) -> Decimal:
    try:
        parsed = Decimal(value)
    except InvalidOperation as error:
        raise argparse.ArgumentTypeError(f"{label} must be a decimal number") from error
    if not parsed.is_finite() or parsed <= 0:
        raise argparse.ArgumentTypeError(f"{label} must be greater than zero")
    return parsed


class H100HalLink:
    _component_sequence = 0

    def __init__(
        self,
        *,
        board_ip: str,
        control_enabled: bool,
        map_path: Path | None = None,
    ) -> None:
        self.board_ip = board_ip
        self.control_enabled = control_enabled
        self.map_path = map_path
        self.session = HalSession()
        self.hal_component: hal.component | None = None
        self.started = False
        self.unsuspended = False

    @staticmethod
    def _pin(name: str) -> str:
        return f"{DEVICE_PREFIX}.{name}"

    def read(self, name: str) -> int:
        return int(hal.get_value(self._pin(name)))

    def link_fault(self) -> bool:
        return bool(hal.get_value(f"{MODBUS_PREFIX}.fault"))

    def set_raw(self, name: str, value: int) -> None:
        hal.set_p(self._pin(name), str(value))

    def configure(self) -> None:
        map_path = self.map_path
        if map_path is None:
            map_path = CONTROL_MAP if self.control_enabled else READ_ONLY_MAP
        if not map_path.is_file():
            raise SpindleControlError(f"compiled Modbus map is missing: {map_path}")

        self.session.start()
        H100HalLink._component_sequence += 1
        component_name = (
            f"h100-tool-{os.getpid()}-{H100HalLink._component_sequence}"
        )
        self.hal_component = hal.component(component_name)
        self.hal_component.ready()
        self.session.commands(
            [
                (
                    "loadrt",
                    "threads",
                    "name1=servo-thread",
                    f"period1={SERVO_PERIOD_NS}",
                ),
                ("loadrt", "hostmot2"),
                (
                    "loadrt",
                    "hm2_eth",
                    f"board_ip={self.board_ip}",
                    "config=num_encoders=0 num_stepgens=0 num_pwmgens=0 "
                    "num_3pwmgens=0 num_inmuxs=0 num_ssrs=0 num_pktuarts=1",
                ),
                (
                    "loadrt",
                    "hm2_modbus",
                    f"ports={PKTUART_NAME}",
                    f"mbccbs={map_path}",
                ),
            ]
        )

        if self.control_enabled:
            # These values are established while communication is still suspended.
            self.set_raw("given-frequency", 0)
            self.set_raw("main-control", CONTROL_STOP)

        self.session.commands(
            [
                ("addf", f"{BOARD_HAL_NAME}.read", "servo-thread"),
                ("addf", f"{MODBUS_PREFIX}.process", "servo-thread"),
                ("addf", f"{BOARD_HAL_NAME}.write", "servo-thread"),
                ("start",),
            ]
        )
        self.started = True
        hal.set_p(f"{MODBUS_PREFIX}.suspend", "false")
        self.unsuspended = True

    def wait_for_communication(self, timeout_seconds: float = 4.0) -> None:
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            if self.link_fault():
                raise SpindleControlError("hm2_modbus reported a communication fault")
            if (
                self.read("slave-address-f163" if self.control_enabled else "slave-address")
                == 1
                and self.read(
                    "baud-selector-f164" if self.control_enabled else "baud-selector"
                )
                == 2
                and self.read("data-mode-f165" if self.control_enabled else "data-mode")
                == 3
            ):
                # F163/F164/F165 prove that command 2/4 completed. Let the same
                # command list finish once so every later status pin is fresh.
                time.sleep(0.5)
                if self.link_fault():
                    raise SpindleControlError(
                        "hm2_modbus reported a communication fault"
                    )
                return
            time.sleep(0.02)
        raise SpindleControlError(
            "timed out before reading the verified F163=1, F164=2, F165=3 settings"
        )

    def status(self) -> dict[str, int]:
        suffix = {
            "f001": "control-mode-f001" if self.control_enabled else "control-mode",
            "f002": (
                "frequency-source-f002" if self.control_enabled else "frequency-source"
            ),
            "f004_hundredths_hz": (
                "reference-f004-centihz"
            ),
            "f005_hundredths_hz": (
                "maximum-f005-centihz"
            ),
            "f014_tenths_second": (
                "accel-f014-deciseconds"
            ),
            "f015_tenths_second": (
                "decel-f015-deciseconds"
            ),
            "f011_hundredths_hz": "lower-limit-f011-centihz",
            "f024": "panel-stop-f024",
            "f163": "slave-address-f163" if self.control_enabled else "slave-address",
            "f164": "baud-selector-f164" if self.control_enabled else "baud-selector",
            "f165": "data-mode-f165" if self.control_enabled else "data-mode",
            "f169": (
                "frequency-decimals-f169"
                if self.control_enabled
                else "frequency-decimals"
            ),
            "output_frequency": "output-frequency",
            "set_frequency": "set-frequency",
            "current_fault": "current-fault",
            "main_status": "main-status",
        }
        return {key: self.read(pin_name) for key, pin_name in suffix.items()}

    def wait_for_value(
        self,
        name: str,
        expected: int,
        *,
        timeout_seconds: float,
    ) -> None:
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            if self.link_fault():
                raise SpindleControlError("hm2_modbus reported a communication fault")
            if self.read(name) == expected:
                return
            time.sleep(0.02)
        raise SpindleControlError(
            f"timed out waiting for {name}={expected}; observed {self.read(name)}"
        )

    def command_stop(self) -> None:
        if self.control_enabled and self.unsuspended:
            self.set_raw("main-control", CONTROL_STOP)

    def close(self) -> None:
        if self.control_enabled and self.unsuspended:
            try:
                self.command_stop()
                time.sleep(0.2)
            except Exception:
                pass
        if self.hal_component is not None:
            try:
                self.hal_component.exit()
            except Exception:
                pass
            self.hal_component = None
        self.session.close()
        self.started = False
        self.unsuspended = False


def check_for_conflicting_processes() -> None:
    active = [
        name
        for name in ("linuxcnc", "halrun", "rtapi_app")
        if process_is_running(name)
    ]
    if active:
        raise SpindleControlError(
            "existing HAL ownership detected ("
            + ", ".join(active)
            + "); no spindle communication started"
        )


def print_status(values: dict[str, int]) -> None:
    print(
        "H100 STATUS "
        + " ".join(f"{name}={value}" for name, value in values.items()),
        flush=True,
    )


def run_status(args: argparse.Namespace) -> int:
    if not args.live:
        print("VALIDATION ONLY — status mode would send Modbus reads only.")
        return 0
    check_for_conflicting_processes()
    link = H100HalLink(board_ip=args.board_ip, control_enabled=False)
    try:
        link.configure()
        link.wait_for_communication()
        print_status(link.status())
        return 0
    finally:
        link.close()


def run_finite_test(args: argparse.Namespace) -> int:
    requested = args.frequency_hz
    maximum = args.maximum_frequency_hz
    duration = args.duration_seconds
    if requested > maximum:
        raise SpindleControlError(
            "requested frequency exceeds the explicit maximum spindle frequency"
        )
    if not args.live:
        print(
            "VALIDATION ONLY — no HAL, Mesa, Modbus, or spindle output opened. "
            f"Prepared finite forward test: frequency={requested} Hz, "
            f"maximum={maximum} Hz, duration={duration} seconds; "
            "F169 will be read before frequency is encoded."
        )
        return 0

    check_for_conflicting_processes()
    link = H100HalLink(board_ip=args.board_ip, control_enabled=True)
    decision: SpindleDecision | None = None
    try:
        link.configure()
        link.wait_for_communication()
        link.wait_for_value(
            "given-frequency-readback",
            0,
            timeout_seconds=2.0,
        )
        status = link.status()
        print_status(status)
        decision = decide_forward_command(
            run_requested=True,
            requested_frequency_hz=requested,
            maximum_frequency_hz=maximum,
            f001=status["f001"],
            f002=status["f002"],
            f005_hundredths_hz=status["f005_hundredths_hz"],
            f024=status["f024"],
            f169=status["f169"],
            link_fault=link.link_fault(),
            vfd_fault_code=status["current_fault"],
        )
        if not decision.run_permitted:
            raise SpindleControlError(f"run refused: {decision.reason}")
        if status["output_frequency"] != 0 or status["main_status"] & 0x0008:
            raise SpindleControlError(
                "run refused because the VFD does not report a stopped state"
            )

        link.set_raw("given-frequency", decision.frequency_register)
        link.wait_for_value(
            "given-frequency-readback",
            decision.frequency_register,
            timeout_seconds=2.0,
        )
        link.set_raw("main-control", decision.control_word)
        print(
            f"FORWARD RUN SENT raw_frequency={decision.frequency_register} "
            f"duration_seconds={duration}",
            flush=True,
        )

        deadline = time.monotonic() + float(duration)
        saw_running_status = False
        while time.monotonic() < deadline:
            current = link.status()
            if link.link_fault() or current["current_fault"] != 0:
                raise SpindleControlError(
                    "link or H100 fault occurred during the finite run"
                )
            if current["output_frequency"] != 0 or current["main_status"] & 0x0008:
                saw_running_status = True
            time.sleep(min(0.02, max(0.0, deadline - time.monotonic())))

        link.command_stop()
        link.wait_for_value("output-frequency", 0, timeout_seconds=30.0)
        link.set_raw("given-frequency", 0)
        print("STOP CONFIRMED output_frequency=0", flush=True)
        if not saw_running_status:
            raise SpindleControlError(
                "the finite test completed but the H100 never reported running"
            )
        return 0
    finally:
        link.close()


def run_configure_400hz(args: argparse.Namespace) -> int:
    """Write the verified DMC2 F004/F005 values once, while stopped."""

    target_raw = 4000  # 400.0 Hz in this installed four-digit H100.
    if not args.live:
        print(
            "VALIDATION ONLY — no HAL, Mesa, Modbus, or spindle output opened. "
            "Prepared one-time stopped configuration: F005=400.0 Hz, then "
            "F004=400.0 Hz; no RUN command exists in the configuration map."
        )
        return 0

    check_for_conflicting_processes()
    preflight = H100HalLink(board_ip=args.board_ip, control_enabled=False)
    before: dict[str, int]
    try:
        preflight.configure()
        preflight.wait_for_communication()
        before = preflight.status()
        print("BEFORE CONFIGURATION", flush=True)
        print_status(before)
        if before["output_frequency"] != 0 or before["main_status"] & 0x0008:
            raise SpindleControlError(
                "configuration refused because the VFD does not report stopped"
            )
        if before["current_fault"] != 0:
            raise SpindleControlError(
                f"configuration refused because H100 fault "
                f"{before['current_fault']} is active"
            )
        if (
            before["f004_hundredths_hz"] == target_raw
            and before["f005_hundredths_hz"] == target_raw
        ):
            print(
                "CONFIGURATION ALREADY CONFIRMED F004=400.0Hz F005=400.0Hz; "
                "no EEPROM write sent",
                flush=True,
            )
            return 0
    finally:
        preflight.close()

    configured = H100HalLink(
        board_ip=args.board_ip,
        control_enabled=False,
        map_path=CONFIGURE_400HZ_MAP,
    )
    try:
        configured.configure()
        configured.wait_for_communication()
        after = configured.status()
        print("AFTER CONFIGURATION", flush=True)
        print_status(after)
        if after["output_frequency"] != 0 or after["main_status"] & 0x0008:
            raise SpindleControlError(
                "VFD reported motion during a configuration-only operation"
            )
        if after["current_fault"] != 0:
            raise SpindleControlError(
                f"H100 fault {after['current_fault']} occurred during configuration"
            )
        if after["f005_hundredths_hz"] != target_raw:
            raise SpindleControlError(
                "F005 write did not verify as raw 4000 (400.0 Hz)"
            )
        if after["f004_hundredths_hz"] != target_raw:
            raise SpindleControlError(
                "F004 write did not verify as raw 4000 (400.0 Hz)"
            )
        print(
            "CONFIGURATION CONFIRMED F004=400.0Hz F005=400.0Hz spindle_stopped=1",
            flush=True,
        )
        return 0
    finally:
        configured.close()


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="H100/Mesa spindle status and finite forward-test controller"
    )
    parser.add_argument("--board-ip", default="192.168.1.121")
    subparsers = parser.add_subparsers(dest="command", required=True)

    status = subparsers.add_parser("status", help="read status without write commands")
    status.add_argument("--live", action="store_true")
    status.set_defaults(handler=run_status)

    test = subparsers.add_parser(
        "finite-test", help="run one explicit forward-frequency test then stop"
    )
    test.add_argument(
        "--frequency-hz",
        required=True,
        type=lambda value: parse_positive_decimal(value, label="frequency"),
    )
    test.add_argument(
        "--maximum-frequency-hz",
        required=True,
        type=lambda value: parse_positive_decimal(value, label="maximum frequency"),
    )
    test.add_argument(
        "--duration-seconds",
        required=True,
        type=lambda value: parse_positive_decimal(value, label="duration"),
    )
    test.add_argument("--live", action="store_true")
    test.set_defaults(handler=run_finite_test)

    configure = subparsers.add_parser(
        "configure-400hz",
        help="one-time stopped write of DMC2 F004/F005=400.0 Hz",
    )
    configure.add_argument("--live", action="store_true")
    configure.set_defaults(handler=run_configure_400hz)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    return args.handler(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("SPINDLE TEST INTERRUPTED; STOP requested during cleanup", file=sys.stderr)
        raise SystemExit(130)
    except (
        H100ProtocolError,
        HalSessionError,
        OSError,
        SpindleControlError,
    ) as error:
        print(f"SPINDLE CONTROL REFUSED: {error}", file=sys.stderr, flush=True)
        raise SystemExit(1)
