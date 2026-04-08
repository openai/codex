#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/tui-resize-stream-repro.sh [options]

Start Codex in tmux, send a long table-heavy prompt, resize repeatedly while
the response streams, and capture scrollback after each resize.

Options:
  --session NAME      tmux session name (default: codex-resize-repro-$$)
  --out DIR           output directory (default: /tmp/codex-resize-repro-*)
  --duration SECONDS  resize duration after prompt submission (default: 20)
  --sleep SECONDS     delay between resizes (default: 0.20)
  --startup SECONDS   wait after launching Codex before sending prompt (default: 8)
  --expect MODE       either, artifact, or clean (default: either)
  --keep-session      leave the tmux session running for manual inspection
  --skip-prebuild     do not run cargo build --bin codex before launching tmux
  --prompt TEXT       override the default prompt
  -h, --help          show this help

Examples:
  scripts/tui-resize-stream-repro.sh --keep-session
  scripts/tui-resize-stream-repro.sh --expect clean --duration 30
EOF
}

session="codex-resize-repro-$$"
out_dir=""
duration=20
sleep_interval=0.20
startup_wait=8
expect="either"
keep_session=0
prebuild=1
prompt="Produce 6 long Markdown tables, 25 rows each. Include emojis, bold, italic, strikethrough, inline code, markdown links, code-like values, short cells, wrapped cells, and pipe characters escaped inside cells. Include the token RESIZE_REPRO_SENTINEL in several table cells. Stream the answer directly. Do not use tools."

while [[ $# -gt 0 ]]; do
  case "$1" in
    --session)
      session="$2"
      shift 2
      ;;
    --out)
      out_dir="$2"
      shift 2
      ;;
    --duration)
      duration="$2"
      shift 2
      ;;
    --sleep)
      sleep_interval="$2"
      shift 2
      ;;
    --startup)
      startup_wait="$2"
      shift 2
      ;;
    --expect)
      expect="$2"
      shift 2
      ;;
    --keep-session)
      keep_session=1
      shift
      ;;
    --skip-prebuild)
      prebuild=0
      shift
      ;;
    --prompt)
      prompt="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$expect" in
  either|artifact|clean) ;;
  *)
    echo "--expect must be one of: either, artifact, clean" >&2
    exit 2
    ;;
esac

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux is required for this repro" >&2
  exit 2
fi

if [[ "$prebuild" -eq 1 ]]; then
  cargo build --bin codex
fi

if [[ -z "$out_dir" ]]; then
  out_dir="$(mktemp -d "${TMPDIR:-/tmp}/codex-resize-repro-XXXXXX")"
else
  mkdir -p "$out_dir"
fi

captures_dir="$out_dir/captures"
log_dir="$out_dir/logs"
mkdir -p "$captures_dir" "$log_dir"

if tmux has-session -t "$session" 2>/dev/null; then
  echo "tmux session already exists: $session" >&2
  exit 2
fi

cleanup() {
  if [[ "$keep_session" -eq 0 ]] && tmux has-session -t "$session" 2>/dev/null; then
    tmux kill-session -t "$session"
  fi
}
trap cleanup EXIT

capture() {
  local idx="$1"
  local width="$2"
  local height="$3"
  local stem
  stem="$(printf '%03d_%sx%s' "$idx" "$width" "$height")"
  tmux capture-pane -t "$session" -p -S -5000 >"$captures_dir/$stem.txt"
  tmux capture-pane -t "$session" -p -e -S -5000 >"$captures_dir/$stem.ansi"
}

wait_for_tui_ready() {
  local deadline=$((SECONDS + startup_wait))
  while [[ "$SECONDS" -lt "$deadline" ]]; do
    if tmux capture-pane -t "$session" -p | grep -q 'OpenAI Codex'; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

detect_artifacts() {
  local report="$out_dir/artifacts.txt"
  : >"$report"

  for capture in "$captures_dir"/*.txt; do
    [[ -e "$capture" ]] || continue
    local raw_count box_count sentinel_count
    raw_count="$(grep -Ec '^[[:space:]]*\|.*\|' "$capture" || true)"
    box_count="$(grep -Ec '[┌┬┐└┴┘├┼┤│]' "$capture" || true)"
    sentinel_count="$(grep -Ec 'RESIZE_REPRO_SENTINEL' "$capture" || true)"

    if [[ "$raw_count" -ge 3 && "$box_count" -ge 3 ]]; then
      printf '%s mixed_raw_and_boxed_table raw=%s boxed=%s sentinel=%s\n' \
        "$(basename "$capture")" "$raw_count" "$box_count" "$sentinel_count" >>"$report"
    fi

    local prompt_count
    prompt_count="$(grep -Ec '^[[:space:]]*› ' "$capture" || true)"
    if [[ "$prompt_count" -ge 2 && "$box_count" -ge 3 ]]; then
      printf '%s duplicated_inline_prompt_with_table prompts=%s boxed=%s sentinel=%s\n' \
        "$(basename "$capture")" "$prompt_count" "$box_count" "$sentinel_count" >>"$report"
    fi

    if awk '
      /^[[:space:]]*\|.*\|[[:space:]]*$/ {
        pipe_rows++
        next
      }
      pipe_rows >= 3 && /[┌┬┐└┴┘├┼┤│]/ {
        found = 1
      }
      END { exit found ? 0 : 1 }
    ' "$capture"; then
      printf '%s raw_table_precedes_boxed_table\n' "$(basename "$capture")" >>"$report"
    fi
  done

  [[ -s "$report" ]]
}

tmux new-session -d -s "$session" -x 100 -y 34

launch_cmd="RUST_LOG=trace ./target/debug/codex --no-alt-screen -C '$PWD' -c 'log_dir=$log_dir'"
tmux send-keys -t "$session" -l "$launch_cmd"
tmux send-keys -t "$session" Enter
if ! wait_for_tui_ready; then
  capture 0 100 34
  echo "Codex TUI did not become ready within ${startup_wait}s" >&2
  echo "captures: $captures_dir" >&2
  exit 1
fi

capture 0 100 34

tmux send-keys -t "$session" -l "$prompt"
sleep 0.2
tmux send-keys -t "$session" Enter

widths=(110 62 132 74 120 56 140)
heights=(36 28 38 24 34 22 40)
idx=1
deadline=$((SECONDS + duration))

while [[ "$SECONDS" -lt "$deadline" ]]; do
  for i in "${!widths[@]}"; do
    width="${widths[$i]}"
    height="${heights[$i]}"
    tmux resize-window -t "$session" -x "$width" -y "$height"
    sleep "$sleep_interval"
    capture "$idx" "$width" "$height"
    idx=$((idx + 1))
    if [[ "$SECONDS" -ge "$deadline" ]]; then
      break
    fi
  done
done

sleep 2
capture "$idx" "final" "final"

cat >"$out_dir/metadata.txt" <<EOF
session=$session
cwd=$PWD
log_dir=$log_dir
duration=$duration
sleep_interval=$sleep_interval
startup_wait=$startup_wait
expect=$expect
keep_session=$keep_session
prebuild=$prebuild
prompt=$prompt
EOF

if detect_artifacts; then
  echo "resize-stream repro captured possible artifacts:"
  cat "$out_dir/artifacts.txt"
  echo "captures: $captures_dir"
  if [[ "$expect" == "clean" ]]; then
    exit 1
  fi
  exit 0
fi

echo "resize-stream repro found no text artifacts"
echo "captures: $captures_dir"
if [[ "$expect" == "artifact" ]]; then
  exit 1
fi
