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

# For aarch64 musl, plain clang --target may pick up glibc headers (leading to
# missing __isoc23_* symbols) and BoringSSL treats large stack frames as
# errors. Use Zig for a musl sysroot and append warning overrides last.
if [[ "${TARGET}" == "aarch64-unknown-linux-musl" ]]; then
  if ! command -v zig >/dev/null; then
    echo "zig is required for ${TARGET} (install via ziglang/setup-zig)" >&2
    exit 1
  fi

  tools_dir="${RUNNER_TEMP:-/tmp}/codex-musl-tools-${TARGET}"
  mkdir -p "${tools_dir}"

  zigcc="${tools_dir}/zigcc"
  zigcxx="${tools_dir}/zigcxx"

  cat >"${zigcc}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
exec zig cc "\$@" -target ${TARGET} -Wno-frame-larger-than -Wno-error=frame-larger-than
EOF

  cat >"${zigcxx}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
exec zig c++ "\$@" -target ${TARGET} -Wno-frame-larger-than -Wno-error=frame-larger-than
EOF

  chmod +x "${zigcc}" "${zigcxx}"

  target_cc_var="CC_${TARGET}"
  target_cc_var="${target_cc_var//-/_}"

  triple="${TARGET//-/_}"
  triple="${triple^^}"
  cargo_linker_var="CARGO_TARGET_${triple}_LINKER"

  echo "CC=${zigcc}" >> "$GITHUB_ENV"
  echo "TARGET_CC=${zigcc}" >> "$GITHUB_ENV"
  echo "${target_cc_var}=${zigcc}" >> "$GITHUB_ENV"
  echo "${cargo_linker_var}=${zigcc}" >> "$GITHUB_ENV"

  echo "CXX=${zigcxx}" >> "$GITHUB_ENV"
  echo "CMAKE_C_COMPILER=${zigcc}" >> "$GITHUB_ENV"
  echo "CMAKE_CXX_COMPILER=${zigcxx}" >> "$GITHUB_ENV"
  echo "CMAKE_ASM_COMPILER=${zigcc}" >> "$GITHUB_ENV"

  # Keep flags minimal; the wrappers provide target + warning overrides last.
  echo "CFLAGS=-pthread" >> "$GITHUB_ENV"
  echo "CXXFLAGS=-pthread -stdlib=libc++" >> "$GITHUB_ENV"

  echo "CMAKE_ARGS=-DCMAKE_HAVE_THREADS_LIBRARY=1 -DCMAKE_USE_PTHREADS_INIT=1 -DCMAKE_THREAD_LIBS_INIT=-pthread -DTHREADS_PREFER_PTHREAD_FLAG=ON" >> "$GITHUB_ENV"
  exit 0
fi

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
