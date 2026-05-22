#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 || $((($# - 3) % 4)) -ne 0 ]]; then
	echo "usage: $0 <bundle-dir> <prismo-src> <prismo-name> [<plugin-id> <manifest> <executable> <executable-name> ...]" >&2
	exit 1
fi

bundle="$1"
prismo_src="$2"
prismo_name="$3"
shift 3

mkdir -p "${bundle}/plugins"
cp "${prismo_src}" "${bundle}/${prismo_name}"
chmod +x "${bundle}/${prismo_name}"

while [[ $# -gt 0 ]]; do
	plugin_id="$1"
	manifest_src="$2"
	executable_src="$3"
	executable_name="$4"
	shift 4

	plugin_dst="${bundle}/plugins/${plugin_id}"
	mkdir -p "${plugin_dst}"
	cp "${manifest_src}" "${plugin_dst}/prismo-plugin.toml"
	cp "${executable_src}" "${plugin_dst}/${executable_name}"
	chmod +x "${plugin_dst}/${executable_name}"
done
