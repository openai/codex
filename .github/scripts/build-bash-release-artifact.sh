#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 <archive-path>" >&2
  exit 1
fi

archive_path="$1"
workspace="${GITHUB_WORKSPACE:?missing GITHUB_WORKSPACE}"
bash_commit="${BASH_COMMIT:?missing BASH_COMMIT}"
bash_patch="${BASH_PATCH:?missing BASH_PATCH}"
temp_root="${RUNNER_TEMP:-/tmp}"
work_root="$(mktemp -d "${temp_root%/}/codex-bash-release.XXXXXX")"
trap 'rm -rf "$work_root"' EXIT

source_root="${work_root}/bash"
package_root="${work_root}/codex-bash"
wrapper_path="${work_root}/exec-wrapper"
stdout_path="${work_root}/stdout.txt"
wrapper_log_path="${work_root}/wrapper.log"
socket_probe_path="${work_root}/socket-probe.txt"

git clone https://git.savannah.gnu.org/git/bash "$source_root"
cd "$source_root"
git checkout "$bash_commit"
git apply "${workspace}/${bash_patch}"

configure_args=(--without-bash-malloc)
if [[ -n "${BASH_HOST:-}" ]]; then
  configure_args+=(
    --host="$BASH_HOST"
    --disable-nls
    --disable-readline
    --enable-static-link
  )
fi
if [[ -n "${MACOSX_DEPLOYMENT_TARGET:-}" ]]; then
  # The macOS 15 SDK exposes strchrnul even though it is unavailable on our
  # minimum supported macOS version. Use Bash's bundled implementation.
  export bash_cv_func_strchrnul_works=no
fi
./configure "${configure_args[@]}"

cores="$(command -v nproc >/dev/null 2>&1 && nproc || getconf _NPROCESSORS_ONLN)"
make -j"${cores}"

# Stand in for codex-execve-wrapper: record each intercepted executable and
# prove that the inherited escalation-socket descriptor is still open.
cat > "$wrapper_path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${CODEX_WRAPPER_LOG:?missing CODEX_WRAPPER_LOG}"
: "${EXEC_WRAPPER:?missing EXEC_WRAPPER}"
: "${CODEX_ESCALATE_SOCKET:?missing CODEX_ESCALATE_SOCKET}"
printf 'socket-open\n' >&"${CODEX_ESCALATE_SOCKET}"
printf '%s\n' "$@" >> "$CODEX_WRAPPER_LOG"
file="$1"
shift
if [[ "$#" -eq 0 ]]; then
  exec "$file"
fi
arg0="$1"
shift
exec -a "$arg0" "$file" "$@"
EOF
chmod +x "$wrapper_path"

# The nested bash and /bin/echo should each pass through EXEC_WRAPPER while
# retaining the same inherited descriptor.
CODEX_WRAPPER_LOG="$wrapper_log_path" \
CODEX_ESCALATE_SOCKET=9 \
EXEC_WRAPPER="$wrapper_path" \
"${source_root}/bash" \
  -c "\"${source_root}/bash\" -c '/bin/echo smoke-bash'" \
  > "$stdout_path" \
  9> "$socket_probe_path"

grep -Fx "smoke-bash" "$stdout_path"
grep -Fx "${source_root}/bash" "$wrapper_log_path"
grep -Fx "/bin/echo" "$wrapper_log_path"
[[ "$(grep -Fxc "socket-open" "$socket_probe_path")" -eq 2 ]]

if [[ -n "${BASH_HOST:-}" ]]; then
  file "${source_root}/bash" | grep -F "statically linked"
  if readelf -l "${source_root}/bash" | grep -q "INTERP"; then
    echo "bash contains an ELF interpreter despite requesting a static build" >&2
    exit 1
  fi
fi

if [[ -n "${MACOSX_DEPLOYMENT_TARGET:-}" ]]; then
  minos="$(otool -l "${source_root}/bash" | awk '$1 == "minos" { print $2; exit }')"
  if [[ "$minos" != "$MACOSX_DEPLOYMENT_TARGET" ]]; then
    echo "bash has minimum macOS version ${minos}, expected ${MACOSX_DEPLOYMENT_TARGET}" >&2
    exit 1
  fi
  if nm -u "${source_root}/bash" | grep -Eq '[[:space:]]_?strchrnul$'; then
    echo "bash references strchrnul, which is unavailable on macOS 12" >&2
    exit 1
  fi
fi

mkdir -p "$package_root/bin" "$(dirname "${workspace}/${archive_path}")"
cp "${source_root}/bash" "$package_root/bin/bash"
chmod +x "$package_root/bin/bash"

(cd "$work_root" && tar -czf "${workspace}/${archive_path}" codex-bash)
