#!/usr/bin/env bash
set -euo pipefail

# A helper to quickly sync your fork with the upstream repository.
#
# Use cases:
#   1) Merge upstream/main into your branch or main
#   2) Merge a specific upstream tag (e.g., rust-v0.21.0)
#   3) Optionally create a temporary branch and push it for a PR
#   4) Optionally tag the baseline (e.g., base-rust-v0.21.0) for traceability
#
# Defaults:
#   - Upstream remote URL: https://github.com/openai/codex.git (override with --upstream-url)
#   - Target branch: main (override with --branch)
#   - Strategy: merge (use --rebase to rebase instead)
#
# Examples:
#   scripts/sync_upstream.sh merge-main --branch main --push
#   scripts/sync_upstream.sh merge-tag rust-v0.21.0 --branch main --push
#   scripts/sync_upstream.sh list-tags --limit 10

UPSTREAM_URL_DEFAULT="https://github.com/openai/codex.git"
DRY_RUN="false"
FORCE_TAGS="false"
INCLUDE_PRE="false"
PRE_ONLY="false"
FETCH_ALL_TAGS="false"

usage() {
  cat <<EOF
Usage: $(basename "$0") <command> [options]

Commands
  merge-main                Merge (or rebase) upstream/main into a branch
  merge-tag <tag>           Merge (or rebase) a specific upstream tag into a branch
  merge-series              Merge all upstream rust-v* tags after the current baseline up to latest
  init-baseline <tag>       Create a baseline tag (base-<tag>) at current HEAD without merging
  list-tags                 List upstream rust-v* tags
  current-baseline          Show the last known upstream baseline for the current HEAD

Global options
  --branch <name>           Target branch to update (default: main)
  --upstream-url <url>      Upstream repo URL (default: ${UPSTREAM_URL_DEFAULT})
  --rebase                  Use rebase instead of merge
  --no-branch               Do not create a temp branch; operate directly on --branch
  --push                    Push the resulting branch to origin
  --tag-baseline            After merging an upstream tag, create a baseline tag (base-<tag>) on the result
  --baseline-prefix <pfx>   Prefix for baseline tags (default: base-)
  --push-tags               Push created baseline tags to origin
  --from <tag>              Baseline rust-v* tag to start from (used by merge-series)
  --to <tag>                Stop at this rust-v* tag (inclusive) (used by merge-series)
  --limit <N>               Limit number of tags to merge in merge-series (default: 20)
  --limit <N>               Limit for list-tags
  --dry-run                 Preview actions without changing the repo or network calls
  --force-tags              Force-update local tags when fetching (may overwrite diverged local tags)
  --include-pre             Include pre-release tags (-alpha/-beta/-rc) when selecting tags
  --pre-only                Only include pre-release tags (overrides --include-pre)
  --fetch-all-tags          Fetch all tags (not only rust-v*); may trigger unrelated tag conflicts
  -h, --help                Show help

Notes
  - Requires a clean working tree (no uncommitted changes)
  - Will ensure a remote named 'upstream' exists and fetch tags
EOF
}

ensure_clean() {
  if [[ "$DRY_RUN" == "true" ]]; then
    echo "[dry-run] Skipping clean working tree check"
    return 0
  fi
  if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
    echo "ERROR: You have uncommitted or untracked changes." >&2
    exit 1
  fi
}

ensure_upstream_remote() {
  local url="$1"
  if git remote get-url upstream >/dev/null 2>&1; then
    :
  else
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] Would add upstream remote: $url"
    else
      echo "Adding upstream remote: $url"
      git remote add upstream "$url"
    fi
  fi
  if [[ "$DRY_RUN" == "true" ]]; then
    echo "[dry-run] Would fetch upstream branches (--prune) and rust-v* tags${FORCE_TAGS:+ (force)}${FETCH_ALL_TAGS:+ (plus all tags)}"
    # Perform a lightweight connectivity probe so users get early feedback in dry-run
    local TIMEOUT_BIN=""
    if command -v timeout >/dev/null 2>&1; then TIMEOUT_BIN="timeout 5"; fi
    if ! GIT_TERMINAL_PROMPT=0 $TIMEOUT_BIN git ls-remote --heads --tags "$url" >/dev/null 2>&1; then
      echo "[dry-run] 提示：网络可能受限或无法访问 upstream，将仅使用本地 tag 视图。"
    fi
  else
    # 1) Fetch branches
    local fetch_output
    if ! fetch_output=$(git fetch upstream --prune 2>&1); then
      echo "$fetch_output"
      echo "⚠️  分支抓取失败或部分失败（可能是网络受限或权限问题）。将继续使用本地视图。"
    fi
    # 2) Fetch rust-v* tags (stable + pre-release patterns) to avoid unrelated tag conflicts
    local tag_refspec='refs/tags/rust-v*:refs/tags/rust-v*'
    local fetch_cmd
    if [[ "$FORCE_TAGS" == "true" ]]; then
      fetch_cmd=(git fetch upstream --force "$tag_refspec")
    else
      fetch_cmd=(git fetch upstream "$tag_refspec")
    fi
    if ! fetch_output=$("${fetch_cmd[@]}" 2>&1); then
      echo "$fetch_output"
      echo "⚠️  rust-v* 标签抓取失败或部分失败（可能是网络受限或标签冲突）。将继续使用本地 tag 视图。"
      if echo "$fetch_output" | grep -Eiq 'clobber|拒绝|已拒绝|would.*clobber.*tag|标签'; then
        echo "ℹ️  检测到标签冲突：本地与上游存在同名但指向不同的 tag。"
        echo "    - 如需强制对齐上游标签，可追加 --force-tags 重新运行本命令（将覆盖本地同名 tag）。"
        echo "    - 或仅对单个标签：git fetch upstream tag <name> -f"
      fi
    fi
    # 3) Optionally fetch all tags if requested
    if [[ "$FETCH_ALL_TAGS" == "true" ]]; then
      if ! fetch_output=$(git fetch upstream --tags 2>&1); then
        echo "$fetch_output"
        echo "⚠️  全量标签抓取失败或部分失败（可能包含与本地不相关的标签冲突）。"
      fi
    fi
  fi
}

merge_ref() {
  local ref="$1"; shift
  local branch="$1"; shift
  local use_rebase_flag="$1"; shift # true|false
  local create_branch="$1"; shift   # true|false
  local do_push="$1"; shift         # true|false
  local tag_baseline="$1"; shift    # true|false
  local baseline_prefix="$1"; shift # string
  local push_tags="$1"; shift       # true|false

  local base_branch="$branch"

  if [[ "$create_branch" == "true" ]]; then
    local suffix
    suffix="$(date +%Y%m%d-%H%M%S)"
    local new_branch
    # Normalize ref for branch name (replace slashes)
    new_branch="sync/$(echo "$ref" | tr '/' '-')-${suffix}"
    echo "Creating branch: $new_branch (from origin/$base_branch)"
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] git checkout -B \"$new_branch\" \"origin/$base_branch\""
    else
      git checkout -B "$new_branch" "origin/$base_branch"
    fi
  else
    echo "Checking out $base_branch"
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] git checkout \"$base_branch\""
      echo "[dry-run] git pull --ff-only origin \"$base_branch\""
    else
      git checkout "$base_branch"
      git pull --ff-only origin "$base_branch" || true
    fi
  fi

  if [[ "$use_rebase_flag" == "true" ]]; then
    echo "Rebasing onto $ref"
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] git rebase \"$ref\""
    else
      git rebase "$ref"
    fi
  else
    local cur_branch
    if [[ "$DRY_RUN" == "true" ]]; then
      cur_branch="(current-branch)"
    else
      cur_branch="$(git rev-parse --abbrev-ref HEAD)"
    fi
    echo "Merging $ref into ${cur_branch}"
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] git merge --no-ff \"$ref\""
    else
      git merge --no-ff "$ref"
    fi
  fi

  if [[ "$do_push" == "true" ]]; then
    local current
    if [[ "$DRY_RUN" == "true" ]]; then
      current="(current-branch)"
    else
      current="$(git rev-parse --abbrev-ref HEAD)"
    fi
    echo "Pushing branch: $current"
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "[dry-run] git push -u origin \"$current\""
    else
      git push -u origin "$current"
    fi
  fi

  # If requested and ref is an upstream tag, create a baseline tag for traceability
  if [[ "$tag_baseline" == "true" ]]; then
    # Extract plain tag name if ref is refs/tags/<name>
    local name="$ref"
    name="${name#refs/tags/}"
    if [[ "$name" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
      local base_tag
      base_tag="${baseline_prefix}${name}"
      echo "Tagging baseline: $base_tag"
      if [[ "$DRY_RUN" == "true" ]]; then
        echo "[dry-run] git tag -a \"$base_tag\" -m \"Baseline: $name\""
        if [[ "$push_tags" == "true" ]]; then
          echo "[dry-run] git push origin \"$base_tag\""
        fi
      else
        git tag -a "$base_tag" -m "Baseline: $name"
        if [[ "$push_tags" == "true" ]]; then
          echo "Pushing tag: $base_tag"
          git push origin "$base_tag"
        fi
      fi
    else
      echo "Note: --tag-baseline only applies when merging an upstream rust-v* tag; skipping for '$ref'" >&2
    fi
  fi
}

list_tags() {
  local limit="$1"
  local re
  if [[ "$PRE_ONLY" == "true" ]]; then
    re='^rust-v[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*$'
  elif [[ "$INCLUDE_PRE" == "true" ]]; then
    re='^rust-v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
  else
    re='^rust-v[0-9]+\.[0-9]+\.[0-9]+$'
  fi
  git for-each-ref refs/tags --sort=-creatordate --format '%(refname:short)' \
    | grep -E "$re" \
    | head -n "${limit}"
}

current_baseline() {
  local prefix="$1" # baseline tag prefix
  # First, try nearest baseline tag reachable from HEAD
  if git describe --tags --match "${prefix}rust-v*" --abbrev=0 >/dev/null 2>&1; then
    git describe --tags --match "${prefix}rust-v*" --abbrev=0
    return 0
  fi
  # Fallback: parse last merge message mentioning an upstream rust tag
  local msg
  msg=$(git log --grep='^Merge upstream rust-v[0-9]\+\.[0-9]\+\.[0-9]\+' -n 1 --format='%s' || true)
  if [[ -n "$msg" ]]; then
    echo "$msg" | sed -E 's/.*(rust-v[0-9]+\.[0-9]+\.[0-9]+).*/\1/'
    return 0
  fi
  # If there are baseline tags but none are reachable, surface a helpful hint
  local latest_base
  latest_base=$(git for-each-ref refs/tags --sort=-taggerdate --format '%(refname:short)' \
                 | grep -E "^${prefix}rust-v[0-9]+\\.[0-9]+\\.[0-9]+$" | head -n1 || true)
  if [[ -n "$latest_base" ]]; then
    local head_branch
    head_branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD)"
    echo "Hint: found baseline tag '${latest_base}', but it is not reachable from current '${head_branch}'." >&2
    echo "      You may be on a different branch; either switch/merge, or rerun with --from ${latest_base#${prefix}}." >&2
  fi
  echo "(no baseline tag or upstream rust-v merge found)"
}

main() {
  local cmd=""
  local branch="main"
  local upstream_url="$UPSTREAM_URL_DEFAULT"
  local use_rebase="false"
  local no_branch="false"
  local do_push="false"
  local limit="20"
  local tag_baseline="false"
  local baseline_prefix="base-"
  local push_tags="false"
  local from_tag=""
  local to_tag=""
  DRY_RUN="false"

  if [[ $# -eq 0 ]]; then
    usage; exit 1
  fi

  cmd="$1"; shift

  # Parse global flags from anywhere in the argument list. Collect non-flag
  # tokens as positional arguments in order.
  positional=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --branch)
        shift; branch="${1:-}" || true ;;
      --upstream-url)
        shift; upstream_url="${1:-}" || true ;;
      --rebase)
        use_rebase="true" ;;
      --no-branch)
        no_branch="true" ;;
      --push)
        do_push="true" ;;
      --tag-baseline)
        tag_baseline="true" ;;
      --baseline-prefix)
        shift; baseline_prefix="${1:-base-}" || true ;;
      --push-tags)
        push_tags="true" ;;
      --limit)
        shift; limit="${1:-20}" || true ;;
      --from)
        shift; from_tag="${1:-}" || true ;;
      --to)
        shift; to_tag="${1:-}" || true ;;
      --dry-run)
        DRY_RUN="true" ;;
      --force-tags)
        FORCE_TAGS="true" ;;
      --include-pre)
        INCLUDE_PRE="true" ;;
      --pre-only)
        PRE_ONLY="true" ;;
      --fetch-all-tags)
        FETCH_ALL_TAGS="true" ;;
      -h|--help)
        usage; exit 0 ;;
      --*)
        echo "Unknown option: $1" >&2; exit 1 ;;
      *)
        positional+=("$1") ;;
    esac
    shift || true
  done
  set -- "${positional[@]:-}"

  case "$cmd" in
    merge-main)
      ensure_clean
      ensure_upstream_remote "$upstream_url"
      echo "Sync mode: merge upstream/main into $branch (strategy=${use_rebase})"
      local ref="upstream/main"
      # create_branch is the inverse of --no-branch
      local create_branch
      create_branch=$([[ "$no_branch" == "false" ]] && echo true || echo false)
      merge_ref "$ref" "$branch" "$use_rebase" "$create_branch" "$do_push" "$tag_baseline" "$baseline_prefix" "$push_tags"
      ;;
    merge-tag)
      ensure_clean
      ensure_upstream_remote "$upstream_url"
      local tag="${1:-}"
      if [[ -z "$tag" ]]; then
        echo "ERROR: merge-tag requires a tag argument (e.g., rust-v0.21.0)" >&2
        exit 1
      fi
      # Ensure tag exists locally after fetch
      if ! git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
        if [[ "$DRY_RUN" == "true" ]]; then
          echo "[dry-run] Tag not found locally: ${tag}. Using local view only; run without --dry-run to fetch latest upstream tags."
        else
          echo "ERROR: Tag not found: ${tag}" >&2
          exit 1
        fi
      fi
      echo "Sync mode: merge ${tag} into $branch (strategy=${use_rebase})"
      local create_branch
      create_branch=$([[ "$no_branch" == "false" ]] && echo true || echo false)
      merge_ref "refs/tags/${tag}" "$branch" "$use_rebase" "$create_branch" "$do_push" "$tag_baseline" "$baseline_prefix" "$push_tags"
      ;;
    list-tags)
      # Read-only, but refresh upstream tags for accuracy
      ensure_upstream_remote "$upstream_url"
      list_tags "$limit"
      ;;
    merge-series)
      ensure_clean
      ensure_upstream_remote "$upstream_url"
      # Determine baseline
      local base_ref=""
      if [[ -n "$from_tag" ]]; then
        base_ref="$from_tag"
      else
        # Attempt to detect baseline automatically via baseline tag
        local base_detect
        base_detect=$(current_baseline "$baseline_prefix")
        if [[ "$base_detect" == \(* ]]; then
          echo "ERROR: Could not detect current baseline. Specify with --from <rust-vX.Y.Z>." >&2
          exit 1
        fi
        # base_detect may be like base-rust-v0.21.0 or rust-v0.21.0
        base_ref="${base_detect#${baseline_prefix}}"
      fi

  if ! git rev-parse -q --verify "refs/tags/${base_ref}" >/dev/null; then
    echo "ERROR: Baseline tag not found: ${base_ref}" >&2
    exit 1
  fi

      ensure_upstream_remote "$upstream_url"

      # Build ordered tag list
      if [[ "$DRY_RUN" == "true" ]]; then
        echo "[dry-run] Using locally cached tags (no fetch)."
      fi
      mapfile -t all_tags < <(git for-each-ref refs/tags --sort=creatordate --format '%(refname:short)' \
        | grep -E '^rust-v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$')

      # Find start index strictly after baseline
      local start_index=-1
      for i in "${!all_tags[@]}"; do
        if [[ "${all_tags[$i]}" == "$base_ref" ]]; then
          start_index=$((i+1))
          break
        fi
      done
      if [[ $start_index -lt 0 ]]; then
        echo "ERROR: Baseline ${base_ref} not found among upstream rust-v* tags" >&2
        exit 1
      fi

      # Determine end index
      local end_index=$((${#all_tags[@]} - 1))
      if [[ -n "$to_tag" ]]; then
        for i in "${!all_tags[@]}"; do
          if [[ "${all_tags[$i]}" == "$to_tag" ]]; then
            end_index=$i
            break
          fi
        done
      fi

      if [[ $start_index -gt $end_index ]]; then
        echo "Already up to date with upstream tags at baseline ${base_ref}."
        # If requested, ensure a baseline tag exists even when no merges were needed
        if [[ "$tag_baseline" == "true" ]]; then
          base_tag_name="${baseline_prefix}${base_ref}"
          if git rev-parse -q --verify "refs/tags/${base_tag_name}" >/dev/null; then
            echo "Baseline tag already exists: ${base_tag_name}"
          else
            echo "Creating baseline tag for current HEAD: ${base_tag_name}"
            if [[ "$DRY_RUN" == "true" ]]; then
              echo "[dry-run] git tag -a \"$base_tag_name\" -m \"Baseline: ${base_ref}\""
              if [[ "$push_tags" == "true" ]]; then
                echo "[dry-run] git push origin \"$base_tag_name\""
              fi
            else
              git tag -a "$base_tag_name" -m "Baseline: ${base_ref}"
              if [[ "$push_tags" == "true" ]]; then
                git push origin "$base_tag_name"
              fi
            fi
          fi
        fi
        exit 0
      fi

      # Cap by --limit
      local merges_done=0

      # Prepare working branch
      local create_branch
      create_branch=$([[ "$no_branch" == "false" ]] && echo true || echo false)

      # Start from target base branch
      if [[ "$create_branch" == "true" ]]; then
        local ts
        ts=$(date +%Y%m%d-%H%M%S)
        local series_branch
        series_branch="sync/series-from-${base_ref}-${ts}"
        echo "Creating branch: $series_branch (from origin/$branch)"
        if [[ "$DRY_RUN" == "true" ]]; then
          echo "[dry-run] git checkout -B \"$series_branch\" \"origin/$branch\""
        else
          git checkout -B "$series_branch" "origin/$branch"
        fi
        current_branch_name="$series_branch"
      else
        echo "Checking out $branch"
        if [[ "$DRY_RUN" == "true" ]]; then
          echo "[dry-run] git checkout \"$branch\""
          echo "[dry-run] git pull --ff-only origin \"$branch\""
        else
          git checkout "$branch"
          git pull --ff-only origin "$branch" || true
        fi
        current_branch_name="$branch"
      fi

      for ((i=start_index; i<=end_index; i++)); do
        if [[ $merges_done -ge $limit ]]; then
          echo "Reached limit ($limit). Stop after merging ${merges_done} tag(s)."
          break
        fi
        next_tag="${all_tags[$i]}"
        # Skip tags that are not selected by the current filter (stable-only by default)
        if [[ "$PRE_ONLY" == "true" ]]; then
          if ! [[ "$next_tag" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*$ ]]; then
            continue
          fi
        elif [[ "$INCLUDE_PRE" == "true" ]]; then
          # accept both stable and pre-release
          :
        else
          # stable-only
          if ! [[ "$next_tag" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            continue
          fi
        fi
        echo "---"
        echo "Merging ${next_tag} into ${current_branch_name}"
        if [[ "$use_rebase" == "true" ]]; then
          if [[ "$DRY_RUN" == "true" ]]; then
            echo "[dry-run] git rebase \"refs/tags/${next_tag}\""
          elif ! git rebase "refs/tags/${next_tag}"; then
            echo "Merge/rebase conflict at ${next_tag}. Resolve conflicts, commit/continue rebase, then rerun this command to continue from the next tag." >&2
            exit 2
          fi
        else
          if [[ "$DRY_RUN" == "true" ]]; then
            echo "[dry-run] git merge --no-ff -m \"Merge upstream ${next_tag}\" \"refs/tags/${next_tag}\""
          elif ! git merge --no-ff -m "Merge upstream ${next_tag}" "refs/tags/${next_tag}"; then
            echo "Merge conflict at ${next_tag}. Resolve conflicts, commit the merge, then rerun this command to continue from the next tag." >&2
            exit 2
          fi
        fi

        # After each successful merge, create a baseline tag for traceability
        local base_tag_name
        base_tag_name="${baseline_prefix}${next_tag}"
        echo "Tagging baseline: ${base_tag_name}"
        if [[ "$DRY_RUN" == "true" ]]; then
          echo "[dry-run] git tag -a \"$base_tag_name\" -m \"Baseline: ${next_tag}\""
          if [[ "$push_tags" == "true" ]]; then
            echo "[dry-run] git push origin \"$base_tag_name\""
          fi
        else
          git tag -a "$base_tag_name" -m "Baseline: ${next_tag}" || true
          if [[ "$push_tags" == "true" ]]; then
            git push origin "$base_tag_name"
          fi
        fi

        # Optionally push branch progress
        if [[ "$do_push" == "true" ]]; then
          if [[ "$DRY_RUN" == "true" ]]; then
            echo "[dry-run] git push -u origin \"$current_branch_name\""
          else
            local current
            current="$(git rev-parse --abbrev-ref HEAD)"
            git push -u origin "$current"
          fi
        fi

        merges_done=$((merges_done + 1))
      done

      if [[ $merges_done -gt 0 ]]; then
        echo "Completed ${merges_done} merge(s). Current baseline: ${baseline_prefix}${all_tags[$((start_index+merges_done-1))]}"
      else
        echo "No merges performed. Already at latest or outside limits."
      fi
      ;;
    init-baseline)
      ensure_clean
      local tag="${1:-}"
      if [[ -z "$tag" ]]; then
        echo "ERROR: init-baseline requires a tag argument (e.g., rust-v0.21.0)" >&2
        exit 1
      fi
      if ! git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
        if [[ "$DRY_RUN" == "true" ]]; then
          echo "[dry-run] Baseline source tag not found locally: ${tag}. Using local view only; run without --dry-run to fetch latest upstream tags."
        else
          echo "ERROR: Tag not found: ${tag}" >&2
          exit 1
        fi
      fi
      local base_tag_name
      base_tag_name="${baseline_prefix}${tag}"
      if git rev-parse -q --verify "refs/tags/${base_tag_name}" >/dev/null; then
        echo "Baseline tag already exists: ${base_tag_name}"
        exit 0
      fi
      echo "Creating baseline tag at current HEAD: ${base_tag_name} (marks HEAD as based on ${tag})"
      if [[ "$DRY_RUN" == "true" ]]; then
        echo "[dry-run] git tag -a \"$base_tag_name\" -m \"Baseline: ${tag}\""
        if [[ "$push_tags" == "true" ]]; then
          echo "[dry-run] git push origin \"$base_tag_name\""
        fi
      else
        git tag -a "$base_tag_name" -m "Baseline: ${tag}"
        if [[ "$push_tags" == "true" ]]; then
          git push origin "$base_tag_name"
        fi
      fi
      ;;
    current-baseline)
      # Pure read-only; do not require clean tree or network
      current_baseline "$baseline_prefix"
      ;;
    *)
      echo "Unknown command: $cmd" >&2
      usage; exit 1 ;;
  esac
}

main "$@"
