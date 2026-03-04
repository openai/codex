#!/bin/sh

set -eu

VERSION="${1:-latest}"
INSTALL_DIR="${CODEX_INSTALL_DIR:-$HOME/.local/bin}"
path_action="already"
path_profile=""

step() {
  printf '==> %s\n' "$1"
}

normalize_version() {
  case "$1" in
    "" | latest)
      printf 'latest\n'
      ;;
    rust-v*)
      printf '%s\n' "${1#rust-v}"
      ;;
    v*)
      printf '%s\n' "${1#v}"
      ;;
    *)
      printf '%s\n' "$1"
      ;;
  esac
}

download_file() {
  url="$1"
  output="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -q -O "$output" "$url"
    return
  fi

  echo "curl or wget is required to install Codex." >&2
  exit 1
}

download_text() {
  url="$1"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url"
    return
  fi

  if command -v wget >/dev/null 2>&1; then
    wget -q -O - "$url"
    return
  fi

  echo "curl or wget is required to install Codex." >&2
  exit 1
}

add_to_path() {
  path_action="already"
  path_profile=""

  case ":$PATH:" in
    *":$INSTALL_DIR:"*)
      return
      ;;
  esac

  profile="$HOME/.profile"
  case "${SHELL:-}" in
    */zsh)
      profile="$HOME/.zshrc"
      ;;
    */bash)
      profile="$HOME/.bashrc"
      ;;
  esac

  path_profile="$profile"
  path_line="export PATH=\"$INSTALL_DIR:\$PATH\""
  if [ -f "$profile" ] && grep -F "$path_line" "$profile" >/dev/null 2>&1; then
    path_action="configured"
    return
  fi

  {
    printf '\n# Added by Codex installer\n'
    printf '%s\n' "$path_line"
  } >>"$profile"
  path_action="added"
}

release_url_for_asset() {
  asset="$1"
  resolved_version="$2"

  printf 'https://github.com/openai/codex/releases/download/rust-v%s/%s\n' "$resolved_version" "$asset"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "$1 is required to install Codex." >&2
    exit 1
  fi
}

require_command mktemp
require_command tar

resolve_version() {
  normalized_version="$(normalize_version "$VERSION")"

  if [ "$normalized_version" != "latest" ]; then
    printf '%s\n' "$normalized_version"
    return
  fi

  release_json="$(download_text "https://api.github.com/repos/openai/codex/releases/latest")"
  resolved="$(printf '%s\n' "$release_json" | sed -n 's/.*"tag_name":[[:space:]]*"rust-v\([^"]*\)".*/\1/p' | head -n 1)"

  if [ -z "$resolved" ]; then
    echo "Failed to resolve the latest Codex release version." >&2
    exit 1
  fi

  printf '%s\n' "$resolved"
}

case "$(uname -s)" in
  Darwin)
    os="darwin"
    ;;
  Linux)
    os="linux"
    ;;
  *)
    echo "install.sh supports macOS and Linux. Use install.ps1 on Windows." >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64 | amd64)
    arch="x86_64"
    ;;
  arm64 | aarch64)
    arch="aarch64"
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

if [ "$os" = "darwin" ] && [ "$arch" = "x86_64" ]; then
  if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
    arch="aarch64"
  fi
fi

if [ "$os" = "darwin" ]; then
  if [ "$arch" = "aarch64" ]; then
    npm_tag="darwin-arm64"
    vendor_target="aarch64-apple-darwin"
    platform_label="macOS (Apple Silicon)"
  else
    npm_tag="darwin-x64"
    vendor_target="x86_64-apple-darwin"
    platform_label="macOS (Intel)"
  fi
else
  if [ "$arch" = "aarch64" ]; then
    npm_tag="linux-arm64"
    vendor_target="aarch64-unknown-linux-musl"
    platform_label="Linux (ARM64)"
  else
    npm_tag="linux-x64"
    vendor_target="x86_64-unknown-linux-musl"
    platform_label="Linux (x64)"
  fi
fi

if [ -x "$INSTALL_DIR/codex" ]; then
  install_mode="Updating"
else
  install_mode="Installing"
fi

step "$install_mode Codex CLI"
step "Detected platform: $platform_label"

resolved_version="$(resolve_version)"
asset="codex-npm-$npm_tag-$resolved_version.tgz"
download_url="$(release_url_for_asset "$asset" "$resolved_version")"

step "Resolved version: $resolved_version"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive_path="$tmp_dir/$asset"

step "Downloading Codex CLI"
download_file "$download_url" "$archive_path"

tar -xzf "$archive_path" -C "$tmp_dir"

step "Installing to $INSTALL_DIR"
mkdir -p "$INSTALL_DIR"
cp "$tmp_dir/package/vendor/$vendor_target/codex/codex" "$INSTALL_DIR/codex"
cp "$tmp_dir/package/vendor/$vendor_target/path/rg" "$INSTALL_DIR/rg"
chmod 0755 "$INSTALL_DIR/codex"
chmod 0755 "$INSTALL_DIR/rg"

add_to_path

case "$path_action" in
  added)
    step "PATH updated for future shells in $path_profile"
    step "Run now: export PATH=\"$INSTALL_DIR:\$PATH\" && codex"
    step "Or open a new terminal and run: codex"
    ;;
  configured)
    step "PATH is already configured for future shells in $path_profile"
    step "Run now: export PATH=\"$INSTALL_DIR:\$PATH\" && codex"
    step "Or open a new terminal and run: codex"
    ;;
  *)
    step "$INSTALL_DIR is already on PATH"
    step "Run: codex"
    ;;
esac

printf 'Codex CLI %s installed successfully.\n' "$resolved_version"
