#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TOOL_ROOT="${ROOT}/target/diplomat-tool"

if [[ ! -x "${TOOL_ROOT}/bin/diplomat-tool" ]]; then
  cargo install --locked diplomat-tool --version 0.10.0 --root "${TOOL_ROOT}"
fi

mkdir -p "${ROOT}/crates/plugin-sdk-ffi/generated/c" "${ROOT}/crates/plugin-sdk-ffi/generated/cpp"
"${TOOL_ROOT}/bin/diplomat-tool" c "${ROOT}/crates/plugin-sdk-ffi/generated/c" -e "${ROOT}/crates/plugin-sdk-ffi/src/lib.rs"
"${TOOL_ROOT}/bin/diplomat-tool" cpp "${ROOT}/crates/plugin-sdk-ffi/generated/cpp" -e "${ROOT}/crates/plugin-sdk-ffi/src/lib.rs"
