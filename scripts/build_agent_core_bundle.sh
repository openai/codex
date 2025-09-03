#!/usr/bin/env bash
set -euo pipefail

# Build a single Markdown document that contains:
#  1) An ASCII architecture diagram + short explanation
#  2) A directory/file listing under codex-rs/core/src (incl. exec_command/)
#  3) The full source code of every file under codex-rs/core/src
#
# Usage:
#   scripts/build_agent_core_bundle.sh [OUTPUT_MD]
#
# Defaults to docs/agent-core-bundle.md

OUT_MD="${1:-docs/agent-core-bundle.md}"
SRC_ROOT="codex-rs/core/src"

if [[ ! -d "$SRC_ROOT" ]]; then
  echo "Error: $SRC_ROOT not found" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT_MD")"

list_files() {
  find "$SRC_ROOT" -type f | sort
}

cat >"$OUT_MD" <<'EOF'
# Codex Core – Architecture, Directory Map, and Full Source Bundle

This document compiles the core agent architecture (overview), the directory/file map under `codex-rs/core/src`, and the full source code for quick reference when building a terminal AI agent.

## Architecture Overview

```
User ──▶ TUI (chatwidget/bottom_pane)
            │  Enter/keys → AppEvent
            ▼
        codex-core Codex::submit ──▶ submission_loop/run_turn
            │                         │
            │                  builds Prompt (instructions + tools)
            │                         │
            │                  ModelClient.stream() ──▶ OpenAI Provider (Responses/Chat SSE)
            │                         │                              ▲
            │                         └─ process_*_sse → ResponseEvent ┘
            │                                   │
            │                    tools (openai_tools): shell/apply_patch/update_plan/MCP
            │                                   │
            │                 handle tool call → exec/exec_command → seatbelt/landlock sandbox
            │                                   │
            └───────────────────────────────────┴─▶ events back to TUI (deltas, OutputItemDone, Completed)
```

Key building blocks:
- Session + turns: `codex.rs` manages lifecycle and event flow
- Model client/stream: `client.rs`, `chat_completions.rs`
- Tools: `openai_tools.rs`, `plan_tool.rs`, `tool_apply_patch.rs`, `exec_command/*`
- Safe execution: `exec.rs`, `seatbelt.rs`, `landlock.rs`, `spawn.rs`, `safety.rs`
- Config/model: `config.rs`, `config_types.rs`, `model_family.rs`, `model_provider_info.rs`

## Directory Map: codex-rs/core/src

EOF

# Append directory listing
list_files | awk '{print "- `"$0"`"}' >>"$OUT_MD"

cat >>"$OUT_MD" <<'EOF'

---

## Full Source Code: codex-rs/core/src

EOF

# Append all files with fenced code blocks
while IFS= read -r -d '' f; do
  rel="$f"
  ext="${f##*.}"
  case "$ext" in
    rs) lang="rust" ;;
    md) lang="md" ;;
    sbpl) lang="text" ;;
    *) lang="text" ;;
  esac
  printf '### %s\n\n' "$rel" >>"$OUT_MD"
  printf '```%s\n' "$lang" >>"$OUT_MD"
  cat "$f" >>"$OUT_MD"
  printf '\n```\n\n' >>"$OUT_MD"
done < <(find "$SRC_ROOT" -type f -print0 | sort -z)

echo "Wrote $(wc -l <"$OUT_MD") lines to $OUT_MD"

