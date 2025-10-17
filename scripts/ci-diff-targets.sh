#!/usr/bin/env bash
# Detects changed workspaces inside a monorepo to drive selective CI builds.
# Usage: ./scripts/ci-diff-targets.sh origin/main HEAD

set -euo pipefail

BASE_REF=${1:-origin/main}
HEAD_REF=${2:-HEAD}

changed_paths=$(git diff --name-only "${BASE_REF}" "${HEAD_REF}" || true)

if [[ -z "${changed_paths}" ]]; then
  echo "No changes detected; defaulting to full test matrix." >&2
  exit 0
fi

targets=()
while IFS= read -r path; do
  case "${path}" in
    apps/web/*)
      targets+=("web")
      ;;
    apps/api/*)
      targets+=("api")
      ;;
    codex-rs/*|codex-cli/*)
      targets+=("shared")
      ;;
    *)
      targets+=("full")
      ;;
  esac
done <<< "${changed_paths}"

# Collapse duplicates while preserving order.
unique_targets=()
for target in "${targets[@]}"; do
  if [[ ! " ${unique_targets[*]} " =~ " ${target} " ]]; then
    unique_targets+=("${target}")
  fi
done

printf '%s\n' "${unique_targets[@]}"
