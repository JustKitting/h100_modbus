# H100 Modbus status

## User-observed hardware

- VFD: `H100-2.2C2-C0`, 2.2 kW, 110 V single-phase input.
- The VFD is powered and the low-voltage RS-485 wiring is connected.
- Current wiring uses the 7I95T signals printed `RX1+`, `RX1-`, `TX1+`, and
  `TX1-`:
  - `RX1-` joined to `TX1-`, then connected to H100 `485+`.
  - `RX1+` joined to `TX1+`, then connected to H100 `485-`.
  - The 7I95T serial `+5V` and `GND` are not connected to the H100.

## Verified Mesa state

- Board address: `192.168.1.121`.
- Complete pre-change flash backup:
  `../linuxcnc_dmc2/artifacts/firmware-backups/mesa/pre-pktuart/7i95t_before_pktuart_2026-08-21.bin`
- Backup SHA-256:
  `63e951ff8e66df57175c8d933d2ad35bb507e6108fd781ec74202c167c5b522b`
- Active firmware: official Mesa `7i95t_1pktv2d.bin`.
- Live `mesaflash --readhmid` verification found PktUART RX/TX version 2 on
  the printed `RX1`/`TX1` channel. All six StepGen modules, all six
  MuxedQCount modules, InMux, and SSR remain present.

## Read-only test

- LinuxCNC configuration: 19200 baud, 8N1, half duplex, slave address 1.
- The compiled test contains zero initialization commands and only Modbus
  functions 03 and 04 (reads). It contains no write function.
- Before the first read request was transmitted, PktUART reported repeated
  out-of-band receive frames. The frames included address `0x00`, undersized
  packets, and an RX overrun. No valid H100 response was accepted.
- Communication was suspended and HAL was fully unloaded. No spindle-control
  or frequency register was written.

## PktUART electrical-loop capture (2026-08-21)

- A one-shot diagnostic was compiled that can unmask the Mesa receiver during
  its own transmission. It loads zero step generators and contains exactly one
  possible request: `01 04 00 00 00 01 31 CA`.
- With that request sent exactly once, PktUART did not receive the expected
  eight-byte self-echo. It reported one error-marked frame:
  - RX status: `0x0B210F8B`
  - frame size/status: `0x0000C012` (18 bytes, false-start and overrun flags)
  - captured bytes: `00 00 00 00 00 00 00 00 00 00 02 11 01 02 02 14 14 52`
- A second capture configured the receiver identically but transmitted zero
  bytes. It completed with no frames and RX status `0x0B200F8A`. The low status
  byte means RX enabled, receive logic active, and sticky overrun/no-stop-bit.
- Therefore the receiver enters an invalid UART state while Mesa is
  transmitting nothing; this happens before any H100 request or response is
  decoded.
- Both sessions were stopped and HAL was fully unloaded afterward. No motor,
  spindle-run, or frequency command was loaded or transmitted.

## Verified Mesa idle polarity and staged correction

- Mesa's manual maps joined `TX-/RX-` to RS-485 `A`, and joined `TX+/RX+` to
  RS-485 `B`.
- Mesa's designer states that Mesa `RX+` and `TX+` are the inverted data lines.
  The documented external idle bias used on Mesa RS-485 ports is therefore:
  - joined `RX+/TX+` pulled toward `GND` through 330 ohms;
  - joined `RX-/TX-` pulled toward `+5V` through 330 ohms.
- The earlier note that a negative voltage measured red-on-Mesa-`+` and
  black-on-Mesa-`-` would indicate reversed idle polarity was incorrect. With
  Mesa's naming, that negative polarity is the intended UART idle polarity.
- The pre-swap passive zero-transmission capture proved that the serial-1 bus
  was holding the Mesa receiver in a logical low/no-stop-bit state instead of
  UART idle. This was before any Modbus request, CRC, or register was involved.
- The first staged correction was to reverse only the two conductors at the H100
  endpoint while keeping the Mesa half-duplex joins unchanged:
  - H100 `485+` to joined Mesa `RX1-/TX1-`;
  - H100 `485-` to joined Mesa `RX1+/TX1+`.
  This follows Mesa's documented `- = A`, `+ = B` mapping and the available
  H100-family field wiring report (`485+ = A`, `485- = B`). The official H100
  manual labels only `485+`/`485-` and does not itself provide an A/B mapping,
  so the zero-transmission Mesa status is the required validation after the
  change.
- After that change, the passive zero-byte capture reported clean idle with no
  receiver error, so no external bias resistors were added.
- Do not add another cross-pair termination resistor; the measured 117.7 ohms
  already confirms one termination is present.

## Post-swap verification (2026-08-21)

- Final wiring under test:
  - H100 `485+` to joined Mesa `RX1-/TX1-`;
  - H100 `485-` to joined Mesa `RX1+/TX1+`.
- Passive capture transmitted zero bytes. The `send-request` runtime pin was
  read back as `FALSE` before triggering. Result:
  - RX status: `0x0B000F08`;
  - zero frames;
  - receiver idle/busy clear;
  - no parity, receive-FIFO, no-stop/overrun, or false-start errors.
- After the clean passive result, exactly one read-only request was sent:
  `01 04 00 00 00 01 31 CA`.
- Mesa received two clean frames:
  - exact local self-echo: `01 04 00 00 00 01 31 CA`;
  - H100 response: `01 04 02 00 00 B9 30`.
- The response CRC is valid (`0x30B9`, wire order `B9 30`). It reports slave
  address 1, function 04, one input-register value of `0x0000`: current output
  frequency zero.
- This proves bidirectional Mesa-to-H100 Modbus RTU communication at 19200,
  8N1. No run, stop, direction, frequency-set, or parameter-write command was
  sent. HAL was fully unloaded after both captures.
- Successful capture files:
  - `../linuxcnc_dmc2/artifacts/captures/spindle-modbus/passive_20260821_111622/`
  - `../linuxcnc_dmc2/artifacts/captures/spindle-modbus/readonly_20260821_111737/`

Sources:

- 7I95T manual, printed pages 11 and 14:
  https://www.mesanet.com/pdf/parallel/7i95tman.pdf
- Mesa designer's idle-bias wiring:
  https://forum.linuxcnc.org/41-guis/49616-mesa-modbus-and-pktuart?start=20
- Official 7I96S manual, printed page 11, documenting the same 330-ohm Mesa
  RS-485 bias topology:
  https://www.mesanet.com/pdf/parallel/7i96sman.pdf
- H100-family field wiring report (`S+/485+` to A, `S-/485-` to B):
  https://forum.v1e.com/t/0-10v-i-o-cnc-module-voltage-issues/51734/47

## Files

- `rust/crates/h100-spindle/`: the native Rust LinuxCNC realtime component,
  separated into HAL lifecycle/transport and pure fail-stopped sequencer
  modules.
- `src/protocol/`: no-I/O Modbus RTU framing, CRC, response validation,
  frequency conversion, and command gating.
- `maps/live/`: the suspended `hm2_modbus` map used by the live LinuxCNC
  profile, with source and exact compiled form kept together.
- `maps/commissioning/`: the stopped one-time F005/F004 400 Hz writer and its
  exact compiled form. It contains STOP/zero but no RUN.
- `maps/diagnostics/`: read-only status and single-request link maps, each with
  source and exact compiled form.
- The Rust crate tests every public HAL pin and parameter, every lifecycle
  failure, block-code and state-transition boundaries, reset behavior, and an
  exhaustive 122,880-case sequencer invariant matrix.
- `tests/python/`: complete executable-line coverage for the pure protocol
  layer, including every invalid-frame and refusal path.
- `diagnostics/pktuart/`: explicitly invoked hardware diagnostic sources and
  capture scripts. The hardware-free verifier only syntax-checks these files.
- `tools/spindle_control.py`: non-live-by-default status reader and finite
  forward-run test harness. A live run requires explicit frequency, spindle
  maximum, duration, and `--live`. The one-time `configure-400hz --live` path
  was run and verified stopped on 2026-08-24.
- `scripts/build_release.sh`: builds the Rust realtime component twice in
  isolated directories, removes only nondeterministic debug/build-ID metadata,
  requires byte-identical normalized results, and stages
  `target/release/h100_spindle.so`.
- `scripts/verify.sh`: the single hardware-free release gate.

## Hardware-free verification

Run `./scripts/verify.sh`. It locks the build to LinuxCNC 2.9.10, takes every
sequencer and HAL-interface regression test, covers every executable protocol
line, rebuilds and byte-compares every `.mbccb`, builds the Rust realtime
module twice, and verifies the normalized module's architecture and RTAPI
exports.
It neither loads HAL nor opens Mesa or the VFD link.

## Source-verified spindle-control registers

The H100 operation manual identifies these holding registers and selector
values:

- `F001=2`: operating commands come from the communication interface.
- `F002=2`: frequency comes from holding register `0201H`.
- `F169=0`: `0201H` is in `0.1 Hz` units.
- `F169=1`: `0201H` is in `0.01 Hz` units.
- `F004`: motor reference frequency; the manual says it must match the motor
  nameplate. This installed four-digit H100 transfers it in `0.1 Hz` units.
- `F005`: maximum operating frequency, also transferred in `0.1 Hz` units on
  this installed H100. Live raw 500 was the 50.0 Hz factory value, and raw
  4000 read back after the one-time 400.0 Hz write.
- `F014/F015`: acceleration/deceleration time in `0.1 second` units.
- `F024=1`: the panel STOP key remains valid while `F001=2`.
- `0200H=0001H`: the manual's forward-run example.
- `0200H=0002H`: the explicit forward-direction command, from bit 1's mapping
  to parameter address `0049H`. Unlike the generic `0001H` operation command,
  this clears a previously latched reverse selection.
- `0200H=0004H`: reverse operation, from bit 2's mapping to parameter address
  `004AH`.
- `0200H=0008H`: STOP, from bit 3's mapping to parameter address `004BH`.
- `0201H`: given-frequency register.
- `0210H`: operating-state bit field.

The manual also exposes input registers `0000H..0006H` for output frequency,
set frequency, current, speed, DC voltage, AC voltage, and temperature, plus
`000AH` for the current fault. The prepared map reads all of these.

The live controller must observe `F001`, `F002`, `F004`, `F005`, `F011`,
`F024`, `F163-F165`, and `F169`, reject a nonzero fault, and match explicit
frequency and RPM limits before it can convert a requested RPM into a run
command. No spindle frequency has been inferred from the VFD's own `0-1000 Hz`
output capability.

## Prepared LinuxCNC command sequence

- Startup: the Modbus map starts suspended. HAL first establishes
  `0200H=0008H` (STOP) and `0201H=0`, then releases communication. RUN remains
  blocked unless live F004/F005 exactly match raw 4000, every selector and
  fault precondition is valid, and the requested speed is 6000..24000 RPM.
- `S<rpm> M3/M4`: validate every observed selector/limit/fault, convert RPM using
  the explicit rated RPM and F004 value, hold STOP, write `0201H`, wait until
  `0201H` reads back exactly, then apply the physically verified installation
  mapping: LinuxCNC `M3` (standard mill CW) writes the H100 reverse value
  `0200H=0004H`, while LinuxCNC `M4` (standard mill CCW) writes the H100
  explicit forward value `0200H=0002H`.
- A new `S<rpm>` during operation changes only `0201H`; LinuxCNC's
  `spindle.0.at-speed` remains false until live output frequency reaches the
  new target.
- `forward-running`, `reverse-running`, and `at-speed` require H100 `0210H`
  direction status to match the requested, physically verified direction; raw
  rotation at the requested frequency cannot satisfy those outputs. If that
  direction status changes after a running state was confirmed, the sequencer
  latches `DIRECTION_FEEDBACK_MISMATCH` and writes STOP.
- With no RUN request, `spindle.0.at-speed` is true in the conventional
  LinuxCNC stopped state. As soon as M3/M4 requests RUN it becomes false and
  cannot return true until the sequencer reaches RUNNING with live output
  frequency inside the configured tolerance.
- This installed H100's live `0210H` value reports in-operation but does not
  assert the manual's direction-status bit. Direction-running status therefore
  combines that live RUN bit with the sequencer's latched, physically verified
  `0200H` direction. The sequencer owns that command and refuses a direction
  change while operating; at-speed uses live RUN plus live output frequency.
- `M5`, machine-off, or E-stop: write STOP first while retaining the last
  frequency value. After `0210H` no longer reports in-operation and input
  register `0000H` reports zero frequency, clear `0201H` to zero.
- All 13 `hm2_modbus.command.NN.disabled` pins are monitored. A Modbus command
  that reaches the driver's five-consecutive-error disable threshold cannot
  leave stale last-good register values looking ready; it blocks or faults the
  sequencer until an operator reset re-enables the command.

The current live INI records 24,000 RPM rated/maximum speed, a 6,000 RPM
software minimum, and exact expected raw F004/F005 values of 4000. The stopped
integrated launch verified those values and reported `ready=true` without a
RUN request. A serial-link failure cannot deliver a STOP command over that
failed link; the H100 manual does not document a communication-loss stop
timer, so the existing physical stop remains necessary independently of this
software sequence.

Sources:

- Shariff DMC2 Mini specification (2.2 kW, ER20, 24,000 RPM):
  https://shariffdmc.com/dmc2-mini-tech-specs/
- DMC2 Mini/H100 installation settings (F004/F005=400 Hz, F011=100 Hz,
  2 poles, 24,000 RPM):
  https://forums.masso.com.au/threads/h100-vfd-pfi-and-pid.5308/
- H100 manual, printed pages 38 and 79-90:
  https://d2v0huudrf11kh.cloudfront.net/vevor-center-goods/H100manualV1.8%EF%BC%88%E6%96%B0%E5%91%BD%E5%90%8Dls%EF%BC%89%281%29%281%29_1653643589767.pdf
- 7I95T manual, printed pages 14-15:
  https://www.mesanet.com/pdf/parallel/7i95tman.pdf
- LinuxCNC `hm2_modbus` and `mesambccc` documentation:
  https://linuxcnc.org/docs/stable/html/drivers/mesa_modbus.html
