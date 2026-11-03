#!/usr/bin/env bash

set -euo pipefail

bazel_lint_args=("$@")
if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
  # Some Rust top-level targets are still intentionally incompatible with the
  # local Windows exec platform. Skip those explicit targets so the native
  # lint aspect can run across the compatible crate graph instead of failing the
  # whole build after analysis.
  bazel_lint_args+=("--skip_incompatible_explicit_targets")
  if [[ -n "${VOICE_WINDOWS_BAZEL_REPOSITORY:-}" ]]; then
    : "${VOICE_WINDOWS_BAZEL_REPOSITORY:?Windows voice tools are required}"
    : "${VOICE_WINDOWS_SYSTEM_ROOT:?Windows SystemRoot is required}"
    : "${VOICE_WINDOWS_HOST_ARCH:?Windows host architecture is required}"
    bazel_lint_args+=(
      "--extra_toolchains=//third_party/voice:windows_pkg_config_toolchain,//third_party/voice:windows_cmake_toolchain"
      "--repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1"
      "--inject_repository=voice_windows_tools=${VOICE_WINDOWS_BAZEL_REPOSITORY}"
      "--//third_party/voice:windows_installed_tools=@voice_windows_tools//:tools"
      "--action_env=SystemRoot=${VOICE_WINDOWS_SYSTEM_ROOT}"
      "--host_action_env=SystemRoot=${VOICE_WINDOWS_SYSTEM_ROOT}"
      "--action_env=PROCESSOR_ARCHITECTURE=${VOICE_WINDOWS_HOST_ARCH}"
      "--host_action_env=PROCESSOR_ARCHITECTURE=${VOICE_WINDOWS_HOST_ARCH}"
    )
  fi
fi

read_query_labels() {
  local query="$1"
  local query_stdout
  local query_stderr
  query_stdout="$(mktemp)"
  query_stderr="$(mktemp)"

  if ! ./.github/scripts/run-bazel-query-ci.sh \
    --keep_going \
    --output=label \
    -- "$query" >"$query_stdout" 2>"$query_stderr"; then
    cat "$query_stderr" >&2
    rm -f "$query_stdout" "$query_stderr"
    exit 1
  fi

  cat "$query_stdout"
  rm -f "$query_stdout" "$query_stderr"
}

final_build_targets=(//codex-rs/...)
if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
  # Bazel's local Windows platform currently lacks a default test toolchain for
  # `rust_test`, so target the concrete Rust crate rules directly. The lint
  # aspect still walks their crate graph, which preserves incremental reuse for
  # non-test code while avoiding non-Rust wrapper targets such as platform_data.
  final_build_targets=()
  while IFS= read -r label; do
    [[ -n "$label" ]] || continue
    final_build_targets+=("$label")
  done < <(read_query_labels 'kind("rust_(library|binary|proc_macro) rule", //codex-rs/...)')

  if [[ ${#final_build_targets[@]} -eq 0 ]]; then
    echo "Failed to discover Windows Bazel lint targets." >&2
    exit 1
  fi
fi

./.github/scripts/run-bazel-ci.sh \
  -- \
  build \
  "${bazel_lint_args[@]}" \
  -- \
  "${final_build_targets[@]}"
