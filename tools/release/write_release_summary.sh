#!/usr/bin/env bash
set -euo pipefail

printf 'version: %s\n' "${VERSION:-}"
printf 'previous_version: %s\n' "${PREVIOUS_VERSION:-n/a}"
printf 'version_changed: %s\n' "${VERSION_CHANGED:-false}"
printf 'release_exists: %s\n' "${RELEASE_EXISTS:-false}"
