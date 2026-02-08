#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEX_RS_DIR="$ROOT_DIR/codex-rs"

usage() {
  cat <<'EOF'
Run an execpolicy prefix-safety matrix and optionally audit an existing rules file.

Usage:
  scripts/execpolicy_prefix_matrix.sh
  scripts/execpolicy_prefix_matrix.sh --audit /path/to/default.rules

Notes:
  - Runs `cargo run -p codex-execpolicy -- check ...` from codex-rs/.
  - Default mode compares broad vs narrow rules for python3/npm/git push.
  - Audit mode probes a real rules file for broad/risky allow matches.
EOF
}

execpolicy_check() {
  local rules_file="$1"
  shift

  (
    cd "$CODEX_RS_DIR"
    cargo run -q -p codex-execpolicy -- check --rules "$rules_file" -- "$@"
  )
}

decision_for() {
  local rules_file="$1"
  shift

  local json
  json="$(execpolicy_check "$rules_file" "$@")"
  local decision
  decision="$(printf '%s' "$json" | sed -n 's/.*"decision":"\([^"]*\)".*/\1/p')"
  if [[ -z "$decision" ]]; then
    decision="no-match"
  fi
  printf '%s' "$decision"
}

run_case() {
  local rules_file="$1"
  shift
  local decision
  decision="$(decision_for "$rules_file" "$@")"
  printf '  %-10s %s\n' "$decision" "$(printf '%q ' "$@")"
}

run_matrix() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap "rm -rf '$tmp_dir'" EXIT

  local broad_rules="$tmp_dir/broad.rules"
  local narrow_rules="$tmp_dir/narrow.rules"

  cat >"$broad_rules" <<'EOF'
prefix_rule(pattern=["python3"], decision="allow")
prefix_rule(pattern=["npm"], decision="allow")
prefix_rule(pattern=["git", "push"], decision="allow")
EOF

  cat >"$narrow_rules" <<'EOF'
prefix_rule(pattern=["python3", "-V"], decision="allow")
prefix_rule(pattern=["npm", "install"], decision="allow")
prefix_rule(pattern=["git", "push", "origin", "main"], decision="allow")
EOF

  echo "=== Broad rules (unsafe convenience) ==="
  run_case "$broad_rules" python3 -V
  run_case "$broad_rules" python3 -c 'import os; print("x")'
  run_case "$broad_rules" npm install left-pad
  run_case "$broad_rules" npm publish
  run_case "$broad_rules" git push origin main
  run_case "$broad_rules" git push upstream dev
  echo

  echo "=== Narrow rules (recommended) ==="
  run_case "$narrow_rules" python3 -V
  run_case "$narrow_rules" python3 -c 'import os; print("x")'
  run_case "$narrow_rules" npm install left-pad
  run_case "$narrow_rules" npm publish
  run_case "$narrow_rules" git push origin main
  run_case "$narrow_rules" git push upstream dev
  echo

  cat <<'EOF'
Recommendation:
  Prefer task-specific prefixes (e.g. ["npm","install"], ["python3","-V"],
  ["git","push","origin","main"]) over blanket command roots.
EOF
}

audit_rules() {
  local rules_file="$1"
  if [[ ! -f "$rules_file" ]]; then
    echo "error: rules file not found: $rules_file" >&2
    exit 1
  fi

  echo "=== Audit: $rules_file ==="
  local has_warning=0

  local decision
  decision="$(decision_for "$rules_file" python3 -c 'import os; print("x")')"
  if [[ "$decision" == "allow" ]]; then
    echo "  WARN broad python allow: python3 -c is allowed"
    has_warning=1
  fi

  decision="$(decision_for "$rules_file" npm publish)"
  if [[ "$decision" == "allow" ]]; then
    echo "  WARN broad npm allow: npm publish is allowed"
    has_warning=1
  fi

  decision="$(decision_for "$rules_file" git push upstream dev)"
  if [[ "$decision" == "allow" ]]; then
    echo "  WARN broad git push allow: arbitrary remote/branch push is allowed"
    has_warning=1
  fi

  if [[ "$has_warning" -eq 0 ]]; then
    echo "  OK no broad allow matches found in default probes"
  fi
}

main() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
  fi

  if [[ "${1:-}" == "--audit" ]]; then
    if [[ $# -ne 2 ]]; then
      usage
      exit 1
    fi
    audit_rules "$2"
    exit 0
  fi

  if [[ $# -ne 0 ]]; then
    usage
    exit 1
  fi

  run_matrix
}

main "$@"
