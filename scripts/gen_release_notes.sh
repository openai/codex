#!/usr/bin/env bash
set -euo pipefail

# Generate release notes for a rust-v* tag by diffing against the previous
# rust-v* tag that is an ancestor of the given tag. Prefer git-cliff if
# available; otherwise fall back to a simple git log summary.
#
# Usage:
#   scripts/gen_release_notes.sh rust-v0.30.0-fork.3 [OUTPUT]
#     - OUTPUT defaults to ./RELEASE_NOTES.md

TAG_NAME="${1:-}"
OUT_FILE="${2:-RELEASE_NOTES.md}"

if [[ -z "$TAG_NAME" ]]; then
  echo "Usage: $(basename "$0") rust-vX.Y.Z[-suffix] [OUTPUT]" >&2
  exit 1
fi

git fetch --tags --force >/dev/null 2>&1 || true

current_ref="refs/tags/${TAG_NAME}"
current_commit=$(git rev-list -n 1 "$current_ref")
if [[ -z "$current_commit" ]]; then
  echo "Error: tag '$TAG_NAME' not found" >&2
  exit 1
fi

# Find the nearest previous rust-v* tag that is an ancestor of current.
# Previous rust-v* tag by creation time (newest before current, not equal to current)
prev_tag=$(git for-each-ref --sort=-creatordate --format='%(refname:short)' refs/tags \
  | awk -v cur="$TAG_NAME" '/^rust-v[0-9]/{ if ($0!=cur) { print $0; exit } }')

echo "Current tag : $TAG_NAME ($current_commit)" >&2
echo "Previous tag: ${prev_tag:-<none>}" >&2

if command -v git-cliff >/dev/null 2>&1; then
  if [[ -n "$prev_tag" ]]; then
    git cliff --config ./cliff-release.toml "$prev_tag..$TAG_NAME" --output "$OUT_FILE"
  else
    git cliff --config ./cliff-release.toml --tag "$TAG_NAME" --output "$OUT_FILE"
  fi
else
  # Fallback: simple log summary
  {
    echo "## $(echo "$TAG_NAME" | sed -E 's/^rust-v//')"
    echo
    if [[ -n "$prev_tag" ]]; then
      echo "### Changes"
      git log --no-merges --pretty='- %s (%h)' "$prev_tag..$TAG_NAME"
    else
      echo "(git-cliff not installed; showing log from initial commit to $TAG_NAME)"
      git log --no-merges --pretty='- %s (%h)' "$TAG_NAME"
    fi
  } >"$OUT_FILE"
fi

echo "Wrote $OUT_FILE" >&2
