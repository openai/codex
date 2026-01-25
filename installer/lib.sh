#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ]; then
  echo "Codex installer requires bash." >&2
  exit 1
fi

set -euo pipefail

CODEX_HOME_DEFAULT="${HOME}/.codex"
CODEX_HOME_RESOLVED="${CODEX_HOME:-$CODEX_HOME_DEFAULT}"

codex_home() {
  printf '%s\n' "$CODEX_HOME_RESOLVED"
}

codex_bin_dir() {
  printf '%s/bin\n' "$(codex_home)"
}

codex_versions_dir() {
  printf '%s/versions\n' "$(codex_home)"
}

codex_tools_dir() {
  printf '%s/tools\n' "$(codex_home)"
}

codex_tools_bin_dir() {
  printf '%s/bin\n' "$(codex_tools_dir)"
}

current_version_link() {
  printf '%s/current\n' "$(codex_versions_dir)"
}

detect_os() {
  local uname_s
  uname_s="$(uname -s)"
  case "$uname_s" in
    Darwin) printf '%s\n' "apple-darwin" ;;
    Linux) printf '%s\n' "unknown-linux-musl" ;;
    *)
      echo "Unsupported OS: $uname_s" >&2
      exit 1
      ;;
  esac
}

detect_arch() {
  local uname_m
  uname_m="$(uname -m)"
  case "$uname_m" in
    arm64|aarch64) printf '%s\n' "aarch64" ;;
    x86_64|amd64) printf '%s\n' "x86_64" ;;
    *)
      echo "Unsupported architecture: $uname_m" >&2
      exit 1
      ;;
  esac
}

github_api() {
  local path="$1"
  curl -fsSL \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "https://api.github.com/repos/openai/codex/${path}"
}

latest_tag() {
  github_api "releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1
}

normalize_version() {
  local tag="$1"
  tag="${tag#rust-v}"
  tag="${tag#v}"
  printf '%s\n' "$tag"
}

release_tag_for_version() {
  local version="$1"
  printf '%s\n' "rust-v${version}"
}

expected_binary_name() {
  local arch="$1"
  local os="$2"
  printf '%s\n' "codex-${arch}-${os}"
}

release_url() {
  local version="$1"
  local arch="$2"
  local os="$3"
  local tag
  tag="$(release_tag_for_version "$version")"
  printf '%s\n' "https://github.com/openai/codex/releases/download/${tag}/codex-${arch}-${os}.tar.gz"
}

ensure_dirs() {
  mkdir -p "$(codex_bin_dir)" "$(codex_versions_dir)" "$(codex_tools_bin_dir)"
}

ensure_path_block() {
  local rc_file="$1"
  local resolved_home="$2"
  local start_marker="# >>> codex >>>"

  if [ ! -f "$rc_file" ]; then
    touch "$rc_file"
  fi

  if grep -Fq "$start_marker" "$rc_file"; then
    return
  fi

  if [ "$resolved_home" = "${HOME}/.codex" ]; then
    cat >>"$rc_file" <<'EOF'
# >>> codex >>>
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
export PATH="$CODEX_HOME/bin:$PATH"
# <<< codex <<<
EOF
  else
    cat >>"$rc_file" <<EOF
# >>> codex >>>
export CODEX_HOME="${resolved_home}"
export PATH="\$CODEX_HOME/bin:\$PATH"
# <<< codex <<<
EOF
  fi
}

choose_rc_file() {
  local shell_name
  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    zsh) printf '%s\n' "${HOME}/.zshrc" ;;
    bash)
      if [ -f "${HOME}/.bashrc" ]; then
        printf '%s\n' "${HOME}/.bashrc"
      else
        printf '%s\n' "${HOME}/.bash_profile"
      fi
      ;;
    *)
      printf '%s\n' "${HOME}/.profile"
      ;;
  esac
}

install_wrapper() {
  local wrapper_path
  wrapper_path="$(codex_bin_dir)/codex"

  cat >"$wrapper_path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
CURRENT_LINK="$CODEX_HOME/versions/current"
TARGET="$CURRENT_LINK/bin/codex"

if [ ! -x "$TARGET" ]; then
  echo "Codex is not installed under $CODEX_HOME." >&2
  echo "Run the installer again to repair the installation." >&2
  exit 1
fi

export CODEX_MANAGED_BY_CURL=1
export PATH="$CODEX_HOME/bin:$CODEX_HOME/tools/bin:$CURRENT_LINK/bin:$PATH"

exec "$TARGET" "$@"
EOF

  chmod +x "$wrapper_path"
}

install_version_from_tarball() {
  local version="$1"
  local tarball="$2"
  local arch="$3"
  local os="$4"

  local versions_dir version_dir bin_dir expected_name tmpdir main_candidate
  versions_dir="$(codex_versions_dir)"
  version_dir="${versions_dir}/${version}"
  bin_dir="${version_dir}/bin"
  expected_name="$(expected_binary_name "$arch" "$os")"

  rm -rf "$version_dir"
  mkdir -p "$bin_dir"

  tmpdir="$(mktemp -d)"
  tar -xzf "$tarball" -C "$tmpdir"

  if [ -f "$tmpdir/$expected_name" ]; then
    main_candidate="$tmpdir/$expected_name"
  elif [ -f "$tmpdir/codex" ]; then
    main_candidate="$tmpdir/codex"
  else
    main_candidate="$(find "$tmpdir" -type f -name 'codex*' | head -n1 || true)"
  fi
  if [ -z "$main_candidate" ]; then
    echo "Failed to locate codex binary in tarball." >&2
    exit 1
  fi

  local files file base dest
  mapfile -t files < <(find "$tmpdir" -type f)
  for file in "${files[@]}"; do
    base="$(basename "$file")"
    if [ "$file" = "$main_candidate" ]; then
      dest="$bin_dir/codex"
    else
      dest="$bin_dir/$base"
    fi
    mv "$file" "$dest"
    chmod +x "$dest" 2>/dev/null || true
  done

  if [ ! -x "$bin_dir/codex" ]; then
    echo "Failed to install codex binary." >&2
    exit 1
  fi

  rm -rf "$tmpdir"
}

activate_version() {
  local version="$1"
  local link
  link="$(current_version_link)"
  ln -sfn "$(codex_versions_dir)/$version" "$link"
}

cleanup_old_versions() {
  local keep="${1:-2}"
  local versions_dir
  versions_dir="$(codex_versions_dir)"
  if [ ! -d "$versions_dir" ]; then
    return
  fi
  if ! compgen -G "$versions_dir/*" >/dev/null; then
    return
  fi

  # Remove older versions while keeping the most recent N directories.
  # We intentionally ignore errors here so cleanup never blocks install.
  ls -1dt "$versions_dir"/* 2>/dev/null \
    | grep -v '/current$' \
    | tail -n +"$((keep + 1))" \
    | xargs -I{} rm -rf "{}" 2>/dev/null || true
}
