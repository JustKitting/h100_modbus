#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
component_dir="${project_dir}/src/component"
release_dir="${project_dir}/target/release"
release_module="${release_dir}/h100_spindle.so"

if [[ "$(linuxcnc_var LINUXCNCVERSION)" != "2.9.10" ]]; then
    echo "refusing to build H100 module against anything except LinuxCNC 2.9.10" >&2
    exit 1
fi

for required_path in \
    "${component_dir}/h100_spindle.comp" \
    "${component_dir}/h100_spindle_logic.c" \
    "${component_dir}/h100_spindle_logic.h"; do
    if [[ ! -f "${required_path}" ]]; then
        echo "missing H100 component source: ${required_path}" >&2
        exit 1
    fi
done

build_root="$(mktemp -d /tmp/h100-release-build.XXXXXX)"
temporary_release=""
cleanup() {
    rm -rf -- "${build_root}"
    if [[ -n "${temporary_release}" ]]; then
        rm -f -- "${temporary_release}"
    fi
}
trap cleanup EXIT

build_normalized_module() {
    local build_dir="$1"
    local output_path="$2"

    mkdir -p -- "${build_dir}"
    install -m 0644 "${component_dir}/h100_spindle.comp" "${build_dir}/"
    install -m 0644 "${component_dir}/h100_spindle_logic.c" "${build_dir}/"
    install -m 0644 "${component_dir}/h100_spindle_logic.h" "${build_dir}/"
    (
        cd -- "${build_dir}"
        halcompile --compile h100_spindle.comp
    )
    objcopy \
        --strip-debug \
        --remove-section=.note.gnu.build-id \
        "${build_dir}/h100_spindle.so" \
        "${output_path}"
    chmod 0755 "${output_path}"
}

build_normalized_module "${build_root}/first" "${build_root}/first.so"
build_normalized_module "${build_root}/second" "${build_root}/second.so"

if ! cmp --silent "${build_root}/first.so" "${build_root}/second.so"; then
    echo "normalized H100 module is not reproducible across isolated builds" >&2
    exit 1
fi
if readelf -SW "${build_root}/first.so" | grep -Eq '\.debug|\.note\.gnu\.build-id'; then
    echo "normalized H100 module still contains nondeterministic metadata" >&2
    exit 1
fi
for symbol in rtapi_app_main rtapi_app_exit; do
    if ! readelf -Ws "${build_root}/first.so" | grep -Eq " ${symbol}$"; then
        echo "H100 realtime module is missing ${symbol}" >&2
        exit 1
    fi
done
if ! readelf -h "${build_root}/first.so" | grep -Eq 'Type:[[:space:]]+DYN'; then
    echo "H100 release is not an ELF shared object" >&2
    exit 1
fi
if ! readelf -h "${build_root}/first.so" | grep -Eq 'Machine:[[:space:]]+AArch64'; then
    echo "H100 release was not built for the controller's AArch64 host" >&2
    exit 1
fi

mkdir -p -- "${release_dir}"
temporary_release="$(mktemp "${release_dir}/.h100_spindle.so.XXXXXX")"
install -m 0755 "${build_root}/first.so" "${temporary_release}"
cmp --silent "${build_root}/first.so" "${temporary_release}"
mv -f -- "${temporary_release}" "${release_module}"
temporary_release=""
cmp --silent "${build_root}/first.so" "${release_module}"

sha256sum "${release_module}"
echo "reproducible H100 realtime release built: ${release_module}"
