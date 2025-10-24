## Overview
`execpolicy::main` provides a CLI for evaluating exec policies. It loads the default or user-specified policy, checks commands (raw or JSON-encoded), and emits machine-readable JSON results with distinct exit codes for unsafe scenarios.

## Detailed Behavior
- CLI (`Args`):
  - `--require-safe`: forces non-zero exit codes when a command is forbidden or unverified.
  - `--policy`: optional path to a `.policy` file; otherwise uses `get_default_policy()`.
  - Subcommands:
    - `check`: treat positional args as a command, splitting the first token as program.
    - `check-json`: parse a JSON object (`{"program": "...", "args": [...]}`) provided via CLI or STDIN.
- Execution flow:
  - Loads policy via `PolicyParser` (reporting Starlark errors through `anyhow`).
  - Builds `ExecCall` from user input.
  - `check_command` runs `Policy::check` and categorizes output:
    - `MatchedExec::Match`: returns `Output::Match` (exit 0 unless `require_safe` and command might write files, in which case exit 12).
    - `MatchedExec::Forbidden`: returns `Output::Forbidden`, exit 14 if `require_safe`.
    - `Err` (policy failure): `Output::Unverified`, exit 13 if `require_safe`.
    - `ValidExec::might_write_files` determines whether `Match` is safe vs needs review.
  - Serializes `Output` to JSON on stdout for tooling.
- Exit codes:
  - 0: safe or (if `require_safe` false) forbidden/unverified.
  - 12: matched but writes files (and `require_safe` true).
  - 13: unverified (and `require_safe` true).
  - 14: forbidden (and `require_safe` true).
- Helper `deserialize_from_json` enables `--exec '{"program":...}'` usage.

## Broader Context
- Developers use this CLI to test policy updates before embedding them. It mirrors the logic used by Codex runtime when approving commands.
- Scripts can rely on exit codes to enforce policy compliance in CI.
- Context can't yet be determined for additional subcommands (e.g., linting); the structure leaves room for future expansion.

## Technical Debt
- JSON output is minimal; adding human-readable summaries could help interactive use.
- Command splitting for `check` is naive (no shell parsing); more accurate tokenization may be desirable.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Enhance `check` input parsing (e.g., respect shell quoting) to avoid misclassifying commands.
    - Offer a human-friendly mode alongside JSON to aid manual testing.
related_specs:
  - ./lib.rs.spec.md
  - ./policy_parser.rs.spec.md
