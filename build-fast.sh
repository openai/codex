#!/usr/bin/env bash
# Fast build script for local development - optimized for speed
set -euo pipefail

# Usage banner
usage() {
  cat <<USAGE
Usage: ./build-fast.sh [env flags]

Environment flags:
  PROFILE=dev-fast|dev|release-prod   Build profile (default: dev-fast)
  TRACE_BUILD=1                       Print toolchain/env and artifact SHA
  KEEP_ENV=0                          Sanitize env for reproducible builds (default skips)
  DETERMINISTIC=1                     Add -C debuginfo=0; promotes to release-prod unless DETERMINISTIC_FORCE_RELEASE=0
  DETERMINISTIC_FORCE_RELEASE=0|1     Keep dev-fast (0) or switch to release-prod (1, default)
  DETERMINISTIC_NO_UUID=1             macOS only: strip LC_UUID on final executables
  BUILD_FAST_BINS="code code-tui"      Override bins to build (space or comma separated)
  --workspace codex|code|both         Select workspace to build (default: code)

Examples:
  ./build-fast.sh
  TRACE_BUILD=1 ./build-fast.sh
  DETERMINISTIC=1 DETERMINISTIC_FORCE_RELEASE=0 ./build-fast.sh
  DETERMINISTIC=1 DETERMINISTIC_NO_UUID=1 ./build-fast.sh
  ./build-fast.sh run
  ./build-fast.sh perf
  ./build-fast.sh perf run
USAGE
}

trim() {
  local value="${1:-}"
  value="${value#${value%%[![:space:]]*}}"
  value="${value%${value##*[![:space:]]}}"
  printf '%s' "$value"
}

hash_string() {
  local input="${1:-}"
  if command -v shasum >/dev/null 2>&1; then
    printf '%s' "$input" | shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$input" | sha256sum | awk '{print $1}'
  else
    python3 - <<'PY' 2>/dev/null
import hashlib, sys
data = sys.stdin.read().encode()
print(hashlib.sha256(data).hexdigest())
PY
  fi
}

sanitize_cache_key() {
  local raw="${1:-}"
  # Replace any unsupported characters with '-'
  raw="${raw//[^A-Za-z0-9._-]/-}"
  # Collapse repeated separators
  while [[ "$raw" == *--* ]]; do
    raw="${raw//--/-}"
  done
  # Trim leading and trailing dashes
  while [[ "$raw" == -* ]]; do
    raw="${raw#-}"
  done
  while [[ "$raw" == *- ]]; do
    raw="${raw%-}"
  done
  if [ -z "$raw" ]; then
    raw="default"
  fi
  # Prevent overly long directory names
  if [ "${#raw}" -gt 120 ]; then
    raw="${raw:0:120}"
  fi
  printf '%s' "$raw"
}

bin_requested() {
  local needle="${1:-}"
  for candidate in "${TARGET_BINS[@]}"; do
    if [ "${candidate}" = "${needle}" ]; then
      return 0
    fi
  done
  return 1
}

resolve_bin_path() {
  case "$PROFILE" in
    dev-fast)
      BIN_SUBDIR="dev-fast"
      ;;
    dev)
      BIN_SUBDIR="debug"
      ;;
    *)
      BIN_SUBDIR="$PROFILE"
      ;;
  esac

  local target_root
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    target_root="${CARGO_TARGET_DIR}"
  else
    target_root="${REPO_ROOT}/${WORKSPACE_DIR}/target"
  fi

  if [[ "${target_root}" != /* ]]; then
    target_root="$(cd "${target_root}" >/dev/null 2>&1 && pwd)"
  fi

  TARGET_DIR_ABS="${target_root}"
  BIN_CARGO_FILENAME="${CRATE_PREFIX}"
  BIN_FILENAME="${CRATE_PREFIX}"
  if [ "$PROFILE" = "perf" ]; then
    BIN_FILENAME="${CRATE_PREFIX}-perf"
  fi
  BIN_SUBPATH="${BIN_SUBDIR}/${BIN_FILENAME}"
  BIN_CARGO_SUBPATH="${BIN_SUBDIR}/${BIN_CARGO_FILENAME}"
  BIN_PATH="${TARGET_DIR_ABS}/${BIN_SUBPATH}"
  BIN_CARGO_PATH="${TARGET_DIR_ABS}/${BIN_CARGO_SUBPATH}"
  BIN_LINK_PATH="./target/${BIN_SUBPATH}"

  if [ -n "${REPO_TARGET_ABS:-}" ] && [ "${TARGET_DIR_ABS}" = "${REPO_TARGET_ABS}" ]; then
    BIN_DISPLAY_PATH="./${WORKSPACE_DIR}/target/${BIN_SUBPATH}"
  else
    BIN_DISPLAY_PATH="${BIN_PATH}"
  fi
}

case "${1:-}" in
  -h|--help) usage; exit 0 ;;
esac

RUN_AFTER_BUILD=0
ARG_PROFILE=""
WORKSPACE_CHOICE="${WORKSPACE:-}"
PASSTHROUGH_ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    run)
      RUN_AFTER_BUILD=1
      PASSTHROUGH_ARGS+=("$1")
      ;;
    --workspace)
      shift || { echo "Error: --workspace requires a value." >&2; usage; exit 1; }
      WORKSPACE_CHOICE="$1"
      ;;
    --workspace=*)
      WORKSPACE_CHOICE="${1#*=}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [ -n "$ARG_PROFILE" ]; then
        echo "Error: Multiple profile arguments provided ('${ARG_PROFILE}' and '$1')." >&2
        usage
        exit 1
      fi
      ARG_PROFILE="$1"
      PASSTHROUGH_ARGS+=("$1")
      ;;
  esac
  shift
done

if [ -z "$WORKSPACE_CHOICE" ]; then
  WORKSPACE_CHOICE="code"
fi

if [ "$WORKSPACE_CHOICE" = "both" ]; then
  if [ "$RUN_AFTER_BUILD" -eq 1 ]; then
    echo "Error: --workspace both cannot be combined with 'run'." >&2
    exit 1
  fi
  for ws in codex code; do
    WORKSPACE="$ws" "$0" "${PASSTHROUGH_ARGS[@]}" --workspace "$ws"
  done
  exit 0
fi

if [ "$ARG_PROFILE" = "pref" ]; then
  ARG_PROFILE="perf"
fi

# Resolve repository paths relative to this script so absolute invocation works
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
if [ -n "${CODE_CALLER_CWD:-}" ]; then
  if ! CALLER_CWD="$(cd "${CODE_CALLER_CWD}" >/dev/null 2>&1 && pwd)"; then
    echo "Error: CODE_CALLER_CWD is not a valid directory: ${CODE_CALLER_CWD}" >&2
    exit 1
  fi
else
  CALLER_CWD="$(pwd)"
fi

if [[ "${SCRIPT_DIR}" == */.code/working/*/branches/* ]]; then
  WORKTREE_PARENT="${SCRIPT_DIR%/branches/*}"
  REPO_NAME="$(basename "${WORKTREE_PARENT}")"
else
  REPO_NAME="$(basename "${SCRIPT_DIR}")"
fi

REPO_ROOT="${SCRIPT_DIR}"

if [ -n "${CODE_HOME:-}" ] && [ -n "${CODE_HOME}" ]; then
  CACHE_HOME="${CODE_HOME%/}"
elif [ -n "${CODEX_HOME:-}" ] && [ -n "${CODEX_HOME}" ]; then
  CACHE_HOME="${CODEX_HOME%/}"
else
  if [ -d "/mnt/data" ] && [ -w "/mnt/data" ]; then
    CACHE_HOME="/mnt/data/.code"
  else
    CACHE_HOME="${REPO_ROOT}/.code"
  fi
fi
case "${CACHE_HOME}" in
  /*) ;;
  *)
    CACHE_HOME="${REPO_ROOT}/${CACHE_HOME#./}"
    ;;
esac

case "$WORKSPACE_CHOICE" in
  codex|codex-rs)
    WORKSPACE_DIR="codex-rs"
    CRATE_PREFIX="codex"
    ;;
  code|code-rs)
    WORKSPACE_DIR="code-rs"
    CRATE_PREFIX="code"
    ;;
  *)
    echo "Error: Unknown workspace '${WORKSPACE_CHOICE}'. Use codex, code, or both." >&2
    exit 1
    ;;
esac

WORKSPACE_PATH="${SCRIPT_DIR}/${WORKSPACE_DIR}"
if [ ! -d "$WORKSPACE_PATH" ]; then
  echo "Error: Workspace directory '${WORKSPACE_PATH}' not found." >&2
  exit 1
fi

TARGET_CACHE_ROOT="${CACHE_HOME}/working/_target-cache/${REPO_NAME}"

# Change to the selected Rust workspace root regardless of caller CWD
cd "${WORKSPACE_PATH}"

WORKTREE_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
if [ -z "${BUILD_FAST_CACHE_KEY:-}" ]; then
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    BRANCH_NAME_RAW="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD)"
    if [ "${BRANCH_NAME_RAW}" = "HEAD" ]; then
      BRANCH_NAME_RAW="detached-$(git rev-parse --short HEAD 2>/dev/null || date +%Y%m%d%H%M%S)"
    fi
  else
    BRANCH_NAME_RAW="unknown"
  fi
  BRANCH_FINGERPRINT="$(hash_string "${BRANCH_NAME_RAW}")"
  BRANCH_HASH_SHORT="${BRANCH_FINGERPRINT:0:12}"
  WORKTREE_HASH="$(hash_string "${WORKTREE_ROOT}")"
  WORKTREE_HASH_SHORT="${WORKTREE_HASH:0:12}"
  CACHE_KEY_RAW="${BRANCH_NAME_RAW}-${BRANCH_HASH_SHORT}-${WORKTREE_HASH_SHORT}"
  CACHE_KEY_SOURCE="branch/worktree"
else
  CACHE_KEY_RAW="${BUILD_FAST_CACHE_KEY}"
  CACHE_KEY_SOURCE="override"
fi

CACHE_KEY="$(sanitize_cache_key "${CACHE_KEY_RAW}")"
if [ -z "${CACHE_KEY}" ]; then
  CACHE_KEY="default"
fi

TARGET_CACHE_DIR="${TARGET_CACHE_ROOT}/${CACHE_KEY}/${WORKSPACE_DIR}"

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  TARGET_CACHE_DIR_ABS="${TARGET_CACHE_DIR}"
  case "${TARGET_CACHE_DIR_ABS}" in
    /*) ;;
    *)
      TARGET_CACHE_DIR_ABS="${REPO_ROOT}/${TARGET_CACHE_DIR_ABS#./}"
      ;;
  esac
  mkdir -p "${TARGET_CACHE_DIR_ABS}" 2>/dev/null || true
  export CARGO_TARGET_DIR="${TARGET_CACHE_DIR_ABS}"
else
  TARGET_CACHE_DIR_ABS="${CARGO_TARGET_DIR}"
fi

echo "Cache bucket: ${CACHE_KEY} (${CACHE_KEY_SOURCE})"

CLI_PACKAGE="$(sed -n 's/^name\s*=\s*"\(.*\)"/\1/p' cli/Cargo.toml | head -n1)"
TUI_PACKAGE="$(sed -n 's/^name\s*=\s*"\(.*\)"/\1/p' tui/Cargo.toml | head -n1)"
EXEC_PACKAGE="$(sed -n 's/^name\s*=\s*"\(.*\)"/\1/p' exec/Cargo.toml | head -n1)"
CRATE_PREFIX="${CLI_PACKAGE%%-*}"
EXEC_BIN="$(awk 'BEGIN{inbin=0} /^\[\[bin\]\]/{inbin=1; next} inbin && /^name[[:space:]]*=/{gsub(/.*"/,"",$0); gsub(/"/,"",$0); print; exit}' exec/Cargo.toml)"
if [ -z "${EXEC_BIN}" ]; then
  EXEC_BIN="${EXEC_PACKAGE}"
fi

TARGET_BINS=()
if [ -n "${BUILD_FAST_BINS:-}" ]; then
  for raw_bin in ${BUILD_FAST_BINS//,/ }; do
    bin_candidate="$(trim "$raw_bin")"
    if [ -n "${bin_candidate}" ]; then
      TARGET_BINS+=("${bin_candidate}")
    fi
  done
fi
if [ "${#TARGET_BINS[@]}" -eq 0 ]; then
  TARGET_BINS=("${CRATE_PREFIX}")
fi
PRIMARY_PRESENT=0
for candidate in "${TARGET_BINS[@]}"; do
  if [ "${candidate}" = "${CRATE_PREFIX}" ]; then
    PRIMARY_PRESENT=1
    break
  fi
done
if [ "$PRIMARY_PRESENT" -eq 0 ]; then
  TARGET_BINS=("${CRATE_PREFIX}" "${TARGET_BINS[@]}")
fi
PRIMARY_BIN="${TARGET_BINS[0]}"

# Default to preserving caller environment unless explicitly disabled
KEEP_ENV="${KEEP_ENV:-1}"

# Track whether the caller explicitly set PROFILE (env or CLI)
PROFILE_ENV_SUPPLIED=0
if [ -n "${PROFILE+x}" ]; then
  PROFILE_ENV_SUPPLIED=1
  PROFILE_VALUE="$PROFILE"
else
  PROFILE_VALUE="dev-fast"
fi

if [ -n "$ARG_PROFILE" ]; then
  PROFILE_VALUE="$ARG_PROFILE"
fi

PROFILE_EXPLICIT=0
if [ "$PROFILE_ENV_SUPPLIED" -eq 1 ] || [ -n "$ARG_PROFILE" ]; then
  PROFILE_EXPLICIT=1
fi

PROFILE="$PROFILE_VALUE"

# Optional deterministic mode: aim for more stable hashes by removing
# UUIDs on macOS, disabling debuginfo, and preferring a single-codegen
# optimized profile when user hasn't explicitly chosen a profile.
if [ "${DETERMINISTIC:-}" = "1" ]; then
    echo "Deterministic build: enabled"
    DET_FORCE_REL="${DETERMINISTIC_FORCE_RELEASE:-1}"
    if [ "$PROFILE" = "dev-fast" ] && [ "$DET_FORCE_REL" = "1" ]; then
        PROFILE="release-prod"
        echo "Deterministic build: switching profile to ${PROFILE}"
    elif [ "$PROFILE" = "dev-fast" ]; then
        echo "Deterministic build: keeping profile ${PROFILE} (DETERMINISTIC_FORCE_RELEASE=0)"
    fi
    # Use SOURCE_DATE_EPOCH if available to stabilize timestamps
    if command -v git >/dev/null 2>&1 && git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        export SOURCE_DATE_EPOCH="$(git -C "$REPO_ROOT" log -1 --pretty=%ct 2>/dev/null || true)"
    fi
    # Keep debuginfo intact so profiling tools can resolve symbols.
fi

ORIGINAL_PROFILE="$PROFILE"
if [ "$PROFILE" != "dev" ] && [ "$PROFILE" != "release" ]; then
  if ! grep -F "[profile.${PROFILE}]" Cargo.toml >/dev/null 2>&1; then
    case "$PROFILE" in
      dev-fast)
        PROFILE="dev"
        ;;
      perf|release-prod)
        PROFILE="release"
        ;;
      *)
        PROFILE="dev"
        ;;
    esac
    if [ "$ORIGINAL_PROFILE" != "$PROFILE" ]; then
      echo "Profile ${ORIGINAL_PROFILE} not defined in ${WORKSPACE_DIR}/Cargo.toml; falling back to ${PROFILE}."
    fi
  fi
fi

# Select the cargo/rustc toolchain to match deploy
# Prefer rustup with the toolchain pinned in rust-toolchain.toml or $RUSTUP_TOOLCHAIN
USE_CARGO="cargo"
if command -v rustup >/dev/null 2>&1; then
  # Determine desired toolchain
  if [ -n "${RUSTUP_TOOLCHAIN:-}" ]; then
    TOOLCHAIN="$RUSTUP_TOOLCHAIN"
  else
    # Try parse channel from rust-toolchain.toml in repo root
    if [ -f "rust-toolchain.toml" ]; then
      TOOLCHAIN=$(sed -n 's/^channel\s*=\s*"\(.*\)"/\1/p' rust-toolchain.toml | head -n1)
    fi
    # Fallback to active default if none found
    TOOLCHAIN="${TOOLCHAIN:-$(rustup show active-toolchain 2>/dev/null | awk '{print $1}')}"
  fi

  if [ -n "$TOOLCHAIN" ]; then
    # Ensure toolchain is installed; if not, attempt install quietly
    if ! rustup which rustc --toolchain "$TOOLCHAIN" >/dev/null 2>&1; then
      echo "rustup: installing toolchain $TOOLCHAIN ..."
      rustup toolchain install "$TOOLCHAIN" >/dev/null 2>&1 || true
    fi
    USE_CARGO="rustup run $TOOLCHAIN cargo"
    echo "Using rustup toolchain: $TOOLCHAIN"
    rustup run "$TOOLCHAIN" rustc --version || true
  else
    echo "rustup found but no toolchain detected; using system cargo"
  fi
else
  echo "Error: rustup is required for consistent builds."
  echo "Please install rustup: https://rustup.rs/"
  exit 1
fi

# Canonicalize build environment only when requested.
# Set KEEP_ENV=0 to force sanitization.
if [ "${KEEP_ENV}" != "1" ]; then
  # Only define RUSTFLAGS via feature flags like DETERMINISTIC=1; otherwise clear.
  if [ -z "${DETERMINISTIC:-}" ]; then
    export RUSTFLAGS=""
  fi
  unset RUSTC_WRAPPER CARGO_BUILD_RUSTC_WRAPPER SCCACHE SCCACHE_BIN CARGO_TARGET_DIR MACOSX_DEPLOYMENT_TARGET
  unset CARGO_PROFILE_RELEASE_LTO CARGO_PROFILE_DEV_FAST_LTO CARGO_PROFILE_RELEASE_CODEGEN_UNITS CARGO_PROFILE_DEV_FAST_CODEGEN_UNITS
  # Ensure incremental uses profile default
  unset CARGO_INCREMENTAL
  CANONICAL_ENV_APPLIED=1
else
  CANONICAL_ENV_APPLIED=0
fi

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="${TARGET_CACHE_DIR_ABS}"
fi

if command -v sccache >/dev/null 2>&1; then
  if [ -z "${RUSTC_WRAPPER:-}" ]; then
    export RUSTC_WRAPPER="$(command -v sccache)"
  fi
  if [ -z "${SCCACHE_DIR:-}" ]; then
    export SCCACHE_DIR="${CACHE_HOME}/sccache"
  fi
  if [ -z "${SCCACHE_CACHE_SIZE:-}" ]; then
    export SCCACHE_CACHE_SIZE="50G"
  fi
  mkdir -p "${SCCACHE_DIR}" 2>/dev/null || true
fi

# Optional debug symbol override for profiling sessions
if [ "${DEBUG_SYMBOLS:-}" = "1" ]; then
  if [ "$PROFILE" = "perf" ]; then
    echo "Debug symbols: profile 'perf' already preserves debuginfo"
  elif [ "$PROFILE_EXPLICIT" -eq 0 ] && [ "$PROFILE" = "dev-fast" ]; then
    echo "Debug symbols requested: switching profile to perf"
    PROFILE="perf"
  else
    PROFILE_ENV_KEY="$(printf "%s" "$PROFILE" | tr '[:lower:]-' '[:upper:]_')"
    DEBUG_VAR="CARGO_PROFILE_${PROFILE_ENV_KEY}_DEBUG"
    STRIP_VAR="CARGO_PROFILE_${PROFILE_ENV_KEY}_STRIP"
    SPLIT_VAR="CARGO_PROFILE_${PROFILE_ENV_KEY}_SPLIT_DEBUGINFO"
    printf -v "$DEBUG_VAR" '%s' '2'
    printf -v "$STRIP_VAR" '%s' 'none'
    printf -v "$SPLIT_VAR" '%s' 'packed'
    export "$DEBUG_VAR" "$STRIP_VAR" "$SPLIT_VAR"
    echo "Debug symbols: forcing debuginfo for profile ${PROFILE}"
  fi

  if [ -n "${RUSTFLAGS:-}" ]; then
    CLEAN_RUSTFLAGS="${RUSTFLAGS//-C debuginfo=0/}"
    CLEAN_RUSTFLAGS="${CLEAN_RUSTFLAGS//  / }"
    CLEAN_RUSTFLAGS="${CLEAN_RUSTFLAGS## }"
    CLEAN_RUSTFLAGS="${CLEAN_RUSTFLAGS%% }"
    export RUSTFLAGS="${CLEAN_RUSTFLAGS}"
  fi

  export CARGO_PROFILE_RELEASE_STRIP="none"
  export CARGO_PROFILE_RELEASE_PROD_STRIP="none"
fi

echo "Building ${CRATE_PREFIX} binary (${PROFILE} mode)..."

# Ensure Cargo cache locations are stable.
# In CI, we can optionally enforce a specific CARGO_HOME regardless of caller env
# by setting STRICT_CARGO_HOME=1 (used by Issue Triage workflow to keep caching deterministic).
# Cargo/rustup home directories
# When STRICT_CARGO_HOME=1, honor CARGO_HOME_ENFORCED if provided; otherwise
# force a per-repo location to avoid HOME permission/caching issues on CI.
if [ "${STRICT_CARGO_HOME:-}" = "1" ]; then
  export CARGO_HOME="${CARGO_HOME_ENFORCED:-${REPO_ROOT}/.cargo-home}"
else
  # Default: write inside workspace if not set to avoid HOME permission issues
  if [ -z "${CARGO_HOME:-}" ]; then
    export CARGO_HOME="${REPO_ROOT}/.cargo-home"
  fi
fi
# Keep rustup’s home alongside Cargo by default when not explicitly set
if [ -z "${RUSTUP_HOME:-}" ]; then
  export RUSTUP_HOME="${CARGO_HOME%/}/rustup"
fi
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  export CARGO_TARGET_DIR="${WORKSPACE_PATH}/target"
fi
mkdir -p "${CARGO_HOME}" "${CARGO_TARGET_DIR}" 2>/dev/null || true
# Ensure repo-local target directory exists for compatibility symlinks
mkdir -p ./target
REPO_TARGET_ABS="$(cd ./target >/dev/null 2>&1 && pwd)"
resolve_bin_path
# Use sparse registry for faster index updates when available
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"

# Resolve actual cargo/rustc binaries that will be used (via rustup) for fingerprinting
REAL_CARGO_BIN="$(rustup which cargo 2>/dev/null || command -v cargo || echo cargo)"
REAL_RUSTC_BIN="$(rustup which rustc 2>/dev/null || command -v rustc || echo rustc)"

# Determine current host triple for dynamic symlink naming
HOST_TRIPLE="$(rustup run "$TOOLCHAIN" rustc -vV 2>/dev/null | awk -F': ' '/^host: /{print $2}')"
TRIPLE="${HOST_TRIPLE:-}"
if [ -z "$TRIPLE" ]; then
  # Fallback for Darwin when rustup is unavailable for some reason
  if [ "$(uname -s)" = "Darwin" ]; then
    TRIPLE="$(uname -m)-apple-darwin"
    [ "$TRIPLE" = "arm64-apple-darwin" ] && TRIPLE="aarch64-apple-darwin"
  else
    TRIPLE="unknown-unknown-unknown"
  fi
fi

# Check Cargo.lock validity (fast, non-blocking check) using the selected cargo
if ! CARGO_HOME="$CARGO_HOME" RUSTUP_HOME="$RUSTUP_HOME" ${USE_CARGO} metadata --locked --format-version 1 >/dev/null 2>&1; then
    echo "⚠️  Warning: Cargo.lock appears out of date or inconsistent"
    echo "  This might mean:"
    echo "  • You've modified Cargo.toml dependencies"
    echo "  • You've changed workspace crate versions"
    echo "  • The lockfile is missing entries"
    echo ""
    echo "  Run 'cargo update' to update all dependencies, or"
    echo "  Run 'cargo update -p <crate-name>' to update specific crates"
    echo ""
    echo "  Continuing with unlocked build for development..."
    echo ""
    USE_LOCKED=""
else
    # Lockfile is valid, use it for consistent builds
    USE_LOCKED="--locked"
fi

# Optional trace mode for diagnosing environment differences
if [ "${TRACE_BUILD:-}" = "1" ]; then
  echo "--- TRACE_BUILD environment ---"
  echo "whoami: $(whoami)"
  echo "pwd: $(pwd)"
  echo "SHELL: ${SHELL:-}"
  bash --version | head -n1 || true
  if [ -n "${TOOLCHAIN:-}" ]; then
    echo "TOOLCHAIN: ${TOOLCHAIN}"
    rustup run "$TOOLCHAIN" rustc -vV || true
    rustup run "$TOOLCHAIN" cargo -vV || true
  fi
  echo "CANONICAL_ENV_APPLIED: ${CANONICAL_ENV_APPLIED} (KEEP_ENV=${KEEP_ENV})"
  echo "Filtered env (CARGO|RUST*|PROFILE|CODE_HOME|CODEX_HOME):"
  env | egrep '^(CARGO|RUST|RUSTUP|PROFILE|CODE_HOME|CODEX_HOME)=' | sort || true
  echo "--------------------------------"
fi

# Compute and compare build cache fingerprint to explain incremental behavior
FPRINT_FILE="./target/${PROFILE}/.env-fingerprint"
# Collect fingerprint inputs (only env/toolchain/settings that affect codegen/caches)
collect_fingerprint() {
  local cargo_v rustc_v host which_cargo which_rustc uname_srm
  cargo_v="$(CARGO_HOME="$CARGO_HOME" RUSTUP_HOME="$RUSTUP_HOME" ${USE_CARGO} -V 2>/dev/null || true)"
  rustc_v="$(rustup run "$TOOLCHAIN" rustc -vV 2>/dev/null || true)"
  host="$(printf "%s\n" "$rustc_v" | awk -F': ' '/^host: /{print $2}' || true)"
  which_cargo="${REAL_CARGO_BIN}"
  which_rustc="${REAL_RUSTC_BIN}"
  uname_srm="$(uname -srm 2>/dev/null || true)"
  cat <<FP
profile=${PROFILE}
toolchain=${TOOLCHAIN:-}
host=${host}
cargo_bin=${which_cargo}
rustc_bin=${which_rustc}
cargo_version=${cargo_v}
rustc_version=$(printf "%s" "$rustc_v" | tr '\n' ' ')
uname=${uname_srm}
RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-}
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-}
RUSTFLAGS=${RUSTFLAGS:-}
RUSTC_WRAPPER=${RUSTC_WRAPPER:-}
CARGO_BUILD_RUSTC_WRAPPER=${CARGO_BUILD_RUSTC_WRAPPER:-}
SCCACHE=${SCCACHE:-}
SCCACHE_BIN=${SCCACHE_BIN:-}
CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-}
MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-}
CODE_HOME=${CODE_HOME:-}
CODEX_HOME=${CODEX_HOME:-}
FP
}

NEW_FPRINT_TEXT="$(collect_fingerprint)"
NEW_FPRINT_HASH="$(printf "%s" "$NEW_FPRINT_TEXT" | shasum -a 256 2>/dev/null | awk '{print $1}')"

FPRINT_CHANGED="0"
if [ -f "$FPRINT_FILE" ]; then
  OLD_FPRINT_HASH="$(sed -n 's/^HASH=//p' "$FPRINT_FILE" 2>/dev/null | head -n1)"
  if [ "${OLD_FPRINT_HASH:-}" != "$NEW_FPRINT_HASH" ]; then
    FPRINT_CHANGED="1"
    echo "⚠️  Build cache fingerprint changed since last run for profile '${PROFILE}'."
    echo "   This can trigger incremental rebuilds the first time you build."
    if [ "${TRACE_BUILD:-}" = "1" ]; then
      echo "--- previous fingerprint (hash: ${OLD_FPRINT_HASH:-none}) ---"; sed -n '1,200p' "$FPRINT_FILE" 2>/dev/null | sed '1d'; echo "--------------------------------"
      echo "--- current fingerprint (hash: ${NEW_FPRINT_HASH}) ---"; printf "%s\n" "$NEW_FPRINT_TEXT"; echo "--------------------------------"
    else
      echo "   Run with TRACE_BUILD=1 to see detailed differences."
    fi
  fi
fi

# Build for native target (no --target flag) for maximum speed
# This reuses the host stdlib and normal cache

# Determine exec binary name based on workspace metadata
if [ -z "${EXEC_BIN}" ]; then
  EXEC_BIN="${CRATE_PREFIX}-exec"
fi

# Build with or without --locked based on lockfile validity
# Keep stderr and stdout separate so downstream tools can capture both streams.
CARGO_BIN_ARGS=()
for bin in "${TARGET_BINS[@]}"; do
  CARGO_BIN_ARGS+=("--bin" "${bin}")
done
echo "Building bins: ${TARGET_BINS[*]}"
${USE_CARGO} build ${USE_LOCKED} --profile "${PROFILE}" "${CARGO_BIN_ARGS[@]}"

# Check if build succeeded
if [ $? -eq 0 ]; then
    resolve_bin_path

    if [ "$PROFILE" = "perf" ]; then
      PERF_SOURCE="${BIN_CARGO_PATH}"
      PERF_TARGET="${BIN_PATH}"
      if [ -e "${PERF_SOURCE}" ]; then
        PERF_DIR="$(dirname "${PERF_TARGET}")"
        mkdir -p "${PERF_DIR}"
        if [ -e "${PERF_TARGET}" ] || [ -L "${PERF_TARGET}" ]; then
          rm -f "${PERF_TARGET}"
        fi
        (
          cd "${PERF_DIR}" >/dev/null 2>&1
          ln -sf "$(basename "${PERF_SOURCE}")" "$(basename "${PERF_TARGET}")"
        )
      fi
    fi

    echo "✅ Build successful!"
    echo "Binary location: ${BIN_DISPLAY_PATH}"
    echo ""

    # Keep old symlink locations working for compatibility
    # Create symlink in target/release for npm wrapper expectations
    release_link_target="../${BIN_SUBDIR}/${BIN_FILENAME}"
    dev_fast_link_target="../${BIN_SUBDIR}/${BIN_FILENAME}"

    SYMLINK_PREFIXES=("${CRATE_PREFIX}")
    if [ "${CRATE_PREFIX}" = "code" ]; then
      SYMLINK_PREFIXES+=("coder")
    fi

    create_cli_symlinks() {
      local cli_dir="$1"
      local default_target="$2"
      mkdir -p "$cli_dir"
      local link_target="$default_target"
      if [ -n "${CLI_LINK_ABSOLUTE}" ]; then
        link_target="${CLI_LINK_ABSOLUTE}"
      fi
      for PREFIX in "${SYMLINK_PREFIXES[@]}"; do
        local dest="${cli_dir}/${PREFIX}-${TRIPLE}"
        [ -e "$dest" ] && rm -f "$dest"
        ln -sf "${link_target}" "$dest"
      done
      for PREFIX in "${SYMLINK_PREFIXES[@]}"; do
        local dest="${cli_dir}/${PREFIX}-aarch64-apple-darwin"
        [ -e "$dest" ] && rm -f "$dest"
        ln -sf "${link_target}" "$dest"
      done
    }

    CLI_TARGET_CODE="../../target/${BIN_SUBDIR}/${BIN_FILENAME}"
    CLI_TARGET_CODEX="../../${WORKSPACE_DIR}/target/${BIN_SUBDIR}/${BIN_FILENAME}"

    CLI_LINK_ABSOLUTE=""
    if [ "${TARGET_DIR_ABS}" != "${REPO_TARGET_ABS}" ]; then
      release_link_target="${BIN_PATH}"
      dev_fast_link_target="${BIN_PATH}"
      CLI_LINK_ABSOLUTE="${BIN_PATH}"

      # Maintain repo-local path for downstream tooling when target dir is external
      if [ -n "${BIN_LINK_PATH:-}" ]; then
        mkdir -p "$(dirname "${BIN_LINK_PATH}")"
        if [ -e "${BIN_LINK_PATH}" ]; then
          rm -f "${BIN_LINK_PATH}"
        fi
        ln -sf "${BIN_PATH}" "${BIN_LINK_PATH}"
      fi
    fi

    mkdir -p ./target/release
    if [ -e "./target/release/${CRATE_PREFIX}" ]; then
        rm -f "./target/release/${CRATE_PREFIX}"
    fi
    ln -sf "${release_link_target}" "./target/release/${CRATE_PREFIX}"

    # Update the symlinks in CLI wrapper directories
    if [ -d "../codex-cli/bin" ]; then
      create_cli_symlinks "../codex-cli/bin" "${CLI_TARGET_CODEX}"
    fi
    if [ -d "./code-cli/bin" ]; then
      create_cli_symlinks "./code-cli/bin" "${CLI_TARGET_CODE}"
    fi

    BIN_DIR="./bin"
    mkdir -p "${BIN_DIR}"
    BIN_DIR_ABS="$(cd "${BIN_DIR}" >/dev/null 2>&1 && pwd)"
    for BIN_NAME in "${TARGET_BINS[@]}"; do
      BIN_TARGET_PATH="${TARGET_DIR_ABS}/${BIN_SUBDIR}/${BIN_NAME}"
      if [ -e "${BIN_TARGET_PATH}" ]; then
        TMP_BIN_PATH="${BIN_DIR}/${BIN_NAME}.tmp"
        cp -f "${BIN_TARGET_PATH}" "${TMP_BIN_PATH}"
        mv -f "${TMP_BIN_PATH}" "${BIN_DIR}/${BIN_NAME}"
        chmod +x "${BIN_DIR}/${BIN_NAME}" 2>/dev/null || true
      fi
    done
    RUN_BIN_PATH="${BIN_DIR_ABS}/${PRIMARY_BIN}"

    # Ensure repo-local developer alias stays mapped to latest build output
    # so the user's `${CRATE_PREFIX}-dev` alias keeps working when pointing at target/dev-fast/${CRATE_PREFIX}
    # Only create this symlink if we're not already building in dev-fast profile
    if [ "$PROFILE" != "dev-fast" ]; then
      mkdir -p ./target/dev-fast
      if [ -e "./target/dev-fast/${CRATE_PREFIX}" ]; then
        rm -f "./target/dev-fast/${CRATE_PREFIX}"
      fi
      ln -sf "${dev_fast_link_target}" "./target/dev-fast/${CRATE_PREFIX}"
    fi

    # Optional post-link step for deterministic builds: re-link executables
    # with -no_uuid only on macOS. Apply per-bin via `cargo rustc` so
    # dependencies/proc-macro dylibs are not affected.
    if [ "${DETERMINISTIC_NO_UUID:-}" = "1" ] && [ "$(uname -s)" = "Darwin" ]; then
      echo "Deterministic post-link: removing LC_UUID from executables"
      if bin_requested "${CRATE_PREFIX}"; then
        ${USE_CARGO} rustc ${USE_LOCKED} --profile "${PROFILE}" -p "${CLI_PACKAGE}" --bin "${CRATE_PREFIX}" -- -C link-arg=-Wl,-no_uuid || true
      fi
      if bin_requested "${CRATE_PREFIX}-tui"; then
        ${USE_CARGO} rustc ${USE_LOCKED} --profile "${PROFILE}" -p "${TUI_PACKAGE}" --bin "${CRATE_PREFIX}-tui" -- -C link-arg=-Wl,-no_uuid || true
      fi
      if bin_requested "${EXEC_BIN}"; then
        ${USE_CARGO} rustc ${USE_LOCKED} --profile "${PROFILE}" -p "${EXEC_PACKAGE}" --bin "${EXEC_BIN}" -- -C link-arg=-Wl,-no_uuid || true
      fi
    fi

    # Compute absolute path and SHA256 for clarity (after any post-linking)
    ABS_BIN_PATH="${BIN_PATH}"
    if [[ "${ABS_BIN_PATH}" != /* ]]; then
      ABS_BIN_PATH="$(cd "$(dirname "${ABS_BIN_PATH}")" >/dev/null 2>&1 && pwd)/$(basename "${ABS_BIN_PATH}")"
    fi

    BIN_SHA=""
    if [ -e "${ABS_BIN_PATH}" ]; then
      if command -v shasum >/dev/null 2>&1; then
        BIN_SHA="$(shasum -a 256 "${ABS_BIN_PATH}" | awk '{print $1}')"
      elif command -v sha256sum >/dev/null 2>&1; then
        BIN_SHA="$(sha256sum "${ABS_BIN_PATH}" | awk '{print $1}')"
      fi
    fi

    if [ -n "$BIN_SHA" ]; then
      echo "Binary Hash: ${BIN_SHA} ($(du -sh "${ABS_BIN_PATH}" | awk '{print $1}'))"
    elif [ -e "${ABS_BIN_PATH}" ]; then
      echo "Binary Size: $(du -h "${ABS_BIN_PATH}" | cut -f1)"
    else
      echo "Binary artifact not found at ${ABS_BIN_PATH}"
    fi

    if [ "$RUN_AFTER_BUILD" -eq 1 ]; then
      RUN_PATH="${RUN_BIN_PATH}"
      if [ ! -x "${RUN_PATH}" ]; then
        RUN_PATH="${TARGET_DIR_ABS}/${BIN_SUBDIR}/${PRIMARY_BIN}"
      fi
      if [ ! -x "${RUN_PATH}" ]; then
        echo "❌ Run failed: ${RUN_PATH} is missing or not executable"
        exit 1
      fi
      echo "Running ${RUN_PATH} (cwd: ${CALLER_CWD})..."
      (
        cd "${CALLER_CWD}" && "${RUN_PATH}"
      )
      RUN_STATUS=$?
      if [ $RUN_STATUS -ne 0 ]; then
        echo "❌ Run failed with status ${RUN_STATUS}"
        exit $RUN_STATUS
      fi
    fi

    if [ "${TRACE_BUILD:-}" = "1" ] && [ -n "$BIN_SHA" ]; then
      echo "--- TRACE_BUILD artifact ---"
      echo "ABS_BIN_PATH: ${ABS_BIN_PATH}"
      echo "SHA256: ${BIN_SHA}"
      echo "--------------------------------"
    fi

    # Persist the current fingerprint to explain future behavior
    mkdir -p "./target/${PROFILE}" 2>/dev/null || true
    {
      echo "HASH=${NEW_FPRINT_HASH}"
      printf "%s\n" "$NEW_FPRINT_TEXT"
    } >"$FPRINT_FILE"
    if [ "$FPRINT_CHANGED" = "1" ]; then
      echo "🧰 Cache normalized to current environment (fingerprint ${NEW_FPRINT_HASH})."
    fi

    # If lockfile was out of date, remind user
    if [ -z "$USE_LOCKED" ]; then
        echo ""
        echo "⚠️  Remember: Built without --locked due to Cargo.lock issues"
        echo "  Consider running 'cargo update' and committing the changes"
    fi
else
    echo "❌ Build failed"
    exit 1
fi
