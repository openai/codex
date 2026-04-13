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
    --output-format json)

  local status submission_id
  status=$(printf '%s\n' "$submission_json" | jq -r '.status // "Unknown"')
  submission_id=$(printf '%s\n' "$submission_json" | jq -r '.id // ""')

  if [[ -z "$submission_id" ]]; then
    echo "Failed to retrieve submission ID for $label"
    exit 1
  fi

  echo "::notice title=Notarization::$label submission ${submission_id} started with status ${status}"
  if [[ "$status" == "Accepted" ]]; then
    return 0
  fi

  local started_at=$SECONDS
  local timeout_seconds="${NOTARY_WAIT_TIMEOUT_SECONDS:-7200}"
  while ((SECONDS - started_at < timeout_seconds)); do
    sleep 30

    submission_json=$(xcrun notarytool info "$submission_id" \
      --key "$notary_key_path" \
      --key-id "$APPLE_NOTARIZATION_KEY_ID" \
      --issuer "$APPLE_NOTARIZATION_ISSUER_ID" \
      --output-format json)
    status=$(printf '%s\n' "$submission_json" | jq -r '.status // "Unknown"')
    echo "::notice title=Notarization::$label submission ${submission_id} status ${status}"

    case "$status" in
      Accepted)
        return 0
        ;;
      Invalid|Rejected)
        xcrun notarytool log "$submission_id" \
          --key "$notary_key_path" \
          --key-id "$APPLE_NOTARIZATION_KEY_ID" \
          --issuer "$APPLE_NOTARIZATION_ISSUER_ID" || true
        echo "Notarization failed for ${label} (submission ${submission_id}, status ${status})"
        exit 1
        ;;
    esac
  done

  echo "Timed out waiting for notarization of ${label} (submission ${submission_id}, last status ${status})"
  exit 1
}
