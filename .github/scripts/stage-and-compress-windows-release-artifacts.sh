#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -lt 1 ]]; then
  echo "usage: $0 <target> [<target> ...]"
  exit 1
fi

process_target() {
  local target="$1"
  local release_dir="target/${target}/release"
  local dest="dist/${target}"
  local repo_root
  repo_root="$(pwd)"

  ls -lh "${release_dir}/codex.exe"
  ls -lh "${release_dir}/codex-responses-api-proxy.exe"
  ls -lh "${release_dir}/codex-windows-sandbox-setup.exe"
  ls -lh "${release_dir}/codex-command-runner.exe"

  mkdir -p "$dest"
  cp "${release_dir}/codex.exe" "$dest/codex-${target}.exe"
  cp "${release_dir}/codex-responses-api-proxy.exe" "$dest/codex-responses-api-proxy-${target}.exe"
  cp "${release_dir}/codex-windows-sandbox-setup.exe" "$dest/codex-windows-sandbox-setup-${target}.exe"
  cp "${release_dir}/codex-command-runner.exe" "$dest/codex-command-runner-${target}.exe"

  for f in "$dest"/*; do
    local base
    base="$(basename "$f")"

    if [[ "$base" == *.tar.gz || "$base" == *.zip || "$base" == *.dmg ]]; then
      continue
    fi

    if [[ "$base" == *.sigstore ]]; then
      continue
    fi

    tar -C "$dest" -czf "$dest/${base}.tar.gz" "$base"

    if [[ "$base" == "codex-${target}.exe" ]]; then
      local bundle_dir
      local runner_src
      local setup_src
      bundle_dir="$(mktemp -d)"
      runner_src="$dest/codex-command-runner-${target}.exe"
      setup_src="$dest/codex-windows-sandbox-setup-${target}.exe"

      if [[ -f "$runner_src" && -f "$setup_src" ]]; then
        cp "$dest/$base" "$bundle_dir/$base"
        cp "$runner_src" "$bundle_dir/codex-command-runner.exe"
        cp "$setup_src" "$bundle_dir/codex-windows-sandbox-setup.exe"
        (cd "$bundle_dir" && 7z a "$repo_root/$dest/${base}.zip" .)
      else
        echo "warning: missing sandbox binaries; falling back to single-binary zip"
        echo "warning: expected $runner_src and $setup_src"
        (cd "$dest" && 7z a "${base}.zip" "$base")
      fi

      rm -rf "$bundle_dir"
    else
      (cd "$dest" && 7z a "${base}.zip" "$base")
    fi

    "${GITHUB_WORKSPACE}/.github/workflows/zstd" -T0 -19 "$dest/$base"
  done
}

for target in "$@"; do
  process_target "$target"
done
