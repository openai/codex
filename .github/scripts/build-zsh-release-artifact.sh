#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 1 ]]; then
  echo "usage: $0 <archive-path>" >&2
  exit 1
fi

archive_path="$1"
workspace="${GITHUB_WORKSPACE:?missing GITHUB_WORKSPACE}"
zsh_commit="${ZSH_COMMIT:?missing ZSH_COMMIT}"
zsh_patch="${ZSH_PATCH:?missing ZSH_PATCH}"
temp_root="${RUNNER_TEMP:-/tmp}"
work_root="$(mktemp -d "${temp_root%/}/codex-zsh-release.XXXXXX")"
trap 'rm -rf "$work_root"' EXIT

source_root="${work_root}/zsh"
package_root="${work_root}/codex-zsh"
wrapper_path="${work_root}/exec-wrapper"
stdout_path="${work_root}/stdout.txt"
wrapper_log_path="${work_root}/wrapper.log"
socket_probe_path="${work_root}/socket-probe.txt"

git clone https://git.code.sf.net/p/zsh/code "$source_root"
cd "$source_root"
git checkout "$zsh_commit"
git apply "${workspace}/${zsh_patch}"
./Util/preconfig
./configure

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

# The nested zsh and /bin/echo should each pass through EXEC_WRAPPER while
# retaining the same inherited descriptor.
CODEX_WRAPPER_LOG="$wrapper_log_path" \
CODEX_ESCALATE_SOCKET=9 \
EXEC_WRAPPER="$wrapper_path" \
"${source_root}/Src/zsh" \
  -fc "\"${source_root}/Src/zsh\" -fc '/bin/echo smoke-zsh'" \
  > "$stdout_path" \
  9> "$socket_probe_path"

grep -Fx "smoke-zsh" "$stdout_path"
grep -Fx "${source_root}/Src/zsh" "$wrapper_log_path"
grep -Fx "/bin/echo" "$wrapper_log_path"
[[ "$(grep -Fxc "socket-open" "$socket_probe_path")" -eq 2 ]]

mkdir -p "$package_root/bin" "$(dirname "${workspace}/${archive_path}")"
cp "${source_root}/Src/zsh" "$package_root/bin/zsh"
chmod +x "$package_root/bin/zsh"

(cd "$work_root" && tar -czf "${workspace}/${archive_path}" codex-zsh)
