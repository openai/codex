#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build-codex-package-archive.sh \
  --target <rust-target> \
  --bundle <primary|app-server> \
  --entrypoint-dir <dir> \
  --archive-dir <dir> \
  [--target-suffixed-entrypoint]
EOF
}

target=""
bundle=""
entrypoint_dir=""
archive_dir=""
target_suffixed_entrypoint="false"

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
    --target-suffixed-entrypoint)
      target_suffixed_entrypoint="true"
      shift
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

entrypoint_name="$entrypoint"
if [[ "$target_suffixed_entrypoint" == "true" ]]; then
  entrypoint_name="${entrypoint_name}-${target}"
fi

repo_root="${GITHUB_WORKSPACE:-}"
if [[ -z "$repo_root" ]]; then
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fi

if command -v python3 >/dev/null 2>&1; then
  python_bin="python3"
else
  python_bin="python"
fi

mkdir -p "$archive_dir"
package_dir="${RUNNER_TEMP:-/tmp}/${archive_stem}-${target}"
archive_path="${archive_dir}/${archive_stem}-${target}.tar.gz"
rm -rf "$package_dir"

"$python_bin" "${repo_root}/scripts/build_codex_package.py" \
  --target "$target" \
  --variant "$variant" \
  --entrypoint-bin "${entrypoint_dir%/}/${entrypoint_name}${exe_suffix}" \
  --cargo-profile release \
  --package-dir "$package_dir" \
  --archive-output "$archive_path" \
  --force
