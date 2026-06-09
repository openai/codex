#!/usr/bin/env bash

set -euo pipefail

workspace="${TEST_SRCDIR}/${TEST_WORKSPACE}"
build_file="${workspace}/BUILD.bazel"
module_file="${workspace}/MODULE.bazel"
llvm_patch="${workspace}/patches/llvm_windows_gnullvm_abi.patch"
llvm_runtime_patch="${workspace}/patches/llvm_windows_arm64_powl.patch"

require_line() {
  local file="$1"
  local line="$2"
  if ! grep -Fqx -- "$line" "$file"; then
    echo "missing '$line' in $file" >&2
    exit 1
  fi
}

require_platform_constraint() {
  local platform="$1"
  local constraint="$2"
  local block
  block="$(sed -n "/name = \"${platform}\"/,/^)/p" "$build_file")"
  if ! grep -Fq -- "\"${constraint}\"," <<<"$block"; then
    echo "platform '$platform' is missing constraint '$constraint'" >&2
    exit 1
  fi
}

require_count() {
  local expected="$1"
  local file="$2"
  local line="$3"
  local actual
  actual="$(grep -Fxc -- "$line" "$file" || true)"
  if [[ "$actual" != "$expected" ]]; then
    echo "expected $expected copies of '$line' in $file, found $actual" >&2
    exit 1
  fi
}

require_platform_constraint local_windows "@llvm//constraints/windows_abi:gnullvm"
require_platform_constraint local_windows_msvc "@llvm//constraints/windows_abi:gnullvm"
require_platform_constraint local_windows_msvc "@platforms//cpu:x86_64"
require_platform_constraint local_windows_msvc "@platforms//os:windows"
require_platform_constraint local_windows_msvc "@rules_rs//rs/experimental/platforms/constraints:windows_msvc"
require_platform_constraint windows_x86_64_gnullvm "@llvm//constraints/windows_abi:gnullvm"
require_platform_constraint windows_x86_64_msvc "@llvm//constraints/windows_abi:msvc"
require_platform_constraint release_windows_arm64 "@llvm//constraints/windows_abi:gnullvm"
require_line "$module_file" '        "//patches:llvm_windows_gnullvm_abi.patch",'
require_line "$module_file" '        "//patches:llvm_windows_arm64_powl.patch",'
require_count 2 "$llvm_patch" '+                    "@llvm//constraints/windows_abi:gnullvm",'
require_line "$llvm_runtime_patch" '+    "math/arm-common/powl.c",'
require_line "$llvm_runtime_patch" '+        "-lmingwex",'
