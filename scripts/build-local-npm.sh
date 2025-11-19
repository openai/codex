#!/usr/bin/env bash
# Build the local codex-super npm package from source, including the native binary.
# Usage: ./scripts/build-local-npm.sh [version] [output-tgz]
set -euo pipefail

VERSION="${1:-0.0.0-local}"
OUTPUT="${2:-}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

CODEX_RS_DIR="${REPO_ROOT}/codex-rs"
CODEX_CLI_DIR="${REPO_ROOT}/codex-cli"
DIST_DIR="${REPO_ROOT}/dist"

mkdir -p "${DIST_DIR}"

if [[ -z "${OUTPUT}" ]]; then
  OUTPUT="${DIST_DIR}/codex-super-${VERSION}.tgz"
fi

echo "==> Building codex CLI (cargo release)…"
(
  cd "${CODEX_RS_DIR}"
  cargo build -p codex-cli --release
)

HOST_TRIPLE="$(rustc -vV | awk '/host:/ {print $2}')"
echo "==> Host target detected: ${HOST_TRIPLE}"

BIN_NAME="codex"
if [[ "${HOST_TRIPLE}" == *"windows"* ]]; then
  BIN_NAME="codex.exe"
fi

SOURCE_BIN="${CODEX_RS_DIR}/target/release/${BIN_NAME}"
if [[ ! -f "${SOURCE_BIN}" ]]; then
  echo "error: expected compiled binary at ${SOURCE_BIN}" >&2
  exit 1
fi

VENDOR_TARGET_DIR="${CODEX_CLI_DIR}/vendor/${HOST_TRIPLE}"
CODEX_VENDOR_DIR="${VENDOR_TARGET_DIR}/codex"
RG_VENDOR_DIR="${VENDOR_TARGET_DIR}/path"
mkdir -p "${CODEX_VENDOR_DIR}" "${RG_VENDOR_DIR}"

echo "==> Installing codex binary into vendor/${HOST_TRIPLE}/codex/"
cp "${SOURCE_BIN}" "${CODEX_VENDOR_DIR}/${BIN_NAME}"
chmod +x "${CODEX_VENDOR_DIR}/${BIN_NAME}"

LINUX_TARGET="x86_64-unknown-linux-musl"
LINUX_VENDOR_DIR="${CODEX_CLI_DIR}/vendor/${LINUX_TARGET}/codex"
LINUX_VENDOR_BIN="${LINUX_VENDOR_DIR}/codex"

update_linux_binary() {
  if [[ "${HOST_TRIPLE}" == *"windows"* ]]; then
    echo "==> Skipping Linux binary build on Windows host"
    return 0
  fi

  if ! rustup target list --installed | grep -q "^${LINUX_TARGET}$"; then
    echo "error: rust target '${LINUX_TARGET}' not installed. Run 'rustup target add ${LINUX_TARGET}' first." >&2
    exit 1
  fi

  mkdir -p "${LINUX_VENDOR_DIR}"

  MUSL_LINKER="${MUSL_LINKER:-x86_64-linux-musl-gcc}"
  MUSL_CXX="${MUSL_CXX:-x86_64-linux-musl-g++}"
  MUSL_AR="${MUSL_AR:-x86_64-linux-musl-ar}"

  if ! command -v "${MUSL_LINKER}" >/dev/null 2>&1; then
    cat >&2 <<EOF
error: '${MUSL_LINKER}' not found. Install the musl cross toolchain (e.g. 'brew install FiloSottile/musl-cross/musl-cross') or set MUSL_LINKER to the desired linker.
EOF
    exit 1
  fi

  if ! command -v "${MUSL_CXX}" >/dev/null 2>&1; then
    cat >&2 <<EOF
error: '${MUSL_CXX}' not found. Install the musl cross toolchain or set MUSL_CXX appropriately.
EOF
    exit 1
  fi

  if ! command -v "${MUSL_AR}" >/dev/null 2>&1; then
    cat >&2 <<EOF
error: '${MUSL_AR}' not found. Install the musl cross toolchain or set MUSL_AR appropriately.
EOF
    exit 1
  fi

  echo "==> Building codex CLI (${LINUX_TARGET} release)…"
  (
    cd "${CODEX_RS_DIR}"
    CC_x86_64_unknown_linux_musl="${MUSL_LINKER}" \
    CXX_x86_64_unknown_linux_musl="${MUSL_CXX}" \
    AR_x86_64_unknown_linux_musl="${MUSL_AR}" \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="${MUSL_LINKER}" \
    cargo build -p codex-cli --release --target "${LINUX_TARGET}"
  )

  LINUX_SOURCE_BIN="${CODEX_RS_DIR}/target/${LINUX_TARGET}/release/codex"
  if [[ ! -f "${LINUX_SOURCE_BIN}" ]]; then
    echo "error: expected compiled binary at ${LINUX_SOURCE_BIN}" >&2
    exit 1
  fi

  echo "==> Installing codex binary into vendor/${LINUX_TARGET}/codex/"
  cp "${LINUX_SOURCE_BIN}" "${LINUX_VENDOR_BIN}"
  chmod +x "${LINUX_VENDOR_BIN}"
}

update_linux_binary

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) not found on PATH. Install it (e.g. 'brew install ripgrep' or 'cargo install ripgrep') before running this script." >&2
  exit 1
fi

RG_BIN="$(command -v rg)"
RG_NAME="rg"
if [[ "${HOST_TRIPLE}" == *"windows"* ]]; then
  RG_NAME="rg.exe"
fi
echo "==> Copying local ripgrep from ${RG_BIN}"
cp "${RG_BIN}" "${RG_VENDOR_DIR}/${RG_NAME}"
chmod +x "${RG_VENDOR_DIR}/${RG_NAME}"

PACKAGE_SCRIPT="${CODEX_CLI_DIR}/scripts/build_npm_package.py"

choose_python() {
  if [[ -n "${PYTHON_BIN:-}" ]]; then
    echo "${PYTHON_BIN}"
    return 0;
  fi
  if command -v python3 >/dev/null 2>&1; then
    if python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' >/dev/null 2>&1; then
      echo "python3"
      return 0
    fi
  fi
  if command -v uv >/dev/null 2>&1; then
    echo "uv run python"
    return 0
  fi
  return 1
}

PYTHON_CMD="$(choose_python)" || {
  echo "error: need Python 3.10+ or uv (set PYTHON_BIN if you have a custom interpreter)." >&2
  exit 1
}

read -r -a PYTHON_ARGS <<< "${PYTHON_CMD}"

echo "==> Staging npm package via ${PYTHON_CMD} ${PACKAGE_SCRIPT}"
"${PYTHON_ARGS[@]}" "${PACKAGE_SCRIPT}" \
  --package codex-super \
  --version "${VERSION}" \
  --vendor-src "${CODEX_CLI_DIR}/vendor" \
  --pack-output "${OUTPUT}"

echo
echo "Package ready: ${OUTPUT}"
echo "Install locally with: npm install -g ${OUTPUT}"
