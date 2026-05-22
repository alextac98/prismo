#!/usr/bin/env bash
set -euo pipefail

: "${ASSET_PATH:?ASSET_PATH is required}"
: "${TAG:?TAG is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

gh release upload \
	"${TAG}" \
	"${ASSET_PATH}" \
	--repo "${GITHUB_REPOSITORY}" \
	--clobber
