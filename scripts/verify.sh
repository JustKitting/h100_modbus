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
        \( -name '*.rs' -o -name '*.c' -o -name '*.h' -o -name '*.py' -o -name '*.sh' \) \
        -not -path "${project_dir}/target/*" \
        -not -path "${project_dir}/rust/target/*" -print0
)

test_build_dir="$(mktemp -d /tmp/h100-tests.XXXXXX)"
map_build_dir="$(mktemp -d /tmp/h100-maps.XXXXXX)"
cleanup() {
    rm -rf -- "${test_build_dir}" "${map_build_dir}"
}
trap cleanup EXIT

cargo fmt --manifest-path "${project_dir}/rust/Cargo.toml" -p h100-spindle -- --check
env RUSTFLAGS=-Dwarnings \
    cargo test --manifest-path "${project_dir}/rust/Cargo.toml" --workspace --locked

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
    -not -path "${project_dir}/target/*" \
    -not -path "${project_dir}/rust/target/*" -print -quit | grep -q .; then
    echo "generated Python cache exists in the H100 project" >&2
    exit 1
fi

echo "all hardware-free H100 verification passed"
