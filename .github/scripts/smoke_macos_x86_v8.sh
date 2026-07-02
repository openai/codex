#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "usage: $0 <archive> <binding> <sandbox:true|false> <cargo-target-dir>" >&2
  exit 2
}

[[ $# -eq 4 ]] || usage

archive="$1"
binding="$2"
sandbox="$3"
target_dir="$4"

case "$sandbox" in
  true | false) ;;
  *) usage ;;
esac

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "x86_64" ]]; then
  echo "Intel macOS V8 smoke must run natively on x86_64 macOS." >&2
  exit 1
fi
if [[ ! -f "$archive" || ! -f "$binding" ]]; then
  echo "V8 archive or binding is missing." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
production_entitlements="${repo_root}/.github/scripts/macos-signing/codex.entitlements.plist"
expanded_entitlements="${target_dir}/codex-allow-unsigned-executable-memory.entitlements.plist"
crash_report_dir="${target_dir}/crash-reports"
crash_report_marker="${target_dir}/runtime-smoke-started"

mkdir -p "$target_dir" "$crash_report_dir"
touch "$crash_report_marker"
cp "$production_entitlements" "$expanded_entitlements"
if /usr/libexec/PlistBuddy \
  -c "Print :com.apple.security.cs.allow-unsigned-executable-memory" \
  "$expanded_entitlements" >/dev/null 2>&1; then
  /usr/libexec/PlistBuddy \
    -c "Set :com.apple.security.cs.allow-unsigned-executable-memory true" \
    "$expanded_entitlements"
else
  /usr/libexec/PlistBuddy \
    -c "Add :com.apple.security.cs.allow-unsigned-executable-memory bool true" \
    "$expanded_entitlements"
fi

cargo_features=()
if [[ "$sandbox" == "true" ]]; then
  cargo_features=(--features sandbox)
fi

(
  cd "${repo_root}/codex-rs"
  export CARGO_TARGET_DIR="$target_dir"
  export RUSTY_V8_ARCHIVE="$archive"
  export RUSTY_V8_SRC_BINDING_PATH="$binding"

  cargo test -p codex-v8-poc "${cargo_features[@]}"
  cargo build --release -p codex-v8-poc --example code_mode_runtime_smoke \
    "${cargo_features[@]}"
)

probe="${target_dir}/release/examples/code_mode_runtime_smoke"
if [[ ! -x "$probe" ]]; then
  echo "Runtime smoke executable was not built at $probe." >&2
  exit 1
fi
if [[ "$(lipo -archs "$probe")" != "x86_64" ]]; then
  echo "Runtime smoke executable is not x86_64: $(lipo -archs "$probe")" >&2
  exit 1
fi

failures=0
for entitlement_profile in production allow-unsigned-executable-memory; do
  case "$entitlement_profile" in
    production) entitlements="$production_entitlements" ;;
    allow-unsigned-executable-memory) entitlements="$expanded_entitlements" ;;
  esac

  signed_probe="${target_dir}/code_mode_runtime_smoke-${entitlement_profile}"
  cp "$probe" "$signed_probe"
  codesign --force --sign - --options runtime --entitlements "$entitlements" "$signed_probe"
  codesign --verify --strict --verbose=2 "$signed_probe"

  for provider in ring aws-lc; do
    echo "Running code-mode runtime smoke: entitlements=${entitlement_profile} provider=${provider}"
    if "$signed_probe" "$provider"; then
      echo "PASS entitlements=${entitlement_profile} provider=${provider}"
    else
      status=$?
      echo "FAIL entitlements=${entitlement_profile} provider=${provider} status=${status}" >&2
      failures=$((failures + 1))
    fi
  done
done

diagnostic_reports="${HOME}/Library/Logs/DiagnosticReports"
if [[ -d "$diagnostic_reports" ]]; then
  find "$diagnostic_reports" -type f -name '*.ips' -newer "$crash_report_marker" \
    -exec cp {} "$crash_report_dir" \;
fi

if ((failures > 0)); then
  echo "$failures Intel macOS code-mode runtime smoke configuration(s) failed." >&2
  exit 1
fi
