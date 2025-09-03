#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${ROOT_DIR}/codex-rs/tui"
OUT_DIR="${ROOT_DIR}/docs"
OUT_FILE="${OUT_DIR}/tui-all-files.md"
NUM_BATCHES="${NUM_BATCHES:-10}"

mkdir -p "${OUT_DIR}"

echo "[dump-tui-to-md] Target: ${TARGET_DIR}" >&2

list_files() {
  if command -v rg >/dev/null 2>&1; then
    (cd "${ROOT_DIR}" && rg --files "codex-rs/tui")
  elif git -C "${ROOT_DIR}" rev-parse >/dev/null 2>&1; then
    git -C "${ROOT_DIR}" ls-files "codex-rs/tui"
  else
    # Fallback: include all, then filter out typical build dirs if present
    find "${TARGET_DIR}" -type f \( -path "*/target/*" -prune -false -o -print \) | sed "s#^${ROOT_DIR}/##"
  fi
}

detect_lang() {
  local f="$1" ext
  ext="${f##*.}"
  case "${ext}" in
    rs) echo "rust" ;;
    toml) echo "toml" ;;
    md|markdown) echo "md" ;;
    json) echo "json" ;;
    yml|yaml) echo "yaml" ;;
    sh|bash) echo "bash" ;;
    ts) echo "ts" ;;
    tsx) echo "tsx" ;;
    js) echo "js" ;;
    jsx) echo "jsx" ;;
    css) echo "css" ;;
    html|htm) echo "html" ;;
    snap|txt|log) echo "text" ;;
    *) echo "" ;;
  esac
}

is_binary() {
  local f="$1"
  # Fast path: treat common text extensions as text
  case "${f##*.}" in
    rs|toml|md|markdown|json|yml|yaml|sh|bash|ts|tsx|js|jsx|css|html|htm|snap|txt|log|new|lock)
      return 1 ;;
  esac
  if command -v file >/dev/null 2>&1; then
    local mime
    mime=$(file -b --mime "$f" || true)
    if echo "$mime" | grep -q "charset=binary"; then
      return 0
    fi
  fi
  if grep -Iq . "$f" 2>/dev/null; then
    return 1
  else
    return 0
  fi
}

gen_tree() {
  if command -v tree >/dev/null 2>&1; then
    # Show hidden files, skip typical build dirs if present
    (cd "${ROOT_DIR}" && tree -a -I 'target' "codex-rs/tui")
  else
    # Fallback: simple find with indentation approximation
    (cd "${ROOT_DIR}" && find "codex-rs/tui" \( -path "*/target/*" -prune -false -o -print \) | sort | \
      awk -F'/' '{
        base=$0
        sub(/^codex-rs\/tui\/?/,"",base)
        if (base=="") { print "codex-rs/tui"; next }
        n=split(base, a, "/");
        indent=""
        for(i=1;i<=n;i++) indent=indent"  "
        print indent a[n]
      }')
  fi
}

FILES_TMP="$(mktemp)"
trap 'rm -f "$FILES_TMP"' EXIT

list_files | sort > "$FILES_TMP"
TOTAL_FILES=$(wc -l < "$FILES_TMP" | tr -d ' ')
if [ "$TOTAL_FILES" -eq 0 ]; then
  echo "[dump-tui-to-md] No files found under ${TARGET_DIR}" >&2
  exit 1
fi

# Compute batch size (ceil)
if [ "$NUM_BATCHES" -lt 1 ]; then NUM_BATCHES=1; fi
BATCH_SIZE=$(( (TOTAL_FILES + NUM_BATCHES - 1) / NUM_BATCHES ))
if [ "$BATCH_SIZE" -lt 1 ]; then BATCH_SIZE=1; fi

# Header
{
  echo "# codex-rs/tui ファイル一覧と中身"
  echo
  echo "- 生成日時: $(date +"%Y-%m-%d %H:%M:%S %z")"
  echo "- 総ファイル数: ${TOTAL_FILES}"
  echo "- バッチ数: ${NUM_BATCHES} (1バッチあたり最大 ${BATCH_SIZE} 件)"
  echo
  echo "## ディレクトリ構成"
  echo
  echo '```text'
  gen_tree
  echo '```'
  echo
  echo "## ファイル本文"
  echo
} >"${OUT_FILE}"

batch_index=1
start_line=1

while [ "$start_line" -le "$TOTAL_FILES" ]; do
  end_line=$(( start_line + BATCH_SIZE - 1 ))
  if [ "$end_line" -gt "$TOTAL_FILES" ]; then end_line=$TOTAL_FILES; fi

  {
    echo "### Batch ${batch_index} (${start_line}-${end_line}/${TOTAL_FILES})"
    echo
  } >>"${OUT_FILE}"

  sed -n "${start_line},${end_line}p" "$FILES_TMP" | while IFS= read -r relpath; do
    # Resolve absolute path properly
    if [ "${relpath#"/"}" != "${relpath}" ]; then
      abs_path="$relpath"
    else
      abs_path="${ROOT_DIR}/${relpath}"
    fi

    # Display relative path if possible
    display_path="$relpath"
    case "$display_path" in
      ${ROOT_DIR}/*) display_path="${display_path#${ROOT_DIR}/}" ;;
    esac

    lang=$(detect_lang "$display_path")

    echo "#### ${display_path}" >>"${OUT_FILE}"
    if is_binary "$abs_path"; then
      echo '```text' >>"${OUT_FILE}"
      echo "(バイナリのため内容省略)" >>"${OUT_FILE}"
      echo '```' >>"${OUT_FILE}"
      echo >>"${OUT_FILE}"
      continue
    fi

    if [ -n "$lang" ]; then
      echo "${lang}" | sed 's//`/g' >>"${OUT_FILE}"
    else
      echo '```' >>"${OUT_FILE}"
    fi
    cat "$abs_path" >>"${OUT_FILE}"
    echo >>"${OUT_FILE}"
    echo '```' >>"${OUT_FILE}"
    echo >>"${OUT_FILE}"
  done

  batch_index=$(( batch_index + 1 ))
  start_line=$(( end_line + 1 ))
done

echo "[dump-tui-to-md] Done: ${OUT_FILE}" >&2
