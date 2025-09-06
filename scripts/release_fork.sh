#!/usr/bin/env bash
set -euo pipefail

# Release helper for fork versions.
# - Computes next fork version based on baseline and existing tags
# - Delegates to codex-rs/scripts/create_github_release.sh to update Cargo.toml,
#   create the release commit, tag, and push the tag to origin
#
# Usage examples:
#   scripts/release_fork.sh                   # auto-detect baseline, compute next -fork.N, release from main
#   scripts/release_fork.sh --baseline 0.30.0 # override baseline
#   scripts/release_fork.sh --version 0.30.0-fork.2  # fully explicit
#   scripts/release_fork.sh --dry-run         # preview only

DRY_RUN="false"
BRANCH="main"
BASELINE=""
EXPLICIT_VERSION=""

usage() {
  cat <<EOF
Usage: $(basename "$0") [--baseline X.Y.Z] [--version X.Y.Z-fork.N] [--branch main] [--dry-run]

Options
  --baseline X.Y.Z   Baseline upstream version (numeric). Defaults to numeric part of codex-rs/Cargo.toml
  --version  V       Full version to release (e.g. 0.30.0-fork.2). Overrides --baseline.
  --branch   NAME    Branch to release from (default: main)
  --dry-run          Preview the computed version and actions without making changes
  -h, --help         Show this help

Notes
  - Runs codex-rs/scripts/create_github_release.sh under the hood.
  - That script enforces: clean tree, on 'main', and that HEAD is present on origin/main.
    If you pass --branch other than main, switch manually before running.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline)
      shift; BASELINE="${1:-}" || true ;;
    --version)
      shift; EXPLICIT_VERSION="${1:-}" || true ;;
    --branch)
      shift; BRANCH="${1:-}" || true ;;
    --dry-run)
      DRY_RUN="true" ;;
    -h|--help)
      usage; exit 0 ;;
    --*)
      echo "Unknown option: $1" >&2; exit 1 ;;
    *)
      echo "Unexpected argument: $1" >&2; usage; exit 1 ;;
  esac
  shift || true
done

# Helper: read current Cargo.toml version numeric part
read_numeric_version_from_cargo() {
  local ver
  ver=$(grep -m1 '^version' codex-rs/Cargo.toml | sed -E 's/version *= *"([^"]+)".*/\1/')
  # Extract numeric x.y.z part
  echo "$ver" | sed -E 's/^([0-9]+\.[0-9]+\.[0-9]+).*$/\1/'
}

# Helper: compute next fork number given baseline X.Y.Z
next_fork_version() {
  local base="$1"
  local last_n
  last_n=$(git tag --list "rust-v${base}-fork.*" \
              | sed -E 's/.*-fork\.([0-9]+)$/\1/' \
              | sort -n | tail -n1)
  if [[ -z "$last_n" ]]; then
    echo "${base}-fork.1"
  else
    echo "${base}-fork.$((last_n+1))"
  fi
}

# Helper: detect a sensible baseline X.Y.Z
detect_baseline() {
  local from_base_tag
  from_base_tag=$(git tag --list 'base-rust-v*' \
                    | sed -E 's/^base-rust-v(.*)$/\1/' \
                    | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' \
                    | sort -V | tail -n1)
  if [[ -n "$from_base_tag" ]]; then
    echo "$from_base_tag"; return 0
  fi
  local from_rust_tag
  from_rust_tag=$(git tag --list 'rust-v*' \
                    | sed -E 's/^rust-v(.*)$/\1/' \
                    | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' \
                    | sort -V | tail -n1)
  if [[ -n "$from_rust_tag" ]]; then
    echo "$from_rust_tag"; return 0
  fi
  read_numeric_version_from_cargo
}

# Validate environment
if [[ -z "$EXPLICIT_VERSION" ]]; then
  if [[ -z "$BASELINE" ]]; then
    BASELINE="$(detect_baseline)"
  fi
  if ! [[ "$BASELINE" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: Could not determine a valid baseline (got '${BASELINE}'). Use --baseline X.Y.Z or --version X.Y.Z-fork.N" >&2
    exit 1
  fi
  TARGET_VERSION="$(next_fork_version "$BASELINE")"
else
  TARGET_VERSION="$EXPLICIT_VERSION"
fi

echo "Release plan:"
echo "  Branch         : ${BRANCH}"
echo "  Target version : ${TARGET_VERSION}"
echo "  Tag            : rust-v${TARGET_VERSION}"

if [[ "$DRY_RUN" == "true" ]]; then
  echo "[dry-run] Would run: (cd codex-rs && ./scripts/create_github_release.sh ${TARGET_VERSION})"
  exit 0
fi

# Ensure we are on the right branch before delegating (create_github_release.sh will re-check)
current_branch=$(git symbolic-ref --short -q HEAD 2>/dev/null || true)
if [[ "$current_branch" != "$BRANCH" ]]; then
  echo "ERROR: Current branch is '$current_branch'. Switch to '$BRANCH' before releasing (or pass --branch)." >&2
  exit 1
fi

(
  cd codex-rs
  ./scripts/create_github_release.sh "${TARGET_VERSION}"
)

echo "Release tag pushed: rust-v${TARGET_VERSION}"
echo "GitHub Actions should start the release workflow shortly."
