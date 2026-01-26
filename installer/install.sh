#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ]; then
  echo "This installer requires bash." >&2
  echo "Re-run with: curl -fsSL https://raw.githubusercontent.com/openai/codex/main/installer/install.sh | bash" >&2
  exit 1
fi

set -euo pipefail

INSTALLER_BASE_URL="${CODEX_INSTALLER_BASE_URL:-https://raw.githubusercontent.com/openai/codex/main/installer}"

load_lib() {
  local script_dir lib_path tmp_lib
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
  lib_path="${script_dir}/lib.sh"

  if [ -f "$lib_path" ]; then
    # Running from a repo checkout.
    # shellcheck disable=SC1090
    source "$lib_path"
    return
  fi

  tmp_lib="$(mktemp)"
  curl -fsSL "${INSTALLER_BASE_URL}/lib.sh" -o "$tmp_lib"
  # shellcheck disable=SC1090
  source "$tmp_lib"
  rm -f "$tmp_lib"
}

load_lib

main() {
  ensure_dirs

  local arch os tag version url tarball rc_file resolved_home
  arch="$(detect_arch)"
  os="$(detect_os)"

  if [ -n "${CODEX_VERSION:-}" ]; then
    version="$(normalize_version "$CODEX_VERSION")"
    tag="$(release_tag_for_version "$version")"
  else
    tag="$(latest_tag)"
    if [ -z "$tag" ]; then
      echo "Failed to determine the latest Codex release tag." >&2
      exit 1
    fi
    version="$(normalize_version "$tag")"
  fi

  url="$(release_url "$version" "$arch" "$os")"
  tarball="$(mktemp)"
  curl -fsSL "$url" -o "$tarball"

  install_version_from_tarball "$version" "$tarball" "$arch" "$os"
  activate_version "$version"
  install_wrapper
  cleanup_old_versions 2

  rc_file="$(choose_rc_file)"
  resolved_home="$(codex_home)"
  ensure_path_block "$rc_file" "$resolved_home"

  rm -f "$tarball"

  echo "Codex ${version} installed to ${resolved_home}"
  echo "Updated PATH in ${rc_file}"
  echo "Open a new shell or run: source ${rc_file}"
}

main "$@"
