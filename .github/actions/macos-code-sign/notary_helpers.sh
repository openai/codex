#!/usr/bin/env bash

notarize_submission() {
  local label="$1"
  local path="$2"
  local notary_key_path="$3"

  if [[ -z "${APPLE_NOTARIZATION_KEY_ID:-}" || -z "${APPLE_NOTARIZATION_ISSUER_ID:-}" ]]; then
    echo "APPLE_NOTARIZATION_KEY_ID and APPLE_NOTARIZATION_ISSUER_ID are required for notarization"
    exit 1
  fi

  if [[ -z "$notary_key_path" || ! -f "$notary_key_path" ]]; then
    echo "Notary key file $notary_key_path not found"
    exit 1
  fi

  if [[ ! -f "$path" ]]; then
    echo "Notarization payload $path not found"
    exit 1
  fi

  local submission_json
  submission_json=$(xcrun notarytool submit "$path" \
    --key "$notary_key_path" \
    --key-id "$APPLE_NOTARIZATION_KEY_ID" \
    --issuer "$APPLE_NOTARIZATION_ISSUER_ID" \
    --output-format json \
    --wait)

  local status submission_id
  status=$(printf '%s\n' "$submission_json" | jq -r '.status // "Unknown"')
  submission_id=$(printf '%s\n' "$submission_json" | jq -r '.id // ""')

  if [[ -z "$submission_id" ]]; then
    echo "Failed to retrieve submission ID for $label"
    exit 1
  fi

  echo "::notice title=Notarization::$label submission ${submission_id} completed with status ${status}"

  if [[ "$status" != "Accepted" ]]; then
    echo "Notarization failed for ${label} (submission ${submission_id}, status ${status})"
    exit 1
  fi
}
