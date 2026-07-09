#!/usr/bin/env bash

set -euo pipefail

bazel_lint_args=("$@")
if [[ "${RUNNER_OS:-}" == "Windows" ]]; then
  has_host_platform_override=0
  for arg in "${bazel_lint_args[@]}"; do
    if [[ "$arg" == --host_platform=* ]]; then
      has_host_platform_override=1
      break
    fi
  done

  if [[ $has_host_platform_override -eq 0 ]]; then
    # The nightly Windows lint toolchain is registered with an MSVC exec
    # platform even though the lint target platform stays on `windows-gnullvm`.
    # Override the host platform here so the exec-side helper binaries actually
    # match the registered toolchain set.
    bazel_lint_args+=("--host_platform=//:local_windows_msvc")
  fi

  # Native Windows lint runs need exec-side Rust helper binaries and proc-macros
  # to use rust-lld instead of the C++ linker path. The default `none`
  # preference resolves to `cc` when a cc_toolchain is present, which currently
  # routes these exec actions through clang++ with an argument shape it cannot
  # consume.
  bazel_lint_args+=("--@rules_rust//rust/settings:toolchain_linker_preference=rust")

  # Some Rust top-level targets are still intentionally incompatible with the
  # local Windows MSVC exec platform. Skip those explicit targets so the native
  # lint aspect can run across the compatible crate graph instead of failing the
  # whole build after analysis.
  bazel_lint_args+=("--skip_incompatible_explicit_targets")
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
