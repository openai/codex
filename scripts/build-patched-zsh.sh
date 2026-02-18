#!/usr/bin/env bash
set -euo pipefail

# Build the Codex-patched zsh used by shell-tool-mcp.
#
# Defaults are intentionally override-friendly:
#   CODEX_REPO       Codex repo root containing shell-tool-mcp/patches
#   ZSH_SRC_DIR      Working clone location for zsh source
#   INSTALL_PREFIX   Install prefix for make install
#   JOBS             Parallel jobs for make
#   ZSH_REMOTE       Upstream zsh remote
#   INSTALL_DOCS     Set to 1 to run full `make install` (includes manpages)

ZSH_COMMIT="77045ef899e53b9598bebc5a41db93a548a40ca6"
ZSH_REMOTE="${ZSH_REMOTE:-https://git.code.sf.net/p/zsh/code}"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local/codex-zsh-${ZSH_COMMIT:0:7}}"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

resolve_codex_repo() {
  if [[ -n "${CODEX_REPO:-}" ]]; then
    printf '%s\n' "$CODEX_REPO"
    return
  fi

  # Most reliable when this script is run from inside the codex repo.
  local from_script
  from_script="$(cd -- "$SCRIPT_DIR/.." && pwd)"
  if [[ -f "$from_script/shell-tool-mcp/patches/zsh-exec-wrapper.patch" ]]; then
    printf '%s\n' "$from_script"
    return
  fi

  # Common local workspace layouts.
  if [[ -f "$HOME/repos/codex/shell-tool-mcp/patches/zsh-exec-wrapper.patch" ]]; then
    printf '%s\n' "$HOME/repos/codex"
    return
  fi

  if [[ -f "$HOME/code/codex/shell-tool-mcp/patches/zsh-exec-wrapper.patch" ]]; then
    printf '%s\n' "$HOME/code/codex"
    return
  fi

  echo "Could not locate codex repo. Set CODEX_REPO=/path/to/codex." >&2
  exit 1
}

resolve_zsh_src_dir() {
  if [[ -n "${ZSH_SRC_DIR:-}" ]]; then
    printf '%s\n' "$ZSH_SRC_DIR"
    return
  fi

  if [[ -d "$HOME/repos/zsh/.git" ]]; then
    printf '%s\n' "$HOME/repos/zsh"
    return
  fi

  if [[ -d "$HOME/code/zsh/.git" ]]; then
    printf '%s\n' "$HOME/code/zsh"
    return
  fi

  # Fallback for users without an existing clone.
  printf '%s\n' "$HOME/src/zsh-code"
}

CODEX_REPO="$(resolve_codex_repo)"
ZSH_SRC_DIR="$(resolve_zsh_src_dir)"
PATCH_FILE="$CODEX_REPO/shell-tool-mcp/patches/zsh-exec-wrapper.patch"

if [[ ! -f "$PATCH_FILE" ]]; then
  echo "Patch file not found: $PATCH_FILE" >&2
  exit 1
fi

if [[ -z "${JOBS:-}" ]]; then
  if command -v nproc >/dev/null 2>&1; then
    JOBS="$(nproc)"
  elif command -v sysctl >/dev/null 2>&1; then
    JOBS="$(sysctl -n hw.ncpu)"
  else
    JOBS=4
  fi
fi

mkdir -p "$(dirname -- "$ZSH_SRC_DIR")"

if [[ ! -d "$ZSH_SRC_DIR/.git" ]]; then
  git clone "$ZSH_REMOTE" "$ZSH_SRC_DIR"
fi

git -C "$ZSH_SRC_DIR" fetch --depth 1 origin "$ZSH_COMMIT"
git -C "$ZSH_SRC_DIR" checkout --detach -f "$ZSH_COMMIT"
git -C "$ZSH_SRC_DIR" reset --hard "$ZSH_COMMIT"

if git -C "$ZSH_SRC_DIR" apply --reverse --check "$PATCH_FILE" >/dev/null 2>&1; then
  echo "Patch already applied: $PATCH_FILE"
else
  git -C "$ZSH_SRC_DIR" apply --check "$PATCH_FILE"
  git -C "$ZSH_SRC_DIR" apply "$PATCH_FILE"
fi

(
  cd "$ZSH_SRC_DIR"
  ./Util/preconfig
  ./configure --prefix="$INSTALL_PREFIX"
  make -j"$JOBS"
  if [[ "${INSTALL_DOCS:-0}" == "1" ]]; then
    make install
  else
    make install.bin
  fi
)

cat <<OUT

Built patched zsh successfully.
Binary:
  $INSTALL_PREFIX/bin/zsh

Quick checks:
  $INSTALL_PREFIX/bin/zsh --version
  $INSTALL_PREFIX/bin/zsh -fc '/bin/echo smoke-zsh'
OUT
