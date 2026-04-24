#!/usr/bin/env bash
set -euo pipefail

: "${TAG:?TAG is required}"
: "${VERSION:?VERSION is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT is required}"

repo_name="${GITHUB_REPOSITORY#*/}"
archive_name="${repo_name}-${TAG}.tar.gz"
prefix_dir="${repo_name}-${VERSION}/"
archive_ref="${ARCHIVE_REF:-${GITHUB_SHA:-}}"

if [[ -z "${archive_ref}" ]]; then
  archive_ref="${TAG}"
fi

git archive --format=tar --prefix="${prefix_dir}" "${archive_ref}" | gzip -n > "${archive_name}"

echo "archive_path=${archive_name}" >> "${GITHUB_OUTPUT}"
