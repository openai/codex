#!/usr/bin/env bash

# Notarizes and staples a signed macOS DMG through rcodesign.
#
# This is the Linux-compatible notarization path for the AKV/PKCS#11 signing
# flow. It records notarization inputs and logs so workflow artifacts can be
# audited without exposing the App Store Connect private key.

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: notarize_macos_dmg_with_rcodesign.sh --dmg PATH [--report-dir PATH] [--max-wait-seconds SECONDS]

Options:
  --dmg PATH                    Signed DMG to submit to Apple notarization.
  --report-dir PATH             Directory for notarization logs.
  --max-wait-seconds SECONDS    Maximum rcodesign notarization wait time.
EOF
}

dmg_path=""
report_dir="${RUNNER_TEMP:-/tmp}/macos-notarization-verification"
max_wait_seconds="600"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dmg)
      dmg_path="${2:-}"
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

if [[ -z "$dmg_path" ]]; then
  echo "--dmg is required." >&2
  usage
  exit 2
fi

if [[ ! -f "$dmg_path" ]]; then
  echo "DMG does not exist: $dmg_path" >&2
  exit 1
fi

if [[ ! "$max_wait_seconds" =~ ^[0-9]+$ ]]; then
  echo "--max-wait-seconds must be a non-negative integer." >&2
  exit 2
fi

if ! command -v rcodesign > /dev/null 2>&1; then
  echo "rcodesign was not found on PATH." >&2
  exit 1
fi

missing_environment=0
for variable_name in \
  APPLE_NOTARIZATION_ISSUER_ID \
  APPLE_NOTARIZATION_KEY_ID \
  APPLE_NOTARIZATION_KEY_P8
do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "$variable_name must be set from CI secrets before notarizing a DMG." >&2
    missing_environment=1
  fi
done

if [[ "$missing_environment" -ne 0 ]]; then
  exit 2
fi

mkdir -p "$report_dir"

notarization_temp_dir="$(mktemp -d)"
trap 'rm -rf "$notarization_temp_dir" > /dev/null' EXIT

private_key_path="$notarization_temp_dir/AuthKey_${APPLE_NOTARIZATION_KEY_ID}.p8"
if ! printf '%s' "$APPLE_NOTARIZATION_KEY_P8" | base64 --decode > "$private_key_path" 2> /dev/null; then
  if ! printf '%s' "$APPLE_NOTARIZATION_KEY_P8" | base64 -D > "$private_key_path" 2> /dev/null; then
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
  > "$report_dir/encode-app-store-connect-api-key.log" 2>&1

notarization_log="$report_dir/dmg-notarization.log"
rcodesign notarize \
  --api-key-file "$api_key_path" \
  --max-wait-seconds "$max_wait_seconds" \
  --staple \
  "$dmg_path" \
  2>&1 | tee "$notarization_log"

{
  echo "dmg_path=$dmg_path"
  echo "max_wait_seconds=$max_wait_seconds"
  echo "dmg_sha256=$(shasum -a 256 "$dmg_path" | awk '{ print $1 }')"
  echo "rcodesign_notarize_staple=completed"
} > "$report_dir/dmg-notarization-summary.txt"
