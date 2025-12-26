#!/bin/sh
set -eu

# Native installer for codex-mine (prebuilt binaries from GitHub Releases).
#
# Usage examples:
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/install.sh | sh
#   CODEX_MINE_VERSION=mine-v0.76.0-alpha.8-mine.0 sh install.sh
#
# Environment variables:
#   CODEX_MINE_GITHUB_REPO   (default: inoueryo/codex-mine)
#   CODEX_MINE_VERSION       (default: latest)
#   CODEX_MINE_ROOT          (default: ~/.local/codex-mine)
#   CODEX_MINE_BIN_DIR       (default: ~/.local/bin)

repo="${CODEX_MINE_GITHUB_REPO:-harukary/codex-mine}"
version="${CODEX_MINE_VERSION:-latest}"

install_root="${CODEX_MINE_ROOT:-$HOME/.local/codex-mine}"
bin_dir="${CODEX_MINE_BIN_DIR:-$HOME/.local/bin}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'ERROR: required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

need_cmd uname
need_cmd mktemp
need_cmd mkdir
need_cmd rm
need_cmd tar
need_cmd curl

os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os_name="apple-darwin" ;;
  Linux) os_name="unknown-linux-musl" ;;
  *)
    printf 'ERROR: unsupported OS: %s\n' "$os" >&2
    exit 1
    ;;
esac

case "$arch" in
  arm64|aarch64) arch_name="aarch64" ;;
  x86_64|amd64) arch_name="x86_64" ;;
  *)
    printf 'ERROR: unsupported arch: %s\n' "$arch" >&2
    exit 1
    ;;
esac

target="${arch_name}-${os_name}"
asset="codex-${target}.tar.gz"
checksums="checksums.txt"

download_url() {
  file="$1"
  if [ "$version" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download/%s' "$repo" "$file"
  else
    printf 'https://github.com/%s/releases/download/%s/%s' "$repo" "$version" "$file"
  fi
}

sha256_verify() {
  file_path="$1"
  sums_path="$2"
  file_name="$3"

  if [ ! -f "$sums_path" ]; then
    printf 'WARN: checksums file not found, skipping verification: %s\n' "$sums_path" >&2
    return 0
  fi

  expected="$(grep -E "  ${file_name}\$" "$sums_path" | awk '{print $1}' | head -n 1 || true)"
  if [ -z "$expected" ]; then
    printf 'WARN: no checksum entry for %s; skipping verification\n' "$file_name" >&2
    return 0
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$file_path" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$file_path" | awk '{print $1}')"
  else
    printf 'WARN: sha256sum/shasum not found; skipping verification\n' >&2
    return 0
  fi

  if [ "$actual" != "$expected" ]; then
    printf 'ERROR: checksum mismatch for %s\n' "$file_name" >&2
    printf '  expected: %s\n' "$expected" >&2
    printf '  actual:   %s\n' "$actual" >&2
    exit 1
  fi
}

tmp="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT INT TERM

printf '==> Downloading %s (%s)\n' "$asset" "$target" >&2
curl -fsSL -o "$tmp/$asset" "$(download_url "$asset")"

printf '==> Downloading %s\n' "$checksums" >&2
if curl -fsSL -o "$tmp/$checksums" "$(download_url "$checksums")"; then
  sha256_verify "$tmp/$asset" "$tmp/$checksums" "$asset"
else
  printf 'WARN: failed to download checksums.txt; continuing without verification\n' >&2
fi

mkdir -p "$install_root/bin" "$bin_dir"
tar -xzf "$tmp/$asset" -C "$tmp"

if [ ! -f "$tmp/codex" ]; then
  printf 'ERROR: expected extracted binary not found: %s\n' "$tmp/codex" >&2
  exit 1
fi

chmod +x "$tmp/codex"
mv "$tmp/codex" "$install_root/bin/codex"

printf '==> Writing wrapper: %s\n' "$bin_dir/codex-mine" >&2
cat > "$bin_dir/codex-mine" <<SH
#!/bin/sh
set -eu
exec "$install_root/bin/codex" --config check_for_update_on_startup=false "\$@"
SH
chmod +x "$bin_dir/codex-mine"

printf '\nOK: %s\n' "$bin_dir/codex-mine" >&2
printf 'Try: %s\n' "codex-mine --version" >&2
