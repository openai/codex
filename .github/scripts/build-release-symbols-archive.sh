#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: build-release-symbols-archive.sh \
  --target <rust-target> \
  --artifact-name <artifact-name> \
  --release-dir <dir> \
  --archive-dir <dir> \
  --binaries "<space-delimited binary basenames>" \
  [--append]
EOF
}

target=""
artifact_name=""
release_dir=""
archive_dir=""
binaries=""
append="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      target="${2:?--target requires a value}"
      shift 2
      ;;
    --artifact-name)
      artifact_name="${2:?--artifact-name requires a value}"
      shift 2
      ;;
    --release-dir)
      release_dir="${2:?--release-dir requires a value}"
      shift 2
      ;;
    --archive-dir)
      archive_dir="${2:?--archive-dir requires a value}"
      shift 2
      ;;
    --binaries)
      binaries="${2:?--binaries requires a value}"
      shift 2
      ;;
    --append)
      append="true"
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

if [[ -z "$target" || -z "$artifact_name" || -z "$release_dir" || -z "$archive_dir" || -z "$binaries" ]]; then
  usage >&2
  exit 1
fi

symbols_root="${RUNNER_TEMP:-/tmp}/codex-symbols-${artifact_name}"
symbols_dir="${symbols_root}/codex-symbols-${artifact_name}"
archive_path="${archive_dir%/}/codex-symbols-${artifact_name}.tar.gz"
if [[ "$append" != "true" ]]; then
  rm -rf "$symbols_root"
fi
mkdir -p "$symbols_dir" "$archive_dir"
read -r -a binary_names <<< "$binaries"

case "$target" in
  *apple-darwin)
    for binary in "${binary_names[@]}"; do
      binary_path="${release_dir%/}/${binary}"
      dsym_path="${binary_path}.dSYM"
      if [[ ! -f "$binary_path" ]]; then
        echo "Binary $binary_path not found" >&2
        exit 1
      fi
      if [[ ! -d "$dsym_path" ]]; then
        echo "dSYM $dsym_path not found" >&2
        exit 1
      fi

      cp -RL "$dsym_path" "${symbols_dir}/${binary}.dSYM"
      strip -S -x "$binary_path"
    done
    ;;
  *linux*)
    objcopy_bin="${OBJCOPY:-objcopy}"
    strip_bin="${STRIP:-strip}"
    for binary in "${binary_names[@]}"; do
      binary_path="${release_dir%/}/${binary}"
      debug_path="${symbols_dir}/${binary}.debug"
      if [[ ! -f "$binary_path" ]]; then
        echo "Binary $binary_path not found" >&2
        exit 1
      fi

      "$objcopy_bin" --only-keep-debug "$binary_path" "$debug_path"
      "$strip_bin" --strip-debug --strip-unneeded "$binary_path"
      "$objcopy_bin" --add-gnu-debuglink="$debug_path" "$binary_path"
    done
    ;;
  *windows*)
    for binary in "${binary_names[@]}"; do
      pdb_path="${release_dir%/}/${binary}.pdb"
      if [[ ! -f "$pdb_path" ]]; then
        echo "PDB $pdb_path not found" >&2
        exit 1
      fi

      cp "$pdb_path" "${symbols_dir}/${binary}.pdb"
    done
    ;;
  *)
    echo "No symbols packaging support for target: $target" >&2
    exit 1
    ;;
esac

rm -f "$archive_path"
tar -C "$symbols_root" -czf "$archive_path" "codex-symbols-${artifact_name}"
