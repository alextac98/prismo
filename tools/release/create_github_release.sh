#!/usr/bin/env bash
set -euo pipefail

: "${TAG:?TAG is required}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
: "${PRERELEASE:?PRERELEASE is required}"

args=(
  release
  create
  "$TAG"
  --repo
  "$GITHUB_REPOSITORY"
  --target
  "$GITHUB_SHA"
  --title
  "$TAG"
  --generate-notes
)

if [[ "$PRERELEASE" == "true" ]]; then
  args+=(--prerelease)
fi

gh "${args[@]}"
