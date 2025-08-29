#!/usr/bin/env bash
set -euo pipefail

# Simple stderr logger
header() { echo "==> $*" >&2; }

# Generate summarized release notes using Codex CLI based on PR dump.
# Can also generate just the dump via --dump-only.

usage() {
  cat <<'USAGE'
Usage: scripts/release_gen.sh [--dump-only] [-q|--quiet] [owner/repo] <from_tag> <to_tag> [version]

Examples:
  scripts/release_gen.sh openai/codex v0.23.0 v0.24.0
  scripts/release_gen.sh v0.23.0 v0.24.0                # auto-detect repo from git remote
  scripts/release_gen.sh v0.23.0 v0.24.0 0.24.0         # auto-detect with explicit version
  scripts/release_gen.sh --dump-only v0.23.0 v0.24.0    # only generate releases/release_dump_<ver>.txt
  scripts/release_gen.sh -q v0.23.0 v0.24.0             # quiet Codex call with progress dots

Notes:
  - Requires: gh and jq for dump generation; codex CLI for note generation.
  - If release_dump_<ver>.txt does not exist, it will be created automatically.
  - Then runs codex to generate <ver>.txt based on the dump (unless --dump-only).
  - If you omit tags, the script lists the last 20 releases for the repo.
USAGE
}

# Parse flags (currently: --dump-only, --quiet)
DUMP_ONLY=0
QUIET=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --dump-only)
      DUMP_ONLY=1
      ;;
    -q|--quiet)
      QUIET=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      ARGS+=("$arg")
      ;;
  esac
done
# Reset positional args safely under set -u, even if ARGS is empty
if ((${#ARGS[@]})); then
  set -- "${ARGS[@]}"
else
  set --
fi

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  usage
  exit 1
fi

# Resolve repo: allow optional first arg; otherwise detect from git remote
detect_repo() {
  local remote
  remote=$(git remote get-url origin 2>/dev/null || git remote get-url upstream 2>/dev/null || true)
  if [[ -z "$remote" ]]; then
    echo ""; return 1
  fi
  # Normalize and extract owner/repo from SSH or HTTPS/HTTP URL
  local path="$remote"
  # Strip protocols and user@
  path="${path#git@}"
  path="${path#ssh://}"
  path="${path#https://}"
  path="${path#http://}"
  path="${path#*@}"
  # If contains github.com:, take after ':'; else after 'github.com/' if present
  if [[ "$path" == *":"* ]]; then
    path="${path#*:}"
  fi
  if [[ "$path" == *github.com/* ]]; then
    path="${path#*github.com/}"
  fi
  # Trim leading slashes
  path="${path#/}"
  # Drop trailing .git
  path="${path%.git}"
  # Ensure only owner/repo
  echo "$path" | awk -F/ '{print $1"/"$2}'
}

if [[ ${1:-} == */* ]]; then
  REPO="$1"; shift
else
  REPO="$(detect_repo || true)"
  if [[ -z "$REPO" ]]; then
    echo "Error: failed to auto-detect repository from git remote. Provide [owner/repo] explicitly." >&2
    exit 1
  fi
fi

# Show a recent releases list if tags are missing
show_recent_releases_and_exit() {
  local repo="$1"
  echo "" >&2
  echo "Please pass a source/target release." >&2
  echo "" >&2
  echo "e.g.: ./scripts/release_gen.sh rust-v0.23.0 rust-v0.24.0" >&2
  echo "" >&2
  header "Recent releases for $repo:"
  echo "" >&2
  local list
  list=$(gh release list --repo "$repo" --limit 20 2>/dev/null || true)
  if [[ -z "$list" ]]; then
    echo "Error: unable to fetch releases for $repo" >&2
    exit 1
  fi
  # Print only the tag (first column) as bullets to stderr
  printf '%s\n' "$list" | awk '{print "- " $1}' >&2
  exit 1
}

if [[ $# -lt 2 ]]; then
  show_recent_releases_and_exit "$REPO"
fi

FROM_TAG="$1"
TO_TAG="$2"
VER="${3:-$TO_TAG}"
VER="${VER#v}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASES_DIR="$SCRIPT_DIR/releases"
DUMP_FILE="$RELEASES_DIR/release_dump_$VER.txt"
GEN_FILE="$RELEASES_DIR/$VER.txt"

# Ensure releases directory exists (under scripts/)
mkdir -p "$RELEASES_DIR"

abspath() {
  local p="$1"
  if command -v realpath >/dev/null 2>&1; then
    realpath "$p"
  else
    python3 -c 'import os,sys;print(os.path.abspath(sys.argv[1]))' "$p" 2>/dev/null || echo "$(pwd)/$p"
  fi
}

# ========== Dump generation logic (ported from release_dump_util.sh) ==========
header() { echo "==> $*" >&2; }

# Get an ISO 8601 datetime for a tag. Prefer release publish date; fallback to tag/commit date.
get_tag_datetime_iso() {
  local repo="$1" tag="$2"
  # Try release by tag
  local ts
  ts=$(gh release view "$tag" --repo "$repo" --json publishedAt --jq '.publishedAt' 2>/dev/null || true)
  if [[ -n "$ts" && "$ts" != "null" ]]; then
    echo "$ts"; return 0
  fi
  # Fallback: tag ref -> (annotated tag ->) commit -> date
  local ref obj_type obj_url commit_sha commit
  ref=$(gh api "repos/$repo/git/ref/tags/$tag")
  obj_type=$(jq -r '.object.type' <<<"$ref")
  obj_url=$(jq -r '.object.url' <<<"$ref")
  if [[ "$obj_type" == "tag" ]]; then
    local tag_obj
    tag_obj=$(gh api "$obj_url")
    commit_sha=$(jq -r '.object.sha' <<<"$tag_obj")
  else
    commit_sha=$(jq -r '.object.sha' <<<"$ref")
  fi
  commit=$(gh api "repos/$repo/commits/$commit_sha")
  jq -r '.commit.committer.date' <<<"$commit"
}

collect_prs_within_range() {
  local repo="$1" from_iso="$2" to_iso="$3"
  gh pr list --repo "$repo" --state merged --limit 1000 \
    --json number,title,mergedAt,author,body | \
    jq -c --arg from "$from_iso" --arg to "$to_iso" \
      '[ .[]
         | select(.mergedAt != null and .mergedAt >= $from and .mergedAt <= $to)
         | {
             number: .number,
             title: .title,
             merged_at: .mergedAt,
             author: (.author.login // "-"),
             body: (.body // "")
           }
      ] | sort_by(.merged_at) | reverse | .[]'
}

format_related_issues() {
  # shellcheck disable=SC2016
  sed 's/\r//g' | \
    grep -Eio '(close|closed|closes|fix|fixed|fixes|resolve|resolved|resolves)[[:space:]:]+([[:alnum:]_.-]+\/[[:alnum:]_.-]+)?#[0-9]+' || true | \
    grep -Eo '#[0-9]+' | tr -d '#' | sort -n -u | sed 's/^/#/' | paste -sd ', ' -
}

generate_dump() {
  local repo="$1" from_tag="$2" to_tag="$3" out_file="$4"
  command -v gh >/dev/null 2>&1 || { echo "Error: gh (GitHub CLI) is required" >&2; exit 1; }
  command -v jq >/dev/null 2>&1 || { echo "Error: jq is required" >&2; exit 1; }

  header "Resolving tag dates ($from_tag -> $to_tag)"
  local from_iso to_iso
  from_iso=$(get_tag_datetime_iso "$repo" "$from_tag")
  to_iso=$(get_tag_datetime_iso "$repo" "$to_tag")
  if [[ -z "$from_iso" || -z "$to_iso" ]]; then
    echo "Error: failed to resolve tag dates. from=$from_tag ($from_iso) to=$to_tag ($to_iso)" >&2
    exit 1
  fi

  header "Collecting merged PRs via gh pr list"
  local tmpdir sorted
  tmpdir=$(mktemp -d)
  sorted="$tmpdir/prs.sorted.ndjson"
  collect_prs_within_range "$repo" "$from_iso" "$to_iso" > "$sorted"

  local count
  count=$(wc -l < "$sorted" | tr -d ' ')

  header "Writing $out_file (Total PRs: $count)"
  {
    echo "Repository: $repo"
    echo "Range: $from_tag ($from_iso) -> $to_tag ($to_iso)"
    echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Total PRs: $count"
    echo ""
  } > "$out_file"

  if [[ "$count" -eq 0 ]]; then
    return 0
  fi

  while IFS= read -r line; do
    local title number merged_at author body issues
    title=$(jq -r '.title' <<<"$line")
    number=$(jq -r '.number' <<<"$line")
    merged_at=$(jq -r '.merged_at' <<<"$line")
    author=$(jq -r '.author' <<<"$line")
    body=$(jq -r '.body' <<<"$line")

    issues=$(printf '%s' "$body" | format_related_issues || true)
    [[ -z "$issues" ]] && issues="-"

    {
      echo "PR #$number: $title"
      echo "Merged: $merged_at | Author: $author"
      echo "Related issues: $issues"
      echo ""
      # Skip verbose descriptions for Dependabot PRs
      if [[ "$author" != "app/dependabot" && "$author" != "dependabot[bot]" && ! "$author" =~ [Dd]ependabot ]]; then
        echo "Description:"
        # Limit descriptions to 2000 characters; add ellipses if truncated
        local max=2000
        if (( ${#body} > max )); then
          printf '%s\n' "${body:0:max}..."
        else
          printf '%s\n' "$body"
        fi
        echo ""
      fi
      echo "-----"
      echo ""
    } >> "$out_file"
  done < "$sorted"

  header "Done -> $out_file"
}

# ========== Orchestrate dump + optional codex generation ==========

# Create dump if missing
if [[ ! -f "$DUMP_FILE" ]]; then
  header "Dump not found: $DUMP_FILE. Generating..."
  generate_dump "$REPO" "$FROM_TAG" "$TO_TAG" "$DUMP_FILE"
else
  header "Using existing dump: $DUMP_FILE"
fi

if (( DUMP_ONLY )); then
  # Dump-only mode: no stdout output
  exit 0
fi

# Now run codex to generate notes
command -v codex >/dev/null 2>&1 || { echo "Error: codex CLI is required for generation. Use --dump-only to skip." >&2; exit 1; }

DUMP_PATH="$(abspath "$DUMP_FILE")"
PROMPT="`cat ${DUMP_PATH}`\n\n---\n\nPlease generate a summarized release note based on the list of PRs above. Then, write a file called $GEN_FILE with your suggested release notes. It should follow this structure (+ the style/tone/brevity in this example):\n\n\"## Highlights:\n\n### New Features\n- Queued messages (#2637)\n- Copy Paste / Drag & Drop image files (#2567)\n- Add web_search tool (#2371)\n- Add transcript mode (Ctrl+T) with scrolling ability (#2525)\n- Edit/resume conversation (esc-esc) from previous messages (#2607)\n\n### TUI\n- Hide CoT by default; show headers in status indicator (#2316)\n- Show diff output in pager (+ with hunk headers) (#2568)\n- Simplify command approval UI (#2708)\n- Unify Esc/Ctrl+C interrupt handling (#2661)\n- Fix windows powershell paste (#2544)\n\n### Tools and execution\n- Add support for long-running shell commands \`exec_command\`/\`write_stdin\` (#2574)\n- Improve apply_patch reliability (#2646)\n- Cap retry counts (#2701)\n- Sort MCP tools deterministically (#2611)\n\n### Misc\n- Add model_verbosity config for GPT-5 (#2108)\n- Read all AGENTS.md files up to git root (#2532)\n- Fix git root resolution in worktrees (#2585)\n- Improve error messages & handling (#2695, #2587, #2640, #2540)\n\n\n## Full list of merged PRs:\n\n - #2708 [feat] Simplfy command approval UI\n - #2706 [chore] Tweak...\""

header "Calling codex to generate $GEN_FILE"
if (( QUIET )); then
  # Quiet mode: run Codex silently and show progress dots
  (
    set +x 2>/dev/null || true
    codex exec --sandbox workspace-write "$PROMPT" >/dev/null 2>&1
  ) &
  CODEX_PID=$!
  while :; do
    kill -0 "$CODEX_PID" 2>/dev/null || break
    printf "." >&2
    sleep 1
  done
  wait "$CODEX_PID" || true
  CODEX_STATUS=$?
  echo "" >&2
else
  # Normal mode: stream Codex output to stderr as before
  codex exec --sandbox workspace-write "$PROMPT" 1>&2
fi

if [[ -f "$GEN_FILE" ]]; then
  # On success, output only the generated release notes to stdout
  cat "$GEN_FILE"
else
  echo "Warning: $GEN_FILE not created. Check codex output." >&2
  exit 1
fi
