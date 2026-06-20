#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "This audit has no configurable arguments." >&2
  exit 2
fi
if [[ "${RUNNER_OS:-}" != "Windows" ]]; then
  echo "The Windows Bazel hermeticity audit must run on a Windows runner." >&2
  exit 1
fi
if [[ -z "${RUNNER_TEMP:-}" ]]; then
  echo "RUNNER_TEMP must be set." >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

native_aquery="${RUNNER_TEMP}/windows-hermetic-native-aquery.json"
v8_aquery="${RUNNER_TEMP}/windows-hermetic-v8-aquery.json"
lint_aquery="${RUNNER_TEMP}/windows-hermetic-lint-aquery.json"

native_targets='deps(set(
  @crates//:aws-lc-sys-0.39.0
  @crates//:ring-0.17.14
  @crates//:zstd-0.13.3
  //codex-rs/codex-experimental-api-macros:codex-experimental-api-macros
  //tools/windows-toolchain:stack-protector-probe
))'
v8_actions='mnemonic(
  "(Action|CargoBuildScriptRun|CppArchive|CppLink|RunBinary|Rustc|V8Mksnapshot)",
  deps(//codex-rs/v8-poc:v8-poc)
)'

./.github/scripts/run-bazel-ci.sh \
  -- \
  aquery \
  --host_platform=//:local_windows \
  --platforms=//:windows_x86_64_gnullvm \
  --extra_execution_platforms=//:windows_x86_64_gnullvm \
  --extra_toolchains=//:windows_gnullvm_tests_on_gnullvm_host_toolchain \
  --include_aspects \
  --include_param_files \
  --output=jsonproto \
  "--output_file=${native_aquery}" \
  -- \
  "${native_targets}"

./.github/scripts/run-bazel-ci.sh \
  -- \
  aquery \
  --host_platform=//:local_windows \
  --platforms=//:windows_x86_64_gnullvm \
  --extra_execution_platforms=//:windows_x86_64_gnullvm \
  --extra_toolchains=//:windows_gnullvm_tests_on_gnullvm_host_toolchain \
  --include_aspects \
  --include_param_files \
  --output=jsonproto \
  "--output_file=${v8_aquery}" \
  -- \
  "${v8_actions}"

./.github/scripts/run-bazel-ci.sh \
  -- \
  aquery \
  --config=argument-comment-lint \
  --host_platform=//:local_windows \
  --platforms=//:windows_x86_64_gnullvm \
  --extra_execution_platforms=//:windows_x86_64_gnullvm \
  --extra_toolchains=//:windows_gnullvm_tests_on_gnullvm_host_toolchain \
  --include_aspects \
  --include_param_files \
  --output=jsonproto \
  "--output_file=${lint_aquery}" \
  -- \
  //codex-rs/codex-experimental-api-macros:codex-experimental-api-macros

python .github/scripts/audit_bazel_windows_hermeticity.py \
  "${native_aquery}" \
  "${v8_aquery}" \
  "${lint_aquery}"
