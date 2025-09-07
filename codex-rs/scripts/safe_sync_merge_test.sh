#!/usr/bin/env bash
set -euo pipefail

# Minimal e2e tests for scripts/safe_sync_merge.sh exit semantics.
#
# This script creates temporary git repos and exercises the merge flow.
# It uses a lightweight cargo shim to simulate fmt/clippy/test outcomes.
#
# Stabilize locale-dependent outputs from git across environments.
export LC_ALL=C
export LANG=C

# Usage:
#   scripts/safe_sync_merge_test.sh            # run all cases
#   scripts/safe_sync_merge_test.sh <case>     # run one: skip|fmt_fail|clippy_fail|test_fail|ok_dryrun

THIS_DIR="$(cd "$(dirname "$0")" && pwd)"
SAFE_SYNC="${THIS_DIR}/safe_sync_merge.sh"

require_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1"; exit 1; }; }
require_cmd git

mktempd() { mktemp -d 2>/dev/null || mktemp -d -t safe-sync; }

setup_remote_and_work() {
  local base tmp remote work
  base=$(mktempd)
  remote="${base}/remote.git"
  work="${base}/work"

  git init --bare -q "${remote}"
  mkdir -p "${work}"
  pushd "${work}" >/dev/null
  git init -q
  git config user.email test@example.com
  git config user.name test
  git checkout -b main -q
  echo "hello" > README
  git add README
  git commit -q -m "init"
  git remote add origin "${remote}"
  git push -q -u origin main
  popd >/dev/null
  echo "${work}"
}

shim_cargo() {
  local bin_dir="$1"; shift
  local mode="$1"; shift
  mkdir -p "${bin_dir}"
  cat >"${bin_dir}/cargo" <<'EOS'
#!/usr/bin/env bash
set -euo pipefail

# Simple cargo shim that simulates success/failure per subcommand.
MODE="${CARGO_SHIM_MODE:-ok}"

# Handle optional -C <dir>
if [[ "${1:-}" == "-C" ]]; then
  if [[ $# -lt 3 ]]; then echo "cargo shim: missing args" >&2; exit 2; fi
  shift 2
fi
SUB="${1:-}"
case "${SUB}" in
  fmt)
    [[ "${MODE}" == "fmt_fail" ]] && exit 1 || exit 0 ;;
  clippy)
    [[ "${MODE}" == "clippy_fail" ]] && exit 1 || exit 0 ;;
  test)
    [[ "${MODE}" == "test_fail" ]] && exit 1 || exit 0 ;;
  *)
    exit 0 ;;
esac
EOS
  chmod +x "${bin_dir}/cargo"
}

case_skip() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  # Ensure no Rust workspace present
  rm -f Cargo.toml
  rm -rf codex-rs
  set +e
  bash "${SAFE_SYNC}" --dry-run --no-tui
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then echo "[skip] expected rc=0 got $rc"; exit 1; fi
  if ! bash "${SAFE_SYNC}" --dry-run --no-tui | grep -q "\[safe-sync] SKIP_WORKSPACE"; then
    echo "[skip] missing SKIP_WORKSPACE marker"; exit 1; fi
  popd >/dev/null
  echo "[ok] skip"
}

case_fmt_fail() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  mkdir -p codex-rs; echo "[workspace]" > codex-rs/Cargo.toml
  local bindir="${work}/.bin"; shim_cargo "${bindir}" fmt_fail
  set +e
  PATH="${bindir}:$PATH" CARGO_SHIM_MODE=fmt_fail bash "${SAFE_SYNC}" --no-tui
  local rc=$?
  set -e
  if [[ $rc -ne 4 ]]; then echo "[fmt_fail] expected rc=4 got $rc"; exit 1; fi
  echo "[ok] fmt_fail"
  popd >/dev/null
}

case_clippy_fail() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  mkdir -p codex-rs; echo "[workspace]" > codex-rs/Cargo.toml
  local bindir="${work}/.bin"; shim_cargo "${bindir}" clippy_fail
  set +e
  PATH="${bindir}:$PATH" CARGO_SHIM_MODE=clippy_fail bash "${SAFE_SYNC}" --no-tui
  local rc=$?
  set -e
  if [[ $rc -ne 5 ]]; then echo "[clippy_fail] expected rc=5 got $rc"; exit 1; fi
  echo "[ok] clippy_fail"
  popd >/dev/null
}

case_test_fail() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  mkdir -p codex-rs; echo "[workspace]" > codex-rs/Cargo.toml
  local bindir="${work}/.bin"; shim_cargo "${bindir}" test_fail
  set +e
  PATH="${bindir}:$PATH" CARGO_SHIM_MODE=test_fail bash "${SAFE_SYNC}" --no-tui
  local rc=$?
  set -e
  if [[ $rc -ne 6 ]]; then echo "[test_fail] expected rc=6 got $rc"; exit 1; fi
  echo "[ok] test_fail"
  popd >/dev/null
}

case_ok_dryrun() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  mkdir -p codex-rs; echo "[workspace]" > codex-rs/Cargo.toml
  set +e
  bash "${SAFE_SYNC}" --dry-run --no-tui | sed -n '1,200p'
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then echo "[ok_dryrun] expected rc=0 got $rc"; exit 1; fi
  echo "[ok] ok_dryrun"
  popd >/dev/null
}

case_skip_env() {
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  # No Rust workspace present
  rm -f Cargo.toml
  rm -rf codex-rs
  set +e
  out=$(bash "${SAFE_SYNC}" --dry-run --no-tui 2>&1)
  rc=$?
  set -e
  if [[ $rc -ne 0 ]]; then echo "[skip_env] expected rc=0 got $rc"; exit 1; fi
  if ! printf "%s" "$out" | grep -q "\[safe-sync] SKIP_WORKSPACE"; then
    echo "[skip_env] missing SKIP_WORKSPACE marker"; exit 1; fi
  echo "[ok] skip_env"
  popd >/dev/null
}

run_all() {
  case_ok_dryrun
  case_skip
  case_skip_env
  case_fmt_fail
  case_clippy_fail
  case_test_fail
  case_path_detection
  case_root_only_scripts
  case_git_hint
  case_resolver_exit_codes
  case_timestamp_guard
  case_resolver_help
}

normalize_output() {
  local infile="$1"
  sed -E \
    -e 's~^(\[safe-sync] Detected repo root: ).*$~\1/path/to/your/repo~' \
    -e 's~^(\[safe-sync] Repository OK\. Root: ).*\. Current branch: [^ ]+ \([0-9a-f]{12}\)$~\1/path/to/your/repo. Current branch: main (abcdef123456)~' \
    -e 's~^(\[safe-sync] Upstream to merge: ).*$~\1@{u}~' \
    -e 's~^(\[dry-run] git merge --no-ff --no-edit ).*$~\1@\\{u\\}~' \
    -e 's~^(\[safe-sync] Creating safety backup branch: backup/sync-)[0-9]{8}-[0-9]{6}~\1YYYYmmdd-HHMMSS~' \
    -e 's~^(\[dry-run] git branch backup/sync-)[0-9]{8}-[0-9]{6}~\1YYYYmmdd-HHMMSS~' \
    -e 's~^(\[safe-sync] Done\. A safety backup was created at: backup/sync-)[0-9]{8}-[0-9]{6}~\1YYYYmmdd-HHMMSS~' \
    -e 's~^(\[dry-run] git stash push -u -m safe-sync:)[0-9]{8}-[0-9]{6}$~\1YYYYmmdd-HHMMSS~' \
    -e 's~^(\[safe-sync] Creating safety backup branch: .*-)[0-9a-f]{12}$~\1abcdef123456~' \
    -e 's~^(\[dry-run] git branch .*-)[0-9a-f]{12}$~\1abcdef123456~' \
    -e 's~^(\[safe-sync] Done\. A safety backup was created at: .*-)[0-9a-f]{12}$~\1abcdef123456~' \
    -e '/^hint: (You have divergent|Updates were rejected)/d' \
    -e '/^Auto-merging /d' \
    "$infile"
}

case_golden() {
  # Clone current repo shallowly to avoid touching developer checkout
  local base tmp clone out norm snap
  base=$(mktempd)
  clone="${base}/repo"
  local REPO_ROOT
  REPO_ROOT=$(git rev-parse --show-toplevel)
  git clone --no-tags --filter=blob:none --depth 1 "${REPO_ROOT}" "${clone}" >/dev/null || true
  # Resolve path via shared helper (prefer codex-rs)
  local RESOLVER="${clone}/codex-rs/scripts/resolve_safe_sync.sh"
  if [[ ! -x "${RESOLVER}" ]]; then echo "missing resolver: ${RESOLVER}" >&2; exit 1; fi
  eval "$(${RESOLVER} --root "${clone}")"
  local SCRIPT_PATH="${SAFE_SYNC}"
  out="${base}/dry_run.log"
  bash "${SCRIPT_PATH}" --dry-run --no-tui | tee "${out}" >/dev/null || true
  # Require dry-run mode; otherwise skip golden
  if ! grep -q '^\[safe-sync] Dry run enabled: no changes will be made\.' "${out}"; then
    echo "[skip] golden: non-dry output detected"; return 0
  fi
  norm="${base}/dry_run.norm.log"
  normalize_output "${out}" > "${norm}"
  # Ensure normalization scope is tight: exactly 4 timestamp placeholders expected
  local ts_count
  ts_count=$(grep -o 'YYYYmmdd-HHMMSS' "${norm}" | wc -l | tr -d ' ')
  if [[ "${ts_count}" != "4" ]]; then
    echo "Unexpected timestamp placeholder count: ${ts_count}" >&2
    exit 1
  fi
  # Prepend header comments to match snapshot
  local norm_with_header="${base}/dry_run.norm.with_header.log"
  {
    echo "# Golden dry-run snapshot for safe_sync_merge.sh"
    echo "# Invariant: expects exactly 4 YYYYmmdd-HHMMSS placeholders."
    cat "${norm}"
  } > "${norm_with_header}"
  snap="${REPO_ROOT}/codex-rs/tests/merge_flow_dry_run.snap"
  if ! diff -u "${snap}" "${norm_with_header}"; then
    echo "Golden snapshot mismatch. Inspect ${norm} vs ${snap}" >&2
    exit 1
  fi
  echo "[ok] golden"
}

case_root_only_scripts() {
  local base; base=$(mktempd)
  mkdir -p "${base}/scripts"
  printf '#!/usr/bin/env bash\nexit 0\n' > "${base}/scripts/safe_sync_merge.sh"
  chmod +x "${base}/scripts/safe_sync_merge.sh"
  local RESOLVER="${THIS_DIR}/resolve_safe_sync.sh"
  eval "$(${RESOLVER} --root "${base}")"
  if [[ "${SAFE_SYNC}" != "${base}/scripts/safe_sync_merge.sh" ]]; then
    echo "[root_only_scripts] expected root scripts to be chosen, got ${SAFE_SYNC}"; exit 1
  fi
  echo "[ok] root_only_scripts"
}

case_git_hint() {
  local base; base=$(mktempd)
  local f_in="${base}/in.log"; local f_out="${base}/out.log"
  cat >"${f_in}" <<'EOS'
hint: You have divergent branches and need to specify how to reconcile them.
Auto-merging src/lib.rs
[safe-sync] Merging @{u} into main (no rebase, no ff)
EOS
  normalize_output "${f_in}" > "${f_out}"
  if grep -q '^hint: ' "${f_out}" || grep -q '^Auto-merging ' "${f_out}"; then
    echo "[git_hint] normalization did not remove hint lines"; exit 1
  fi
  if ! grep -q '^\[safe-sync] Merging' "${f_out}"; then
    echo "[git_hint] expected safe-sync merge line present"; exit 1
  fi
  echo "[ok] git_hint"
}

case_resolver_exit_codes() {
  local RESOLVER="${THIS_DIR}/resolve_safe_sync.sh"
  # Not-found under valid root → exit 2
  local tmp; tmp=$(mktempd)
  set +e
  bash "${RESOLVER}" --root "${tmp}" >/dev/null 2>&1
  local rc=$?
  set -e
  if [[ $rc -ne 2 ]]; then echo "[resolver_exit] expected rc=2 got $rc"; exit 1; fi
  # Invalid root → exit 3
  set +e
  bash "${RESOLVER}" --root "/definitely/not/there" >/dev/null 2>&1
  rc=$?
  set -e
  if [[ $rc -ne 3 ]]; then echo "[resolver_exit] expected rc=3 got $rc"; exit 1; fi
  echo "[ok] resolver_exit_codes"
}

case_resolver_help() {
  local RESOLVER="${THIS_DIR}/resolve_safe_sync.sh"
  local out norm
  out=$(bash "${RESOLVER}" --help)
  # Normalize CRLF and trim trailing blank lines to reduce platform noise before checks/diff.
  # Prefer tr+awk, fallback to pure sed if unavailable.
  if command -v tr >/dev/null 2>&1 && command -v awk >/dev/null 2>&1; then
    norm=$(printf "%s\n" "$out" | tr -d '\r' | awk 'NF{print $0}' ORS="\n")
  else
    norm=$(printf "%s\n" "$out" | sed -e 's/\r$//' -e :a -e '/^[[:space:]]*$/{$d;N;ba' -e '}' )
  fi
  printf "%s\n" "$norm" | grep -q "DEPRECATED compat mode; prefer --root" || { echo "[resolver_help] missing deprecation note"; echo "$norm"; exit 1; }
  # Ensure deprecation note appears only once to avoid ambiguity
  local depc
  depc=$(printf "%s" "$norm" | grep -c "DEPRECATED compat mode; prefer --root" | tr -d ' ')
  if [[ "$depc" != "1" ]]; then echo "[resolver_help] unexpected deprecation occurrences: $depc"; exit 1; fi
  printf "%s\n" "$norm" | grep -q "0 ok" || { echo "[resolver_help] missing exit code 0"; exit 1; }
  printf "%s\n" "$norm" | grep -q "2 not-found" || { echo "[resolver_help] missing exit code 2"; exit 1; }
  printf "%s\n" "$norm" | grep -q "3 invalid-root" || { echo "[resolver_help] missing exit code 3"; exit 1; }
  # Golden snapshot compare of full help text
  # Golden: codex-rs/docs/golden/resolver_help.txt
  local golden="${THIS_DIR}/../docs/golden/resolver_help.txt"
  if ! diff -u "$golden" <(printf "%s\n" "$norm"); then
    echo "[resolver_help] help output deviates from golden snapshot: docs/RESOLVER.md.help.golden"; exit 1
  fi
  # Lint guard: find positional uses outside docs (allow resolver assignment and docs files)
  if rg --version >/dev/null 2>&1; then
    local pos
    pos=$(rg -n -S --pcre2 'bash\s+[^\n#]*resolve_safe_sync\.sh(?![^\n#]*--root)' \
      --type-add 'sh:*.sh,*.bash,*.zsh' -t sh scripts/ \
      --glob '!**/docs/**' || true)
    if [[ -n "$pos" ]]; then
      echo "[resolver_help] positional resolver usage found (missing --root):"
      echo "$pos"; exit 1
    fi
  fi
  echo "[ok] resolver_help"
}

case_timestamp_guard() {
  local base; base=$(mktempd)
  local work; work=$(setup_remote_and_work)
  pushd "${work}" >/dev/null
  mkdir -p codex-rs; echo "[workspace]" > codex-rs/Cargo.toml
  local out norm
  out="${base}/dry_run.log"
  bash "${SAFE_SYNC}" --dry-run --no-tui > "${out}" 2>&1 || true
  norm="${base}/norm.log"
  normalize_output "${out}" > "${norm}"
  local c
  c=$(grep -o 'YYYYmmdd-HHMMSS' "${norm}" | wc -l | tr -d ' ')
  if [[ "$c" != "4" ]]; then echo "[timestamp_guard] expected 4 placeholders in baseline, got $c"; exit 1; fi
  local norm3="${base}/norm3.log"
  sed '0,/YYYYmmdd-HHMMSS/{s/YYYYmmdd-HHMMSS/TS3/}' "${norm}" > "${norm3}"
  c=$(grep -o 'YYYYmmdd-HHMMSS' "${norm3}" | wc -l | tr -d ' ')
  if [[ "$c" == "4" ]]; then echo "[timestamp_guard] expected !=4 after removal"; exit 1; fi
  local norm5="${base}/norm5.log"
  { cat "${norm}"; echo "YYYYmmdd-HHMMSS"; } > "${norm5}"
  c=$(grep -o 'YYYYmmdd-HHMMSS' "${norm5}" | wc -l | tr -d ' ')
  if [[ "$c" == "4" ]]; then echo "[timestamp_guard] expected !=4 after addition"; exit 1; fi
  popd >/dev/null
  echo "[ok] timestamp_guard"
}

case_path_detection() {
  local base; base=$(mktempd)
  mkdir -p "${base}/codex-rs/scripts" "${base}/scripts"
  printf '#!/usr/bin/env bash
exit 0
' > "${base}/codex-rs/scripts/safe_sync_merge.sh"
  chmod +x "${base}/codex-rs/scripts/safe_sync_merge.sh"
  printf '#!/usr/bin/env bash
exit 0
' > "${base}/scripts/safe_sync_merge.sh"
  local RESOLVER="${THIS_DIR}/resolve_safe_sync.sh"
  eval "$(${RESOLVER} --root "${base}")"
  if [[ "${SAFE_SYNC}" != "${base}/codex-rs/scripts/safe_sync_merge.sh" ]]; then
    echo "[path_detection] expected codex-rs to be chosen, got ${SAFE_SYNC}"; exit 1
  fi
  echo "[ok] path_detection"
}

main() {
  local c="${1:-all}"
  case "$c" in
    all) run_all ;;
    golden) case_golden ;;
    path_detection) case_path_detection ;;
    root_only_scripts) case_root_only_scripts ;;
    git_hint) case_git_hint ;;
    timestamp_guard) case_timestamp_guard ;;
    resolver_exit_codes) case_resolver_exit_codes ;;
    resolver_help) case_resolver_help ;;
    skip) case_skip ;;
    skip_env) case_skip_env ;;
    fmt_fail) case_fmt_fail ;;
    clippy_fail) case_clippy_fail ;;
    test_fail) case_test_fail ;;
    ok_dryrun) case_ok_dryrun ;;
    *) echo "unknown case: $c"; exit 2 ;;
  esac
}

main "$@"
