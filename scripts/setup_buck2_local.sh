#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: scripts/setup_buck2_local.sh [--build]

Sets up local-only Buck2 + Reindeer artifacts for codex-rs:
  - vendors crates into codex-rs/third-party/vendor/
  - generates codex-rs/third-party/BUCK via reindeer buckify
  - generates codex-rs/**/BUCK for workspace crates
  - generates a toolchain definition under codex-rs/toolchains/BUCK

All generated Buck artifacts are expected to be gitignored (BUCK files,
codex-rs/third-party, buck-out).

Environment:
  REINDEER_BIN  Optional path to reindeer binary (default: scripts/reindeer)
  BUCK2_BIN     Optional path to buck2 binary (default: scripts/buck2)

If invoked from within Codex CLI, you may want:
  env -u BASH_EXEC_WRAPPER -u CODEX_ESCALATE_SOCKET buck2 build //codex-rs/cli:codex
EOF
}

DO_BUILD=0
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
elif [[ "${1:-}" == "--build" ]]; then
  DO_BUILD=1
elif [[ -n "${1:-}" ]]; then
  echo "Unknown argument: $1" >&2
  usage >&2
  exit 2
fi

REINDEER_BIN="${REINDEER_BIN:-}"
if [[ -z "${REINDEER_BIN}" ]]; then
  if [[ -x "${REPO_ROOT}/scripts/reindeer" ]]; then
    REINDEER_BIN="${REPO_ROOT}/scripts/reindeer"
  elif command -v reindeer >/dev/null 2>&1; then
    REINDEER_BIN="$(command -v reindeer)"
  else
    echo "Could not find reindeer. Set REINDEER_BIN or use scripts/reindeer." >&2
    exit 1
  fi
fi

BUCK2_BIN="${BUCK2_BIN:-}"
if [[ -z "${BUCK2_BIN}" ]]; then
  if [[ -x "${REPO_ROOT}/scripts/buck2" ]]; then
    BUCK2_BIN="${REPO_ROOT}/scripts/buck2"
  elif command -v buck2 >/dev/null 2>&1; then
    BUCK2_BIN="$(command -v buck2)"
  else
    echo "Could not find buck2. Set BUCK2_BIN or use scripts/buck2." >&2
    exit 1
  fi
fi

BUILDIFIER_BIN="${BUILDIFIER_BIN:-}"
if [[ -z "${BUILDIFIER_BIN}" ]]; then
  if [[ -e "${REPO_ROOT}/scripts/buildifier" ]]; then
    BUILDIFIER_BIN="${REPO_ROOT}/scripts/buildifier"
  elif command -v buildifier >/dev/null 2>&1; then
    BUILDIFIER_BIN="$(command -v buildifier)"
  else
    BUILDIFIER_BIN=""
  fi
fi

cd "${REPO_ROOT}"

# Resolve the Rust toolchain used by codex-rs so Buck uses the same compiler
# regardless of the working directory Buck actions run in.
RUST_TOOLCHAIN="$(
  python3 - <<'PY'
import pathlib, re
text = pathlib.Path("codex-rs/rust-toolchain.toml").read_text(encoding="utf-8")
m = re.search(r'(?m)^channel\s*=\s*"([^"]+)"\s*$', text)
if not m:
    raise SystemExit("Could not find toolchain channel in codex-rs/rust-toolchain.toml")
print(m.group(1))
PY
)"

if command -v rustup >/dev/null 2>&1; then
  RUSTC_PATH="$(rustup which rustc --toolchain "${RUST_TOOLCHAIN}")"
  RUSTDOC_PATH="$(rustup which rustdoc --toolchain "${RUST_TOOLCHAIN}")"
  if rustup which clippy-driver --toolchain "${RUST_TOOLCHAIN}" >/dev/null 2>&1; then
    CLIPPY_DRIVER_PATH="$(rustup which clippy-driver --toolchain "${RUST_TOOLCHAIN}")"
  else
    CLIPPY_DRIVER_PATH="clippy-driver"
  fi
else
  echo "rustup not found; falling back to rustc/rustdoc from PATH (may not match codex-rs toolchain)." >&2
  RUSTC_PATH="rustc"
  RUSTDOC_PATH="rustdoc"
  CLIPPY_DRIVER_PATH="clippy-driver"
fi

# Reindeer canonicalizes third_party_dir up-front, so it must exist.
mkdir -p codex-rs/third-party

# Ensure we have an ignored Cargo.lock next to the manifest Reindeer uses.
if [[ ! -e "codex-rs/cli/Cargo.lock" ]]; then
  (
    cd codex-rs/cli
    # Prefer a symlink (fast), but fall back to copying if symlinks are unsupported.
    if ln -s ../Cargo.lock Cargo.lock 2>/dev/null; then
      true
    else
      cp ../Cargo.lock Cargo.lock
    fi
  )
fi

# Ensure Buck can load bzl files from codex-rs/buck2 by creating an (ignored)
# BUCK file to define the package.
mkdir -p codex-rs/buck2
if [[ ! -e "codex-rs/buck2/BUCK" ]]; then
  : > codex-rs/buck2/BUCK
fi

# Generate a minimal toolchain package under the toolchains cell.
mkdir -p codex-rs/toolchains
# This BUCK file is local-only (gitignored), so we bake in absolute tool paths.
cat > codex-rs/toolchains/BUCK <<EOF
load("@prelude//tests:test_toolchain.bzl", "noop_test_toolchain")
load("@prelude//toolchains:cxx.bzl", "system_cxx_toolchain")
load("@prelude//toolchains:genrule.bzl", "system_genrule_toolchain")
load(
    "@prelude//toolchains:python.bzl",
    "system_python_bootstrap_toolchain",
    "system_python_toolchain",
)
load("@prelude//toolchains:remote_test_execution.bzl", "remote_test_execution_toolchain")
load("@root//codex-rs/buck2:codex_rust_toolchain.bzl", "codex_rust_toolchain")

system_cxx_toolchain(
    name = "cxx",
    visibility = ["PUBLIC"],
)

system_genrule_toolchain(
    name = "genrule",
    visibility = ["PUBLIC"],
)

system_python_toolchain(
    name = "python",
    visibility = ["PUBLIC"],
)

system_python_bootstrap_toolchain(
    name = "python_bootstrap",
    visibility = ["PUBLIC"],
)

codex_rust_toolchain(
    name = "rust",
    default_edition = "2024",
    rustc = "${RUSTC_PATH}",
    rustdoc = "${RUSTDOC_PATH}",
    clippy_driver = "${CLIPPY_DRIVER_PATH}",
    visibility = ["PUBLIC"],
)

remote_test_execution_toolchain(
    name = "remote_test_execution",
    visibility = ["PUBLIC"],
)

noop_test_toolchain(
    name = "test",
    visibility = ["PUBLIC"],
)
EOF

if [[ -n "${BUILDIFIER_BIN}" ]]; then
  # Keep local-only BUCK files readable for debugging.
  "${BUILDIFIER_BIN}" -lint=off -mode=fix codex-rs/toolchains/BUCK codex-rs/buck2/BUCK || true
fi

echo "Using:"
echo "  reindeer: ${REINDEER_BIN}"
echo "  buck2:    ${BUCK2_BIN}"
echo "  rustc:    ${RUSTC_PATH} (toolchain: ${RUST_TOOLCHAIN})"

(
  cd codex-rs
  "${REINDEER_BIN}" vendor
  python3 scripts/gen_reindeer_fixups.py
  "${REINDEER_BIN}" buckify
  python3 scripts/patch_third_party_buck_for_tests.py
  python3 scripts/gen_buck_first_party.py
)

echo ""
echo "Third-party size:"
if command -v du >/dev/null 2>&1; then
  du -sh codex-rs/third-party 2>/dev/null || true
  du -sh codex-rs/third-party/vendor 2>/dev/null || true
fi

echo ""
echo "Next:"
echo "  ${BUCK2_BIN} build //codex-rs/cli:codex"

if [[ "${DO_BUILD}" -eq 1 ]]; then
  echo ""
  echo "Building //codex-rs/cli:codex ..."
  # When running inside Codex CLI, these wrapper env vars can interfere with buck2.
  env -u BASH_EXEC_WRAPPER -u CODEX_ESCALATE_SOCKET "${BUCK2_BIN}" build //codex-rs/cli:codex
fi
