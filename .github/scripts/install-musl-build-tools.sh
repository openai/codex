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

if command -v "${arch}-linux-musl-gcc" >/dev/null; then
  cc="$(command -v "${arch}-linux-musl-gcc")"
elif command -v musl-gcc >/dev/null; then
  cc="$(command -v musl-gcc)"
else
  echo "musl gcc not found after install; arch=${arch}" >&2
  exit 1
fi

sysroot="$("$cc" -print-sysroot 2>/dev/null || "$cc" --print-sysroot 2>/dev/null || true)"
if [[ -z "${sysroot}" || "${sysroot}" == "/" ]]; then
  libc_path="$("$cc" -print-file-name=libc.a 2>/dev/null || true)"
  if [[ -n "${libc_path}" && "${libc_path}" != "libc.a" && -f "${libc_path}" ]]; then
    sysroot="$(cd "$(dirname "${libc_path}")/.." && pwd)"
  fi
fi
if [[ -n "${sysroot}" && "${sysroot}" != "/" ]]; then
  echo "BORING_BSSL_SYSROOT=${sysroot}" >> "$GITHUB_ENV"
  boring_sysroot_var="BORING_BSSL_SYSROOT_${TARGET}"
  boring_sysroot_var="${boring_sysroot_var//-/_}"
  echo "${boring_sysroot_var}=${sysroot}" >> "$GITHUB_ENV"
fi

echo "CFLAGS=-pthread" >> "$GITHUB_ENV"
echo "CC=${cc}" >> "$GITHUB_ENV"
echo "TARGET_CC=${cc}" >> "$GITHUB_ENV"
target_cc_var="CC_${TARGET}"
target_cc_var="${target_cc_var//-/_}"
echo "${target_cc_var}=${cc}" >> "$GITHUB_ENV"

if command -v "${arch}-linux-musl-g++" >/dev/null; then
  cxx="$(command -v "${arch}-linux-musl-g++")"
  cxxflags="-pthread"
elif command -v musl-g++ >/dev/null; then
  cxx="$(command -v musl-g++)"
  cxxflags="-pthread"
elif command -v clang++ >/dev/null; then
  cxx="$(command -v clang++)"
  cxxflags="--target=${TARGET} -stdlib=libc++ -pthread"
  if [[ -n "${sysroot}" && "${sysroot}" != "/" ]]; then
    cxxflags="${cxxflags} --sysroot=${sysroot}"
  fi
  echo "BORING_BSSL_RUST_CPPLIB=c++" >> "$GITHUB_ENV"
  boring_cpp_var="BORING_BSSL_RUST_CPPLIB_${TARGET}"
  boring_cpp_var="${boring_cpp_var//-/_}"
  echo "${boring_cpp_var}=c++" >> "$GITHUB_ENV"
else
  cxx="${cc}"
  cxxflags="-pthread"
fi

echo "CXXFLAGS=${cxxflags}" >> "$GITHUB_ENV"
echo "CXX=${cxx}" >> "$GITHUB_ENV"
echo "TARGET_CXX=${cxx}" >> "$GITHUB_ENV"
target_cxx_var="CXX_${TARGET}"
target_cxx_var="${target_cxx_var//-/_}"
echo "${target_cxx_var}=${cxx}" >> "$GITHUB_ENV"
echo "CMAKE_CXX_COMPILER=${cxx}" >> "$GITHUB_ENV"
echo "CMAKE_ARGS=-DCMAKE_HAVE_THREADS_LIBRARY=1 -DCMAKE_USE_PTHREADS_INIT=1 -DCMAKE_THREAD_LIBS_INIT=-pthread -DTHREADS_PREFER_PTHREAD_FLAG=ON" >> "$GITHUB_ENV"
