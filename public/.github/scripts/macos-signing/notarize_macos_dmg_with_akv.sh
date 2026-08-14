#!/usr/bin/env bash

# Notarize a signed disk image and staple its ticket.

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: notarize_macos_dmg_with_akv.sh --dmg PATH [--report-dir PATH] [--max-wait-seconds SECONDS]

Options:
  --dmg PATH                    Signed DMG to submit to Apple notarization.
  --report-dir PATH             Directory for notarization logs.
  --max-wait-seconds SECONDS    Maximum Apple notarization wait time.
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
  APPLE_NOTARIZATION_AKV_KEY_NAME \
  AZURE_KEYVAULT_NAME
do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "$variable_name must be configured before notarizing a DMG." >&2
    missing_environment=1
  fi
done

if [[ "$missing_environment" -ne 0 ]]; then
  exit 2
fi

mkdir -p "$report_dir"

notarization_log="$report_dir/dmg-notarization.log"
python3 "$(dirname "$0")/notarize_with_akv.py" \
  --file "$dmg_path" \
  --report-log "$report_dir/dmg-notarization-developer-log.json" \
  --max-wait-seconds "$max_wait_seconds" \
  2>&1 | tee "$notarization_log"

rcodesign staple "$dmg_path" 2>&1 | tee -a "$notarization_log"

{
  echo "dmg_path=$dmg_path"
  echo "max_wait_seconds=$max_wait_seconds"
  echo "dmg_sha256=$(shasum -a 256 "$dmg_path" | awk '{ print $1 }')"
  echo "notarization_staple=completed"
} > "$report_dir/dmg-notarization-summary.txt"
