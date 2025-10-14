# Sample Codex Home Setup

This directory mirrors a minimal `~/.codex` layout so you can try the multi-agent loader without touching your real config.

## Directory Structure

- `config.toml`: baseline settings used when no sub-agent is selected.
- `AGENTS.md`: default instruction set for the primary agent (orchestrator).
- `agents/ideas_provider/`: GPT-5 sub-agent that proposes multiple approaches.
- `agents/critic/`: GPT-5-nano sub-agent that reviews the leading option.
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

# Inside the primary session, describe the task as usual.
# The main assistant decides when to call the `delegate_agent` tool.
# Use tags like `#ideas_provider` in your prompts only as hints for the AI.
# Watch logs in log/codex-tui.log to confirm delegation activity.
```

### Suggested Prompt for the Read-Only Flow

```
We’re scoping a read-only refactor of the parser—no code yet. Please gather three approaches from the ideas provider, pick the leading option, run it by the critic for risks, and then give me a combined recommendation.
```

Unset `CODEX_HOME` (or point it back to your real path) once you're done experimenting.
