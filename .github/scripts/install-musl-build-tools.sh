#!/usr/bin/env bash
set -euo pipefail

: "${TARGET:?TARGET environment variable is required}"
: "${GITHUB_ENV:?GITHUB_ENV environment variable is required}"

apt_update_args=()
if [[ -n "${APT_UPDATE_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  apt_update_args=(${APT_UPDATE_ARGS})
fi

apt_install_args=()
if [[ -n "${APT_INSTALL_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  apt_install_args=(${APT_INSTALL_ARGS})
fi

sudo apt-get update "${apt_update_args[@]}"
sudo apt-get install -y "${apt_install_args[@]}" musl-tools pkg-config g++ clang libc++-dev libc++abi-dev lld

case "${TARGET}" in
  x86_64-unknown-linux-musl)
    arch="x86_64"
    ;;
  aarch64-unknown-linux-musl)
    arch="aarch64"
    ;;
  *)
    echo "Unexpected musl target: ${TARGET}" >&2
    exit 1
    ;;
esac

if command -v clang++ >/dev/null; then
  cxx="$(command -v clang++)"
  echo "CXXFLAGS=--target=${TARGET} -stdlib=libc++ -pthread" >> "$GITHUB_ENV"
  echo "CFLAGS=--target=${TARGET} -pthread" >> "$GITHUB_ENV"
  if command -v clang >/dev/null; then
    cc="$(command -v clang)"
    echo "CC=${cc}" >> "$GITHUB_ENV"
    echo "TARGET_CC=${cc}" >> "$GITHUB_ENV"
    target_cc_var="CC_${TARGET}"
    target_cc_var="${target_cc_var//-/_}"
    echo "${target_cc_var}=${cc}" >> "$GITHUB_ENV"
  fi
elif command -v "${arch}-linux-musl-g++" >/dev/null; then
  cxx="$(command -v "${arch}-linux-musl-g++")"
elif command -v musl-g++ >/dev/null; then
  cxx="$(command -v musl-g++)"
elif command -v musl-gcc >/dev/null; then
  cxx="$(command -v musl-gcc)"
  echo "CFLAGS=-pthread" >> "$GITHUB_ENV"
else
  echo "musl g++ not found after install; arch=${arch}" >&2
  exit 1
fi

echo "CXX=${cxx}" >> "$GITHUB_ENV"
echo "CMAKE_CXX_COMPILER=${cxx}" >> "$GITHUB_ENV"
echo "CMAKE_ARGS=-DCMAKE_HAVE_THREADS_LIBRARY=1 -DCMAKE_USE_PTHREADS_INIT=1 -DCMAKE_THREAD_LIBS_INIT=-pthread -DTHREADS_PREFER_PTHREAD_FLAG=ON" >> "$GITHUB_ENV"
