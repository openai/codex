#!/usr/bin/env bash
set -euo pipefail

# Generate a single Markdown file that contains the full source of
# every file under codex-rs/core/src, grouped by path.
#
# Usage:
#   scripts/generate_core_src_bundle.sh [OUTPUT_MD]
#
# Defaults to docs/core-src-all-code.md

ROOT_DIR="codex-rs/core/src"
OUT_MD="${1:-docs/core-src-all-code.md}"

if [[ ! -d "$ROOT_DIR" ]]; then
  echo "Error: $ROOT_DIR not found" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT_MD")"

{
  echo "# codex-rs/core/src 全ファイルコード集"
  echo
  echo "生成元: \`$ROOT_DIR\`\n"

  while IFS= read -r -d '' f; do
    rel="$f"
    # Pick a fence language based on extension
    ext="${f##*.}"
    case "$ext" in
      rs) lang="rust" ;;
      md) lang="md" ;;
      sbpl) lang="text" ;;
      *) lang="text" ;;
    esac

    printf '## %s\n\n' "$rel"
    # Print fenced block with language, escaping backticks for the shell.
    printf '```%s\n' "$lang"
    cat "$f"
    printf '\n```\n\n'
  done < <(find "$ROOT_DIR" -type f -print0 | sort -z)
} >"$OUT_MD"

echo "Wrote $(wc -l <"$OUT_MD") lines to $OUT_MD"
