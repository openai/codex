#!/usr/bin/env bash

# Small compatibility wrapper around native codesign and rcodesign.
#
# Existing packaging scripts call this instead of choosing a signing backend
# directly. OAI_CODESIGN_BACKEND=akv-pkcs11 routes signing through rcodesign
# while preserving the option, entitlement, identifier, timestamp, and deep
# signing surface used by the native codesign path.

set -euo pipefail

usage() {
  cat >&2 <<'EOF'
Usage: sign_macos_code.sh --target PATH --identity IDENTITY [options]

Options:
  --deep true|false
  --entitlements PATH
  --identifier IDENTIFIER
  --identity IDENTITY
  --options FLAGS
  --target PATH
  --timestamp true|false|none
EOF
}

target=""
identity=""
options=""
entitlements_file=""
identifier=""
deep="false"
timestamp="true"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deep)
      deep="${2:-}"
      shift 2
      ;;
    --entitlements)
      entitlements_file="${2:-}"
      shift 2
      ;;
    --identifier)
      identifier="${2:-}"
      shift 2
      ;;
    --identity)
      identity="${2:-}"
      shift 2
      ;;
    --options)
      options="${2:-}"
      shift 2
      ;;
    --target)
      target="${2:-}"
      shift 2
      ;;
    --timestamp)
      timestamp="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown signing argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$target" ]]; then
  echo "--target is required." >&2
  usage
  exit 2
fi

if [[ ! -e "$target" ]]; then
  echo "Signing target does not exist: $target" >&2
  exit 1
fi

case "$deep" in
  true|false) ;;
  *)
    echo "--deep must be true or false, got '$deep'." >&2
    exit 2
    ;;
esac

case "$timestamp" in
  true|false|none) ;;
  *)
    echo "--timestamp must be true, false, or none, got '$timestamp'." >&2
    exit 2
    ;;
esac

sign_with_codesign() {
  if [[ -z "$identity" ]]; then
    echo "Native codesign requires --identity." >&2
    exit 2
  fi

  local -a args
  args=(--force)

  if [[ "$deep" == "true" ]]; then
    args+=(--deep)
  fi

  if [[ -n "$options" ]]; then
    args+=(--options "$options")
  fi

  case "$timestamp" in
    true)
      args+=(--timestamp)
      ;;
    false|none)
      args+=(--timestamp=none)
      ;;
  esac

  if [[ -n "$entitlements_file" ]]; then
    args+=(--entitlements "$entitlements_file")
  fi

  if [[ -n "$identifier" ]]; then
    args+=(--identifier "$identifier")
  fi

  args+=(--sign "$identity" "$target")
  codesign "${args[@]}"
}

append_rcodesign_flags() {
  local raw_options="$1"
  local option=""

  if [[ -z "$raw_options" ]]; then
    return 0
  fi

  IFS=',' read -ra split_options <<< "$raw_options"
  for option in "${split_options[@]}"; do
    option="${option//[[:space:]]/}"
    [[ -z "$option" ]] && continue

    case "$option" in
      host|hard|kill|expires|restrict|library|runtime|linker-signed)
        rcodesign_args+=(--code-signature-flags "$option")
        ;;
      *)
        echo "Unsupported rcodesign code signature option: $option" >&2
        exit 2
        ;;
    esac
  done
}

rcodesign_options_require_notarization() {
  local raw_options="$1"
  local option=""

  if [[ -z "$raw_options" || "$timestamp" != "true" ]]; then
    return 1
  fi

  IFS=',' read -ra split_options <<< "$raw_options"
  for option in "${split_options[@]}"; do
    option="${option//[[:space:]]/}"
    if [[ "$option" == "runtime" ]]; then
      return 0
    fi
  done

  return 1
}

sign_with_rcodesign() {
  : "${OAI_AKV_PKCS11_LIBRARY:?OAI_AKV_PKCS11_LIBRARY is required for AKV PKCS11 signing.}"
  : "${OAI_AKV_SIGNING_CERTIFICATE_PEM:?OAI_AKV_SIGNING_CERTIFICATE_PEM is required for AKV PKCS11 signing.}"
  : "${OAI_AKV_KEY_LABEL:?OAI_AKV_KEY_LABEL is required for AKV PKCS11 signing.}"

  if ! command -v rcodesign >/dev/null 2>&1; then
    echo "rcodesign was not found on PATH." >&2
    exit 1
  fi

  local -a rcodesign_args
  rcodesign_args=(
    sign
    --config-file /dev/null
    --pkcs11-library "$OAI_AKV_PKCS11_LIBRARY"
    --pkcs11-certificate-file "$OAI_AKV_SIGNING_CERTIFICATE_PEM"
    --pkcs11-key-label "$OAI_AKV_KEY_LABEL"
  )

  if [[ "$deep" == "false" ]]; then
    rcodesign_args+=(--shallow)
  fi

  case "$timestamp" in
    true)
      ;;
    false|none)
      rcodesign_args+=(--timestamp-url none)
      ;;
  esac

  append_rcodesign_flags "$options"
  if rcodesign_options_require_notarization "$options"; then
    rcodesign_args+=(--for-notarization)
  fi

  if [[ -n "$entitlements_file" ]]; then
    rcodesign_args+=(--entitlements-xml-file "$entitlements_file")
  fi

  if [[ -n "$identifier" ]]; then
    rcodesign_args+=(--binary-identifier "$identifier")
  fi

  rcodesign_args+=("$target")
  rcodesign "${rcodesign_args[@]}"
}

case "${OAI_CODESIGN_BACKEND:-codesign}" in
  codesign|"")
    sign_with_codesign
    ;;
  akv-pkcs11)
    sign_with_rcodesign
    ;;
  *)
    echo "Unsupported OAI_CODESIGN_BACKEND: ${OAI_CODESIGN_BACKEND}" >&2
    exit 2
    ;;
esac
