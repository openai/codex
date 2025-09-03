#!/usr/bin/env bash
set -euo pipefail

# Append the full source code of every file under codex-rs/core/src
# into a given Markdown file, wrapped between BEGIN/END markers.
#
# Usage:
#   scripts/append_core_src_code_to_md.sh docs/core-src-directories-and-files.md

TARGET_MD=${1:?"Usage: $0 <target-md>"}
SRC_ROOT="codex-rs/core/src"

if [[ ! -d "$SRC_ROOT" ]]; then
  echo "Error: $SRC_ROOT not found" >&2
  exit 1
fi

mkdir -p "$(dirname "$TARGET_MD")"
touch "$TARGET_MD"

BEGIN_MARK='<!-- BEGIN: core-src-all-code -->'
END_MARK='<!-- END: core-src-all-code -->'

# Remove previous generated block if present.
if grep -q "$BEGIN_MARK" "$TARGET_MD"; then
  # keep everything before BEGIN, and everything after END
  awk -v begin="$BEGIN_MARK" -v end="$END_MARK" '
    $0==begin {inblock=1; print; next}
    $0==end   {inblock=0; print; next}
    !inblock { print }
  ' "$TARGET_MD" | awk -v begin="$BEGIN_MARK" -v end="$END_MARK" '
    BEGIN { seen_begin=0; }
    {
      if ($0==begin) { seen_begin=1 }
      if (!seen_begin) print
      if ($0==end) seen_begin=0
    }
  ' >"$TARGET_MD.tmp"
  mv "$TARGET_MD.tmp" "$TARGET_MD"
fi

{
  echo "$BEGIN_MARK"
  echo "\n> 自動生成: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  echo
  while IFS= read -r -d '' f; do
    rel="$f"
    ext="${f##*.}"
    case "$ext" in
      rs) lang="rust" ;;
      md) lang="md" ;;
      sbpl) lang="text" ;;
      *) lang="text" ;;
    esac
    printf '### %s\n\n' "$rel"
    printf '```%s\n' "$lang"
    cat "$f"
    printf '\n```\n\n'
  done < <(find "$SRC_ROOT" -type f -print0 | sort -z)
  echo "$END_MARK"
} >>"$TARGET_MD"

echo "Appended core/src code into $TARGET_MD"

