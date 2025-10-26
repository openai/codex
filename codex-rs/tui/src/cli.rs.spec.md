## Overview
`tui::cli` defines the command-line options for launching the Codex TUI. It packages user-facing flags (prompt, model, sandbox policy) alongside internal values set by parent commands (`codex resume`, etc.).

## Detailed Behavior
- Derives `clap::Parser` for `Cli`, exposing:
  - Optional prompt text and image attachments for the initial turn.
  - Model selection (`--model`, `--oss`) and configuration profile overrides.
  - Sandbox/approval behavior (`--sandbox`, `--ask-for-approval`, `--full-auto`, the dangerous bypass flag).
  - Working directory (`-C`), web search toggle, and additional writable directories (`--add-dir`).
- Hidden/internal fields (`resume_picker`, `resume_last`, `resume_session_id`) are populated by higher-level wrappers (e.g., `codex resume`), enabling the TUI to switch into resume mode without exposing extra flags on the base command.
- Embeds a `CliConfigOverrides` instance so configuration settings can flow into `codex-core` initialization.

## Broader Context
- Parsed in `src/main.rs` and reused by other binaries/tests when they need to simulate CLI input. The struct feeds directly into `run_main`.

## Technical Debt
- CLI logic remains declarative; future extensions should ensure new flags integrate with the resume invariants and configuration overrides.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./main.rs.spec.md
  - ./tui.rs.spec.md
