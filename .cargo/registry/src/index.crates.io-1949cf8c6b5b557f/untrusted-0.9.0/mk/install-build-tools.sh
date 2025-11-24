#!/usr/bin/env bash
#
# Copyright 2020 Brian Smith.
#
# Permission to use, copy, modify, and/or distribute this software for any
# purpose with or without fee is hereby granted, provided that the above
# copyright notice and this permission notice appear in all copies.
#
# THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
# WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
# MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY
# SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
# WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
# OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
# CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

set -eux -o pipefail
IFS=$'\n\t'

target=$1
features=${2-}

function install_packages {
  sudo apt-get -yq --no-install-suggests --no-install-recommends install "$@"
}

use_clang=
case $target in
--target*android*)
  mkdir -p "${ANDROID_SDK_ROOT}/licenses"
  android_license_file="${ANDROID_SDK_ROOT}/licenses/android-sdk-license"
  accept_android_license=24333f8a63b6825ea9c5514f83c2829b004d1fee
  grep --quiet --no-messages "$accept_android_license" "$android_license_file" \
    || echo $accept_android_license  >> "$android_license_file"
  sudo "${ANDROID_SDK_ROOT}/tools/bin/sdkmanager" ndk-bundle
  ;;
esac

case $target in
--target=aarch64-unknown-linux-gnu)
  # Clang is needed for code coverage.
  use_clang=1
  install_packages \
    qemu-user \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross
  ;;
--target=aarch64-unknown-linux-musl|--target=armv7-unknown-linux-musleabihf)
  use_clang=1
  install_packages \
    qemu-user
  ;;
--target=arm-unknown-linux-gnueabihf)
  install_packages \
    qemu-user \
    gcc-arm-linux-gnueabihf \
    libc6-dev-armhf-cross
  ;;
--target=i686-unknown-linux-gnu)
  use_clang=1
  install_packages \
    gcc-multilib \
    libc6-dev-i386
  ;;
--target=i686-unknown-linux-musl|--target=x86_64-unknown-linux-musl)
  use_clang=1
  ;;
--target=wasm32-unknown-unknown)
  # The version of wasm-bindgen-cli must match the wasm-bindgen version.
  wasm_bindgen_version=$(cargo metadata --format-version 1 | jq -r '.packages | map(select( .name == "wasm-bindgen")) | map(.version) | .[0]')
  cargo install wasm-bindgen-cli --vers "$wasm_bindgen_version" --bin wasm-bindgen-test-runner
  case ${features-} in
    *wasm32_c*)
      use_clang=1
      ;;
    *)
      ;;
  esac
  ;;
--target=*)
  ;;
esac

if [ -n "$use_clang" ]; then
  llvm_version=10
  if [ -n "${RING_COVERAGE-}" ]; then
    # https://github.com/rust-lang/rust/pull/79365 upgraded the coverage file
    # format to one that only LLVM 11+ can use
    llvm_version=11
    sudo apt-key add mk/llvm-snapshot.gpg.key
    sudo add-apt-repository "deb http://apt.llvm.org/bionic/ llvm-toolchain-bionic-$llvm_version main"
    sudo apt-get update
  fi
  install_packages clang-$llvm_version llvm-$llvm_version
fi
