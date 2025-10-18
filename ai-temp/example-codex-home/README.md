# Sample Codex Home Setup

This directory mirrors a minimal `~/.codex` layout so you can try the multi-agent loader without touching your real config. The flow now demonstrates a chained delegation sequence that mixes detached and synchronous agents:

1. Main agent launches `request_summarizer` in **detached** mode. The summarizer writes a ≤75-word markdown file to `/tmp/notes/` capturing the fresh user request.
2. While the detached run finishes, the agent restates the request and briefs `ideas_provider`.
3. `ideas_provider` consults both `creative_ideas` and `conservative_ideas`, then recommends a blended approach.
4. Main agent forwards that plan to `critic` for risk review before replying to the user.
5. Once the detached summarizer completes, the coordinator checks `/agent`, reviews the saved file (if desired), and dismisses the run to keep the list tidy.

## Directory Structure

- `config.toml`: baseline settings used when no sub-agent is selected.
- `AGENTS.md`: default instruction set for the primary agent (orchestrator).
- `agents/request_summarizer/`: logs the latest user request to `/tmp/notes/*.md` via a detached delegate run.
- `agents/ideas_provider/`: synthesizes outputs from creative and conservative delegates.
- `agents/critic/`: GPT-5-nano sub-agent that reviews the leading option.
- `agents/creative_ideas/`: generates bold, unconventional concepts.
- `agents/conservative_ideas/`: produces safe, low-risk alternatives.
- `scripts/log-notification.sh`: simple helper that appends detached-run notifications to `/tmp/codex-notifications.log`.
- `log/`, `sessions/`: empty placeholders so Codex can write logs and rollouts.

## Quick Start

```bash
# Build the CLI once (from /path/to/repo)
cargo build -p codex-cli

# Launch the TUI against this sample Codex home
CODEX_HOME="$(pwd)/ai-temp/example-codex-home" target/debug/codex

# Launch a specific sub-agent directly (skips the orchestrator)
CODEX_HOME="$(pwd)/ai-temp/example-codex-home" target/debug/codex --agent ideas_provider
CODEX_HOME="$(pwd)/ai-temp/example-codex-home" target/debug/codex --agent critic
CODEX_HOME="$(pwd)/ai-temp/example-codex-home" target/debug/codex --agent creative_ideas
CODEX_HOME="$(pwd)/ai-temp/example-codex-home" target/debug/codex --agent conservative_ideas

# Inside the primary session, describe the task as usual.
# The main assistant decides when to call the `delegate_agent` tool.
# Detached runs surface under /agent and emit desktop/system notifications; dismiss them after review.
# External notifications are logged to /tmp/codex-notifications.log via scripts/log-notification.sh.
# Use tags like `#ideas_provider` in your prompts only as hints for the AI.
# Watch logs in log/codex-tui.log to confirm delegation activity.
```

### Suggested Prompt for the Read-Only Flow

```
We’re assessing a read-only refactor of the parser—no code yet. Ask the ideas provider to explore options, let it consult both the creative and conservative delegates, pick the winning approach, run it by the critic for risks, and then give me the final summary.
```

Unset `CODEX_HOME` (or point it back to your real path) once you're done experimenting.
