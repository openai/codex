#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: $0 <bash-path> <zsh-path>" >&2
  exit 1
fi

bash_path="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
zsh_path="$(cd "$(dirname "$2")" && pwd)/$(basename "$2")"
temp_root="${RUNNER_TEMP:-/tmp}"
work_root="$(mktemp -d "${temp_root%/}/codex-shell-chain.XXXXXX")"
trap 'rm -rf "$work_root"' EXIT

wrapper_path="${work_root}/exec-wrapper"

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

run_chain() {
  local outer_shell="$1"
  local outer_flag="$2"
  local inner_shell="$3"
  local inner_flag="$4"
  local marker="$5"
  local wrapper_log="${work_root}/${marker}-wrapper.log"
  local socket_probe="${work_root}/${marker}-socket.log"
  local stdout="${work_root}/${marker}-stdout.txt"
  local command
  command="\"${inner_shell}\" ${inner_flag} '/bin/echo ${marker}'"

  CODEX_WRAPPER_LOG="$wrapper_log" \
  CODEX_ESCALATE_SOCKET=9 \
  EXEC_WRAPPER="$wrapper_path" \
  "$outer_shell" "$outer_flag" "$command" > "$stdout" 9> "$socket_probe"

  grep -Fx "$marker" "$stdout"
  grep -Fx "$inner_shell" "$wrapper_log"
  grep -Fx "/bin/echo" "$wrapper_log"
  [[ "$(grep -Fxc "socket-open" "$socket_probe")" -eq 2 ]]
}

run_chain "$bash_path" -c "$zsh_path" -fc bash-zsh-chain
run_chain "$zsh_path" -fc "$bash_path" -c zsh-bash-chain
