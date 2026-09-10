#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build-codex-package-archive.sh \
  --target <rust-target> \
  --bundle <primary|app-server> \
  --entrypoint-dir <dir> \
  --archive-dir <dir> \
  [--bwrap-bin <path>] \
  [--code-mode-host-bin <path>] \
  [--rg-bin <path>] \
  [--zsh-bin <path>] \
  [--zsh-manifest <path>] \
  [--codex-command-runner-bin <path>] \
  [--codex-windows-sandbox-setup-bin <path>] \
  [--voice-release-dir <path> --release-version <release-version>] \
  [--target-suffixed-entrypoint]
EOF
}

target=""
bundle=""
entrypoint_dir=""
archive_dir=""
target_suffixed_entrypoint="false"
resource_args=()
bwrap_bin_provided="false"
code_mode_host_bin_provided="false"
command_runner_bin_provided="false"
sandbox_setup_bin_provided="false"
voice_release_dir=""
release_version=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      target="${2:?--target requires a value}"
      shift 2
      ;;
    --bundle)
      bundle="${2:?--bundle requires a value}"
      shift 2
      ;;
    --entrypoint-dir)
      entrypoint_dir="${2:?--entrypoint-dir requires a value}"
      shift 2
      ;;
    --archive-dir)
      archive_dir="${2:?--archive-dir requires a value}"
      shift 2
      ;;
    --bwrap-bin)
      resource_args+=(--bwrap-bin "${2:?--bwrap-bin requires a value}")
      bwrap_bin_provided="true"
      shift 2
      ;;
    --code-mode-host-bin)
      resource_args+=(--code-mode-host-bin "${2:?--code-mode-host-bin requires a value}")
      code_mode_host_bin_provided="true"
      shift 2
      ;;
    --rg-bin)
      resource_args+=(--rg-bin "${2:?--rg-bin requires a value}")
      shift 2
      ;;
    --zsh-bin)
      resource_args+=(--zsh-bin "${2:?--zsh-bin requires a value}")
      shift 2
      ;;
    --zsh-manifest)
      resource_args+=(--zsh-manifest "${2:?--zsh-manifest requires a value}")
      shift 2
      ;;
    --codex-command-runner-bin)
      resource_args+=(
        --codex-command-runner-bin
        "${2:?--codex-command-runner-bin requires a value}"
      )
      command_runner_bin_provided="true"
      shift 2
      ;;
    --codex-windows-sandbox-setup-bin)
      resource_args+=(
        --codex-windows-sandbox-setup-bin
        "${2:?--codex-windows-sandbox-setup-bin requires a value}"
      )
      sandbox_setup_bin_provided="true"
      shift 2
      ;;
    --target-suffixed-entrypoint)
      target_suffixed_entrypoint="true"
      shift
      ;;
    --voice-release-dir)
      voice_release_dir="${2:?--voice-release-dir requires a value}"
      shift 2
      ;;
    --release-version)
      release_version="${2:?--release-version requires a value}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unexpected argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$target" || -z "$bundle" || -z "$entrypoint_dir" || -z "$archive_dir" ]]; then
  usage >&2
  exit 1
fi
if [[ ( -n "$voice_release_dir" || -n "$release_version" ) && ( -z "$voice_release_dir" || -z "$release_version" || "$bundle" != "primary" || ( "$target" != *-apple-darwin && "$target" != *-unknown-linux-musl && "$target" != *-pc-windows-msvc ) ) ]]; then
  echo "Voice resources require a primary supported release package version" >&2
  exit 1
fi

case "$bundle" in
  primary)
    variant="codex"
    entrypoint="codex"
    archive_stem="codex-package"
    ;;
  app-server)
    variant="codex-app-server"
    entrypoint="codex-app-server"
    archive_stem="codex-app-server-package"
    ;;
  *)
    echo "No Codex package variant for bundle: $bundle" >&2
    exit 1
    ;;
esac

exe_suffix=""
case "$target" in
  *windows*)
    exe_suffix=".exe"
    ;;
esac

code_mode_host_bin="${entrypoint_dir%/}/codex-code-mode-host${exe_suffix}"
if [[ "$code_mode_host_bin_provided" == "false" && -f "$code_mode_host_bin" ]]; then
  resource_args+=(--code-mode-host-bin "$code_mode_host_bin")
fi

entrypoint_name="$entrypoint"
if [[ "$target_suffixed_entrypoint" == "true" ]]; then
  entrypoint_name="${entrypoint_name}-${target}"
fi

case "$target" in
  *linux*)
    bwrap_bin="${entrypoint_dir%/}/bwrap"
    if [[ "$bwrap_bin_provided" == "false" && -f "$bwrap_bin" ]]; then
      resource_args+=(--bwrap-bin "$bwrap_bin")
    fi
    ;;
  *windows*)
    command_runner_bin="${entrypoint_dir%/}/codex-command-runner.exe"
    sandbox_setup_bin="${entrypoint_dir%/}/codex-windows-sandbox-setup.exe"
    if [[ "$command_runner_bin_provided" == "false" && -f "$command_runner_bin" ]]; then
      resource_args+=(--codex-command-runner-bin "$command_runner_bin")
    fi
    if [[ "$sandbox_setup_bin_provided" == "false" && -f "$sandbox_setup_bin" ]]; then
      resource_args+=(--codex-windows-sandbox-setup-bin "$sandbox_setup_bin")
    fi
    ;;
esac

repo_root="${GITHUB_WORKSPACE:-}"
if [[ -z "$repo_root" ]]; then
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

if command -v python3 >/dev/null 2>&1; then
  python_bin="python3"
else
  python_bin="python"
fi

if ! command -v zstd >/dev/null 2>&1 && [[ -x "${repo_root}/.github/workflows/zstd" ]]; then
  export PATH="${repo_root}/.github/workflows:${PATH}"
fi

mkdir -p "$archive_dir"
package_dir="${RUNNER_TEMP:-/tmp}/${archive_stem}-${target}"
gzip_archive_path="${archive_dir}/${archive_stem}-${target}.tar.gz"
zstd_archive_path="${archive_dir}/${archive_stem}-${target}.tar.zst"
rm -rf "$package_dir"

python_args=(
  "${repo_root}/scripts/build_codex_package.py"
  --target "$target"
  --variant "$variant"
  --entrypoint-bin "${entrypoint_dir%/}/${entrypoint_name}${exe_suffix}"
  --cargo-profile release
  --package-dir "$package_dir"
)
if [[ -z "$voice_release_dir" ]]; then
  python_args+=(--archive-output "$gzip_archive_path" --archive-output "$zstd_archive_path")
fi
if ((${#resource_args[@]} > 0)); then
  python_args+=("${resource_args[@]}")
fi
python_args+=(--force)

"$python_bin" "${python_args[@]}"

if [[ -n "$voice_release_dir" ]]; then
  voice_target="$target"
  if [[ "$target" == *-unknown-linux-musl ]]; then
    voice_target="${target%-musl}-gnu"
  fi
  voice_package="${RUNNER_TEMP:-/tmp}/${archive_stem}-voice-${target}"
  rm -rf "$voice_package"
  voice_helper="${voice_release_dir%/}/codex-voice-host${exe_suffix}"
  "$python_bin" "${repo_root}/third_party/voice/assemble_package.py" \
    --package "$package_dir" \
    --helper "$voice_helper" \
    --runtime "${voice_release_dir%/}/runtime" \
    --voice-target "$voice_target" \
    --build-commit "$(git -C "$repo_root" rev-parse HEAD)" \
    --release-version "$release_version" \
    --output "$voice_package"
  PYTHONPATH="${repo_root}/scripts" "$python_bin" - \
    "$voice_package" "$gzip_archive_path" "$zstd_archive_path" <<'PY'
import sys
from pathlib import Path
from codex_package.archive import write_archive

package = Path(sys.argv[1])
for archive in sys.argv[2:]:
    write_archive(package, Path(archive), force=True)
PY
fi
