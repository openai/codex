# Sample Codex Home Setup

This directory mirrors a minimal `~/.codex` layout so you can try the new multi-agent loader without touching your real config.

## Directory Structure

- `config.toml`: baseline settings used when no sub-agent is selected.
- `AGENTS.md`: default instruction set for the primary agent.
- `agents/rust_test_writer`: sub-agent focused on Rust testing.
- `agents/test_driver`: sub-agent that keeps sandboxing strict while running checks.
- `log/`, `sessions/`: empty placeholders so Codex can write logs and rollouts.

## Quick Start

```bash
export CODEX_HOME="$(pwd)/ai-temp/example-codex-home"

# Primary agent (uses AGENTS.md + config.toml in this directory)
codex --help

# Rust-focused sub-agent
codex --agent rust_test_writer

# Test driver sub-agent with read-only sandbox
codex --agent test_driver
```

Unset `CODEX_HOME` (or point it back to your real path) once you're done experimenting.
