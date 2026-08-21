#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
minimum_supported_release="$(
  sed -n 's/^pub const MINIMUM_SUPPORTED_CODEX_VERSION: &str = "\([^"]*\)";$/\1/p' \
    "${repo_root}/codex-rs/exec-server-protocol/src/lib.rs"
)"
: "${minimum_supported_release:?minimum supported Codex release is missing}"
release_directory="$(mktemp -d "${TMPDIR:-/tmp}/codex-exec-server-skew.XXXXXX")"
trap 'rm -rf "${release_directory:?}"' EXIT

if [[ $# -eq 0 ]]; then
  releases=(latest "${minimum_supported_release}")
else
  releases=("$@")
fi

case "$(uname -s):$(uname -m)" in
  Darwin:arm64) target="aarch64-apple-darwin" ;;
  Darwin:x86_64) target="x86_64-apple-darwin" ;;
  Linux:aarch64 | Linux:arm64) target="aarch64-unknown-linux-musl" ;;
  Linux:x86_64) target="x86_64-unknown-linux-musl" ;;
  *)
    echo "Unsupported platform: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

asset="codex-${target}.tar.gz"

if [[ "$(uname -s)" == "Linux" ]] \
  && command -v apt-get >/dev/null \
  && command -v dpkg-deb >/dev/null \
  && command -v dpkg-architecture >/dev/null \
  && ! pkg-config --exists alsa glib-2.0 gobject-2.0 gio-2.0; then
  # Debian-like hosts can download development metadata without sudo. The
  # temporary sysroot still relies on runtime libraries installed on the host.
  voice_package_directory="${release_directory}/voice-packages"
  voice_sysroot="${release_directory}/voice-sysroot"
  mkdir -p "${voice_package_directory}" "${voice_sysroot}"
  (
    cd "${voice_package_directory}"
    apt-get download libasound2-dev libglib2.0-dev libcap-dev libffi-dev libmount-dev \
      libpcre2-dev libselinux1-dev zlib1g-dev libblkid-dev libsepol-dev
  )
  for package in "${voice_package_directory}"/*.deb; do
    dpkg-deb --extract "${package}" "${voice_sysroot}"
  done

  voice_multiarch="$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
  voice_library_directory="${voice_sysroot}/usr/lib/${voice_multiarch}"
  for library in "${voice_library_directory}"/*.so; do
    [[ -L "${library}" ]] || continue
    library_target="$(readlink "${library}")"
    if [[ ! -e "${voice_library_directory}/${library_target}" && -e "/usr/lib/${voice_multiarch}/${library_target}" ]]; then
      ln -s "/usr/lib/${voice_multiarch}/${library_target}" "${voice_library_directory}/${library_target}"
    fi
  done

  export PKG_CONFIG_PATH="${voice_library_directory}/pkgconfig${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"
  export PKG_CONFIG_SYSROOT_DIR="${voice_sysroot}"
fi

cd "${repo_root}/codex-rs"
cargo build -p codex-cli --bin codex
export CODEX_TEST_CURRENT_CODEX="${CARGO_TARGET_DIR:-${repo_root}/codex-rs/target}/debug/codex"

echo "Testing current Codex compatibility through authenticated Noise"
export CODEX_TEST_RELEASED_CODEX="${CODEX_TEST_CURRENT_CODEX}"
just test -p codex-exec-server --test relay version_skew --test-threads 1

tested_release_version=""
for release in "${releases[@]}"; do
  release="${release#rust-v}"
  if [[ "${release}" == "${tested_release_version}" ]]; then
    echo "Skipping Codex ${release}; this release was already tested"
    continue
  fi

  if [[ "${release}" == "latest" ]]; then
    release_url="https://github.com/openai/codex/releases/latest/download/${asset}"
  else
    release_url="https://github.com/openai/codex/releases/download/rust-v${release}/${asset}"
  fi

  binary_directory="${release_directory}/${release}"
  mkdir -p "${binary_directory}"
  echo "Downloading released Codex from ${release_url}"
  curl -fsSL "${release_url}" -o "${binary_directory}/${asset}"
  tar -xzf "${binary_directory}/${asset}" -C "${binary_directory}"

  export CODEX_TEST_RELEASED_CODEX="${binary_directory}/codex-${target}"
  release_output="$("${CODEX_TEST_RELEASED_CODEX}" --version)"
  echo "${release_output}"
  tested_release_version="${release_output##* }"

  just test -p codex-exec-server --test relay version_skew --test-threads 1
done
