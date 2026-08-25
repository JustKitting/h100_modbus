#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(linuxcnc_var LINUXCNCVERSION)" != "2.9.10" ]]; then
    echo "refusing unaudited LinuxCNC version" >&2
    exit 1
fi

for script_path in "${project_dir}"/scripts/*.sh; do
    bash -n "${script_path}"
done
for script_path in "${project_dir}"/diagnostics/pktuart/*.sh; do
    bash -n "${script_path}"
done

git -C "${project_dir}" diff --check

while IFS= read -r -d '' source_path; do
    line_count="$(wc -l < "${source_path}")"
    if (( line_count > 1000 )); then
        echo "H100 source module exceeds 1000 lines: ${source_path}" >&2
        exit 1
    fi
done < <(
    find "${project_dir}" -type f \
        \( -name '*.c' -o -name '*.h' -o -name '*.py' -o -name '*.sh' \) \
        -not -path "${project_dir}/target/*" -print0
)

test_build_dir="$(mktemp -d /tmp/h100-tests.XXXXXX)"
map_build_dir="$(mktemp -d /tmp/h100-maps.XXXXXX)"
cleanup() {
    rm -rf -- "${test_build_dir}" "${map_build_dir}"
}
trap cleanup EXIT

cp -a -- "${project_dir}/src" "${project_dir}/tests" "${test_build_dir}/"

(
    cd -- "${test_build_dir}"
    common_flags=(
        -std=c11 -O0 -g --coverage
        -Wall -Wextra -Werror -pedantic
        -I src/component -I tests/c
    )
    cc "${common_flags[@]}" \
        -c src/component/h100_spindle_logic.c \
        -o h100_spindle_logic.o
    for source_path in \
        tests/c/test_support.c \
        tests/c/test_block_codes.c \
        tests/c/test_state_machine.c \
        tests/c/test_invariants.c \
        tests/c/test_main.c; do
        object_name="$(basename -- "${source_path}" .c).o"
        cc "${common_flags[@]}" -c "${source_path}" -o "${object_name}"
    done
    cc --coverage ./*.o -lm -o h100_spindle_tests
    ./h100_spindle_tests
    coverage_output="$(gcov -b -c -o . src/component/h100_spindle_logic.c)"
    printf '%s\n' "${coverage_output}"
    if [[ "${coverage_output}" != *"Lines executed:100.00% of 201"* ]]; then
        echo "H100 sequencer line coverage is not exactly 100%" >&2
        exit 1
    fi
    if [[ "${coverage_output}" != *"Branches executed:100.00% of 215"* ]]; then
        echo "H100 sequencer branch discovery is not exactly 100%" >&2
        exit 1
    fi
    if [[ "${coverage_output}" != *"Taken at least once:100.00% of 215"* ]]; then
        echo "at least one H100 sequencer branch outcome was not tested" >&2
        exit 1
    fi
)

optimized_test="${test_build_dir}/h100_spindle_tests_optimized"
cc \
    -std=c11 -O2 -Wall -Wextra -Werror -pedantic \
    -fsanitize=undefined -fno-sanitize-recover=all \
    -I "${project_dir}/src/component" \
    -I "${project_dir}/tests/c" \
    "${project_dir}/src/component/h100_spindle_logic.c" \
    "${project_dir}/tests/c/test_support.c" \
    "${project_dir}/tests/c/test_block_codes.c" \
    "${project_dir}/tests/c/test_state_machine.c" \
    "${project_dir}/tests/c/test_invariants.c" \
    "${project_dir}/tests/c/test_main.c" \
    -lm -o "${optimized_test}"
"${optimized_test}"

python_trace_dir="${test_build_dir}/python-coverage"
mkdir -p -- "${python_trace_dir}"
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH="${project_dir}/src/protocol" \
python3 -m trace \
    --count --missing --summary --coverdir "${python_trace_dir}" \
    --module unittest discover \
    -s "${project_dir}/tests/python" -p 'test_*.py'
protocol_coverage="$(
    find "${python_trace_dir}" -type f -name 'h100_protocol.cover' -print -quit
)"
if [[ -z "${protocol_coverage}" || ! -f "${protocol_coverage}" ]]; then
    echo "Python trace did not produce H100 protocol coverage" >&2
    exit 1
fi
if grep -q '^>>>>>>' "${protocol_coverage}"; then
    echo "at least one executable H100 protocol line was not tested" >&2
    grep -n '^>>>>>>' "${protocol_coverage}" >&2
    exit 1
fi
echo "H100 protocol executable-line coverage is exactly 100%"

while IFS= read -r -d '' source_path; do
    relative_path="${source_path#"${project_dir}/"}"
    binary_relative="${relative_path%.mbccs}.mbccb"
    binary_path="${project_dir}/${binary_relative}"
    rebuilt_path="${map_build_dir}/${binary_relative}"
    if [[ ! -f "${binary_path}" ]]; then
        echo "missing compiled map for ${relative_path}" >&2
        exit 1
    fi
    mkdir -p -- "$(dirname -- "${rebuilt_path}")"
    mesambccc -o "${rebuilt_path}" "${source_path}"
    if ! cmp --silent "${rebuilt_path}" "${binary_path}"; then
        echo "compiled Modbus map is stale: ${binary_relative}" >&2
        exit 1
    fi
    echo "exact compiled map: ${binary_relative}"
done < <(find "${project_dir}/maps" -type f -name '*.mbccs' -print0 | sort -z)

while IFS= read -r -d '' binary_path; do
    source_path="${binary_path%.mbccb}.mbccs"
    if [[ ! -f "${source_path}" ]]; then
        echo "compiled Modbus map has no source: ${binary_path}" >&2
        exit 1
    fi
done < <(find "${project_dir}/maps" -type f -name '*.mbccb' -print0 | sort -z)

"${project_dir}/scripts/build_release.sh"

if find "${project_dir}" -type d \
    \( -name __pycache__ -o -name .pytest_cache \) \
    -not -path "${project_dir}/target/*" -print -quit | grep -q .; then
    echo "generated Python cache exists in the H100 project" >&2
    exit 1
fi

echo "all hardware-free H100 verification passed"
