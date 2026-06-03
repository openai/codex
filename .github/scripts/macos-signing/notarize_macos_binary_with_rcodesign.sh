#!/usr/bin/env bash

# Submits a signed standalone macOS binary to Apple notarization through
# rcodesign. Standalone binaries cannot carry a stapled ticket, so the binary
# is submitted in a ZIP and the successful notarization log is retained.

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: notarize_macos_binary_with_rcodesign.sh --binary PATH [--report-dir PATH] [--max-wait-seconds SECONDS]

Options:
  --binary PATH                 Signed standalone macOS binary to notarize.
  --report-dir PATH             Directory for notarization logs.
  --max-wait-seconds SECONDS    Maximum rcodesign notarization wait time.
EOF
}

binary_path=""
report_dir="${RUNNER_TEMP:-/tmp}/macos-binary-notarization-verification"
max_wait_seconds="600"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)
      binary_path="${2:-}"
      shift 2
      ;;
    --report-dir)
      report_dir="${2:-}"
      shift 2
      ;;
    --max-wait-seconds)
      max_wait_seconds="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown notarization argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$binary_path" ]]; then
  echo "--binary is required." >&2
  usage
  exit 2
fi

if [[ ! -f "$binary_path" ]]; then
  echo "Binary does not exist: $binary_path" >&2
  exit 1
fi

if [[ ! "$max_wait_seconds" =~ ^[0-9]+$ ]]; then
  echo "--max-wait-seconds must be a non-negative integer." >&2
  exit 2
fi

for command_name in rcodesign zip; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "$command_name was not found on PATH." >&2
    exit 1
  fi
done

missing_environment=0
for variable_name in \
  APPLE_NOTARIZATION_ISSUER_ID \
  APPLE_NOTARIZATION_KEY_ID \
  APPLE_NOTARIZATION_KEY_P8
do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "$variable_name must be set from CI secrets before notarizing a binary." >&2
    missing_environment=1
  fi
done

if [[ "$missing_environment" -ne 0 ]]; then
  exit 2
fi

mkdir -p "$report_dir"

notarization_temp_dir="$(mktemp -d)"
trap 'rm -rf "$notarization_temp_dir" >/dev/null' EXIT

private_key_path="$notarization_temp_dir/AuthKey_${APPLE_NOTARIZATION_KEY_ID}.p8"
if ! printf '%s' "$APPLE_NOTARIZATION_KEY_P8" | base64 --decode >"$private_key_path" 2>/dev/null; then
  if ! printf '%s' "$APPLE_NOTARIZATION_KEY_P8" | base64 -D >"$private_key_path" 2>/dev/null; then
    echo "APPLE_NOTARIZATION_KEY_P8 must be a base64-encoded .p8 private key." >&2
    exit 2
  fi
fi
chmod 600 "$private_key_path"

api_key_path="$notarization_temp_dir/app-store-connect-api-key.json"
rcodesign encode-app-store-connect-api-key \
  --output-path "$api_key_path" \
  "$APPLE_NOTARIZATION_ISSUER_ID" \
  "$APPLE_NOTARIZATION_KEY_ID" \
  "$private_key_path" \
  >"$report_dir/encode-app-store-connect-api-key.log" 2>&1

binary_name="$(basename "$binary_path")"
archive_path="$notarization_temp_dir/${binary_name}.zip"
(
  cd "$(dirname "$binary_path")"
  zip -q "$archive_path" "$binary_name"
)

notarization_log="$report_dir/${binary_name}-notarization.log"
rcodesign notarize \
  --api-key-file "$api_key_path" \
  --max-wait-seconds "$max_wait_seconds" \
  "$archive_path" \
  2>&1 | tee "$notarization_log"

{
  echo "binary_name=$binary_name"
  echo "max_wait_seconds=$max_wait_seconds"
  echo "binary_sha256=$(shasum -a 256 "$binary_path" | awk '{ print $1 }')"
  echo "rcodesign_notarize=completed"
} >"$report_dir/${binary_name}-notarization-summary.txt"
