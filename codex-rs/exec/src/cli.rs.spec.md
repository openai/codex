## Overview
`exec::cli` defines the command-line interface for `codex-exec`. It uses `clap` to capture non-interactive execution options, including prompt sources, sandbox policies, OSS shortcuts, resume controls, and output formatting flags.

## Detailed Behavior
- `Cli` (derive `Parser`):
  - Subcommand support: `Command::Resume` to reopen existing sessions.
  - Prompt handling: positional argument or `-` to read from stdin; images can be attached via repeated `--image`.
  - Model selection: `--model`, `--oss` (forces OSS provider/model and toggles raw reasoning output).
  - Sandbox controls: `--sandbox` (`SandboxModeCliArg`), `--full-auto` (workspace-write + no approvals), `--dangerously-bypass-approvals-and-sandbox` (disables protections), `--skip-git-repo-check`, and `--cd`.
  - Structured output: `--output-schema` path, `--json` for JSONL mode, `--output-last-message` to capture final assistant message.
  - Config integration: `config_profile`, flattened `CliConfigOverrides` (populated externally), color selection (`Color` enum), and raw override passthrough in `config_overrides`.
  - Prompt argument is optional; omission triggers stdin reading logic in `run_main`.
- Subcommands:
  - `ResumeArgs` accept an optional session ID, `--last` shortcut, and optional prompt (stdin when `-`).
- `Color` enum maps to `--color` with `always|never|auto`, informing ANSI enablement downstream.

## Broader Context
- `run_main` converts these options into `ConfigOverrides`, event processors, and conversation workflow. Specs for `exec::lib` describe how each flag influences runtime behavior (OSS bootstrap, sandbox selection, etc.).
- Shared override handling aligns with `codex-common` CLI tooling; modifications here should mirror the broader CLI ecosystem.

## Technical Debt
- None observed; the module is declarative and relies on `clap` for validation.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./lib.rs.spec.md
  - ../core/src/config.rs.spec.md
