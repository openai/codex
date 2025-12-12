#!/bin/sh
set -eu

slug="mine"
repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
codex_rs_cli_dir="$repo_root/codex-rs/cli"

install_root="${CODEX_MINE_ROOT:-$HOME/.local/codex-$slug}"
bin_dir="${CODEX_MINE_BIN_DIR:-$HOME/.local/bin}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'ERROR: required command not found: %s\n' "$1" >&2
    exit 1
  fi
}

need_cmd cargo

if [ ! -d "$codex_rs_cli_dir" ]; then
  printf 'ERROR: expected directory not found: %s\n' "$codex_rs_cli_dir" >&2
  exit 1
fi

mkdir -p "$bin_dir"

printf '==> Installing Rust Codex CLI to %s\n' "$install_root" >&2
cargo install --path "$codex_rs_cli_dir" --bin codex --root "$install_root" --force

printf '==> Writing wrapper: %s\n' "$bin_dir/codex-$slug" >&2
cat > "$bin_dir/codex-$slug" <<SH
#!/bin/sh
set -eu
exec "$install_root/bin/codex" --config check_for_update_on_startup=false "\$@"
SH
chmod +x "$bin_dir/codex-$slug"

printf '\nOK: %s\n' "$bin_dir/codex-$slug" >&2
printf 'NOTE: %s must be on your PATH\n' "$bin_dir" >&2
printf 'Try: %s\n' "codex-$slug --version" >&2
