#!/usr/bin/env bash

set -euo pipefail

tool="$1"
shift

exec "$tool" "$@"
