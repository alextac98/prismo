#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${RUNFILES_DIR:-}" ]]; then
  RUNFILES_ROOT="${RUNFILES_DIR}"
elif [[ -n "${TEST_SRCDIR:-}" ]]; then
  RUNFILES_ROOT="${TEST_SRCDIR}"
else
  echo "runfiles directory not available" >&2
  exit 1
fi

source "${RUNFILES_ROOT}/bazel_tools/tools/bash/runfiles/runfiles.bash"

if [[ $# -ne 2 ]]; then
  echo "expected app and plugin rootpaths" >&2
  exit 1
fi

app_path="$(rlocation "$1")"
plugin_path="$(rlocation "$2")"

if [[ ! -x "${app_path}" ]]; then
  echo "app binary not found: ${app_path}" >&2
  exit 1
fi

if [[ ! -x "${plugin_path}" ]]; then
  echo "plugin binary not found: ${plugin_path}" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

plugins_dir="${tmp_dir}/plugins"
plugin_dir="${plugins_dir}/example-cpp"
manifest_path="${plugin_dir}/prismo-plugin.toml"

mkdir -p "${plugin_dir}"

cat > "${manifest_path}" <<EOF
schema_version = 1
plugin_id = "example-cpp"
display_name = "Example C++ Plugin"
plugin_version = "0.1.0"
protocol_version = 1
language = "cpp"

[entrypoint]
argv = ["${plugin_path}"]
EOF

"${app_path}" smoke-test --plugins "${plugins_dir}" --plugin-id example-cpp --timeout-ms 5000
