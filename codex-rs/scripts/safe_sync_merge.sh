#!/usr/bin/env bash
set -euo pipefail

# Safe sync & merge without history rewrites.
#
# Path resolution note: CI and local tooling should resolve this script
# via the shared helper `codex-rs/scripts/resolve_safe_sync.sh`, which
# prefers `codex-rs/scripts` over root `scripts` to avoid drift.
#
# - Fetches remote updates
# - Creates a safety backup branch from your current HEAD
# - Stashes uncommitted changes (optional) and restores them after merge
# - Merges upstream into the current branch using a merge commit (no rebase)
# - Optionally launches Codex TUI on conflicts to assist resolution
# - Optionally runs checks (fmt/clippy/tests) after a successful merge
#
# Usage:
#   scripts/safe_sync_merge.sh [--remote origin] [--upstream <branch>]
#                              [--no-stash] [--no-checks] [--no-tui]
#                              [--dry-run]
#
# Defaults:
#   --remote origin
#   --upstream: tracked upstream (@{u}) if set; otherwise origin/<current>,
#               else origin/main, else origin/master

REMOTE="origin"
UPSTREAM=""
DO_STASH=1
RUN_CHECKS=1
TUI_ON_CONFLICT=1
DRY_RUN=0

log() { printf "[safe-sync] %s\n" "$*"; }
err() { printf "[safe-sync][error] %s\n" "$*" 1>&2; }

usage() {
  sed -n '1,60p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --remote) REMOTE="$2"; shift 2 ;;
    --upstream) UPSTREAM="$2"; shift 2 ;;
    --no-stash) DO_STASH=0; shift ;;
    --no-checks) RUN_CHECKS=0; shift ;;
    --no-tui) TUI_ON_CONFLICT=0; shift ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) err "Unknown arg: $1"; usage; exit 1 ;;
  esac
done

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { err "Missing required command: $1"; exit 1; }
}

require_cmd git

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  err "Not inside a git repository"
  exit 1
fi

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
CURRENT_SHA=$(git rev-parse --short=12 HEAD)
TS=$(date +%Y%m%d-%H%M%S)
BACKUP_BRANCH="backup/sync-${TS}-${CURRENT_BRANCH}-${CURRENT_SHA}"

REPO_ROOT=""
if REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null); then
  :
else
  err "Unable to determine repository root. Are you inside a git repo?"
  exit 1
fi

if [[ "${PWD}" != "${REPO_ROOT}" ]]; then
  log "Detected repo root: ${REPO_ROOT}"
  log "Changing directory to repo root for consistent behavior"
  cd "${REPO_ROOT}" || { err "Failed to cd to repo root: ${REPO_ROOT}"; exit 1; }
fi

log "Repository OK. Root: ${REPO_ROOT}. Current branch: ${CURRENT_BRANCH} (${CURRENT_SHA})"

# Resolve upstream if not explicitly provided.
if [[ -z "${UPSTREAM}" ]]; then
  if git rev-parse --abbrev-ref --symbolic-full-name @{u} >/dev/null 2>&1; then
    UPSTREAM="@{u}"
  else
    # Prefer remote branch with same name, then main, then master.
    if git ls-remote --heads "${REMOTE}" "${CURRENT_BRANCH}" | grep -q "refs/heads/${CURRENT_BRANCH}$"; then
      UPSTREAM="${REMOTE}/${CURRENT_BRANCH}"
    elif git ls-remote --heads "${REMOTE}" main | grep -q "refs/heads/main$"; then
      UPSTREAM="${REMOTE}/main"
    elif git ls-remote --heads "${REMOTE}" master | grep -q "refs/heads/master$"; then
      UPSTREAM="${REMOTE}/master"
    else
      err "Could not determine an upstream branch. Set one with --upstream <branch> or configure a tracking branch."
      exit 1
    fi
  fi
fi

log "Upstream to merge: ${UPSTREAM}"

if [[ ${DRY_RUN} -eq 1 ]]; then
  log "Dry run enabled: no changes will be made."
fi

run() {
  if [[ ${DRY_RUN} -eq 1 ]]; then
    printf "[dry-run] %s\n" "$*"
  else
    eval "$@"
  fi
}

# Check in-progress operations that could interfere.
if [[ -d .git/rebase-apply || -d .git/rebase-merge ]]; then
  err "Rebase in progress. Abort or finish it before running this script."
  exit 1
fi
if [[ -f .git/MERGE_HEAD ]]; then
  err "Merge in progress. Resolve or abort it before running this script."
  exit 1
fi

# Create safety backup branch
log "Creating safety backup branch: ${BACKUP_BRANCH}"
run git branch "${BACKUP_BRANCH}"

# Optionally stash any uncommitted changes (tracked and untracked)
STASH_REF=""
if [[ ${DO_STASH} -eq 1 ]]; then
  if ! git diff --quiet || ! git diff --cached --quiet || [[ -n "$(git ls-files --others --exclude-standard)" ]]; then
    log "Stashing local changes before merge"
    if [[ ${DRY_RUN} -eq 1 ]]; then
      printf "[dry-run] git stash push -u -m safe-sync:%s\n" "${TS}"
    else
      STASH_REF=$(git stash push -u -m "safe-sync:${TS}") || true
    fi
  else
    log "Working tree clean; no stash needed"
  fi
else
  log "--no-stash specified; proceeding with working tree as-is"
fi

# Fetch remotes
log "Fetching from all remotes (with prune)"
run git fetch --all --prune

# Merge upstream into current branch using a merge commit
log "Merging ${UPSTREAM} into ${CURRENT_BRANCH} (no rebase, no ff)"
set +e
if [[ ${DRY_RUN} -eq 1 ]]; then
  printf "[dry-run] git merge --no-ff --no-edit %q\n" "${UPSTREAM}"
  MERGE_STATUS=0
else
  git merge --no-ff --no-edit "${UPSTREAM}"
  MERGE_STATUS=$?
fi
set -e

if [[ ${MERGE_STATUS} -ne 0 ]]; then
  err "Merge reported conflicts."
  if [[ -n "${STASH_REF}" ]]; then
    err "A stash was created before the merge: ${STASH_REF}"
  fi
  if [[ ${TUI_ON_CONFLICT} -eq 1 ]]; then
    if command -v cargo >/dev/null 2>&1; then
      log "Launching Codex TUI to assist with conflict resolution... (Ctrl+C to stop)"
      if [[ ${DRY_RUN} -eq 0 ]]; then
        # Best-effort: launch TUI in reviewer mode if available; fall back to default
        cargo run --bin codex -- tui || true
      else
        printf "[dry-run] cargo run --bin codex -- tui\n"
      fi
    else
      err "Cargo not found; please resolve conflicts manually or run the TUI via 'just tui' if available."
    fi
  else
    log "--no-tui specified; resolve conflicts manually."
  fi
  err "After resolving conflicts, complete the merge and re-run this script to verify."
  exit 2
fi

log "Merge completed successfully."

# Restore stash after successful merge (if any)
if [[ -n "${STASH_REF}" ]]; then
  log "Restoring stashed changes: ${STASH_REF}"
  if [[ ${DRY_RUN} -eq 1 ]]; then
    printf "[dry-run] git stash pop\n"
  else
    set +e
    git stash pop
    POP_STATUS=$?
    set -e
    if [[ ${POP_STATUS} -ne 0 ]]; then
      err "Stash pop reported conflicts. Resolve them and continue."
      if [[ ${TUI_ON_CONFLICT} -eq 1 && $(command -v cargo >/dev/null 2>&1; echo $?) -eq 0 ]]; then
        log "Launching Codex TUI to assist..."
        cargo run --bin codex -- tui || true
      fi
      exit 3
    fi
  fi
fi

if [[ ${RUN_CHECKS} -eq 1 ]]; then
  if command -v cargo >/dev/null 2>&1; then
    # Determine where to run cargo from. Prefer repo root if it has Cargo.toml,
    # else fall back to a common workspace path like "codex-rs" if present.
    HAS_ROOT_CARGO=0
    HAS_CODEX_RS_CARGO=0
    CARGO_DIR="."
    if [[ -f Cargo.toml ]]; then
      HAS_ROOT_CARGO=1
      CARGO_DIR="."
    elif [[ -f codex-rs/Cargo.toml ]]; then
      HAS_CODEX_RS_CARGO=1
      CARGO_DIR="codex-rs"
    fi

    if [[ ${HAS_ROOT_CARGO} -eq 0 && ${HAS_CODEX_RS_CARGO} -eq 0 ]]; then
      export SAFE_SYNC_NO_WORKSPACE=1
      log "SKIP_WORKSPACE"
      log "No Rust workspace detected (no Cargo.toml or codex-rs/Cargo.toml). Skipping Rust checks."
      # Nothing to do; keep exit code 0. CI can detect SKIP via SAFE_SYNC_NO_WORKSPACE env marker.
    else

      log "Running post-merge checks in '${CARGO_DIR}' (fmt/clippy/tests)"
      if [[ ${DRY_RUN} -eq 1 ]]; then
        printf "[dry-run] cargo -C %q fmt -- --check\n" "${CARGO_DIR}"
        printf "[dry-run] cargo -C %q clippy --workspace -- -D warnings\n" "${CARGO_DIR}"
        printf "[dry-run] cargo -C %q test --workspace\n" "${CARGO_DIR}"
      else
        cargo -C "${CARGO_DIR}" fmt -- --check || { err "Formatting check failed"; exit 4; }
        cargo -C "${CARGO_DIR}" clippy --workspace -- -D warnings || { err "Clippy failed"; exit 5; }
        cargo -C "${CARGO_DIR}" test --workspace || { err "Tests failed"; exit 6; }
      fi
    fi
  else
    log "Cargo not found; skipping Rust checks. If conflicts occurred, you can run 'just tui' or resolve manually."
  fi
else
  log "--no-checks specified; skipping post-merge checks"
fi

log "Done. A safety backup was created at: ${BACKUP_BRANCH}"
log "Tip: if anything went wrong, you can inspect or reset to that branch."
