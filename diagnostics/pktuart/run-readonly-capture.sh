#!/usr/bin/env bash
set -Eeuo pipefail

board_ip="192.168.1.121"
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
capture_stamp="$(date +%Y%m%d_%H%M%S)"
capture_dir="${project_dir}/artifacts/captures/readonly_${capture_stamp}"
hal_log="${capture_dir}/halrun.log"
result_log="${capture_dir}/result.txt"

hal_pid=""
hal_in_fd=""

hal_command() {
    printf '%s\n' "$1" >&"$hal_in_fd"
}

cleanup() {
    if [[ -n "$hal_in_fd" && -e "/proc/$$/fd/${hal_in_fd}" ]]; then
        printf 'stop\nunloadrt all\nquit\n' >&"$hal_in_fd" 2>/dev/null || true
    fi
    if [[ -n "$hal_pid" ]]; then
        wait "$hal_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

if pgrep -x linuxcnc >/dev/null || pgrep -x halrun >/dev/null; then
    echo 'REFUSED: LinuxCNC or halrun is already active; read-only capture not started.' >&2
    exit 1
fi

if ! strings /usr/lib/linuxcnc/modules/pktuart_echo_diag.so \
    | grep -q 'sent_once bytes=8 request=01 04 00 00 00 01 31 CA'; then
    echo 'REFUSED: installed diagnostic does not identify the exact approved request.' >&2
    exit 1
fi

mkdir -p "$capture_dir"

coproc HAL_SESSION { halrun -s >"$hal_log" 2>&1; }
hal_pid=$HAL_SESSION_PID
hal_in_fd=${HAL_SESSION[1]}

hal_command 'loadrt threads name1=servo-thread period1=1000000'
hal_command 'loadrt hostmot2'
hal_command "loadrt hm2_eth board_ip=${board_ip} config=\"num_encoders=0 num_stepgens=0 num_pwmgens=0 num_3pwmgens=0 num_inmuxs=0 num_ssrs=0 num_pktuarts=1\""
hal_command 'loadrt pktuart_echo_diag'

# Explicitly select the component's single hard-coded function-04 request.
hal_command 'setp pktuart-echo-diag.send-request true'

hal_command 'addf hm2_7i95.0.read servo-thread'
hal_command 'addf pktuart-echo-diag servo-thread'
hal_command 'addf hm2_7i95.0.write servo-thread'
hal_command 'start'

sleep 0.25
if ! kill -0 "$hal_pid" 2>/dev/null; then
    cat "$hal_log" >&2 || true
    exit 1
fi

if ! halcmd show funct hm2_7i95.0.read 2>/dev/null | grep -q 'hm2_7i95.0.read'; then
    echo 'REFUSED: Mesa read function did not load; request not triggered.' >&2
    cat "$hal_log" >&2 || true
    exit 1
fi
if ! halcmd show funct hm2_7i95.0.write 2>/dev/null | grep -q 'hm2_7i95.0.write'; then
    echo 'REFUSED: Mesa write function did not load; request not triggered.' >&2
    cat "$hal_log" >&2 || true
    exit 1
fi
if ! halcmd show funct pktuart-echo-diag 2>/dev/null | grep -q 'pktuart-echo-diag'; then
    echo 'REFUSED: diagnostic function did not load; request not triggered.' >&2
    cat "$hal_log" >&2 || true
    exit 1
fi

packet_error="$(halcmd -s getp hm2_7i95.0.packet-error)"
if [[ "$packet_error" != "FALSE" ]]; then
    echo "REFUSED: Mesa packet-error readback was ${packet_error}; request not triggered." >&2
    cat "$hal_log" >&2 || true
    exit 1
fi

send_request="$(halcmd -s getp pktuart-echo-diag.send-request)"
if [[ "$send_request" != "TRUE" ]]; then
    echo "REFUSED: runtime send-request readback was ${send_request}; request not triggered." >&2
    exit 1
fi

hal_command 'setp pktuart-echo-diag.trigger true'
sleep 0.50

done_value="$(halcmd -s getp pktuart-echo-diag.done)"
result_value="$(halcmd -s getp pktuart-echo-diag.result)"
state_value="$(halcmd -s getp pktuart-echo-diag.state)"
rx_status_value="$(halcmd -s getp pktuart-echo-diag.rx-status)"
tx_status_value="$(halcmd -s getp pktuart-echo-diag.tx-status)"
frame_count_value="$(halcmd -s getp pktuart-echo-diag.frame-count)"

{
    printf 'mode=single-read-only-request\n'
    printf 'request=01_04_00_00_00_01_31_CA\n'
    printf 'transmitted_bytes=8\n'
    printf 'send_request_readback=%s\n' "$send_request"
    printf 'done=%s\n' "$done_value"
    printf 'result=%s\n' "$result_value"
    printf 'state=%s\n' "$state_value"
    printf 'rx_status=%s\n' "$rx_status_value"
    printf 'tx_status=%s\n' "$tx_status_value"
    printf 'frame_count=%s\n' "$frame_count_value"
} | tee "$result_log"

hal_command 'stop'
hal_command 'unloadrt all'
hal_command 'quit'
wait "$hal_pid"
hal_pid=""
hal_in_fd=""
trap - EXIT INT TERM

printf 'hal_log=%s\n' "$hal_log"
printf 'result_log=%s\n' "$result_log"
cat "$hal_log"
