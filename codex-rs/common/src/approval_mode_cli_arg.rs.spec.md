## Overview
`common::approval_mode_cli_arg` defines `ApprovalModeCliArg`, a Clap enum that standardizes the `--approval-mode` flag across Codex CLI tools. It enumerates the supported approval policies and converts selections into `codex_core::protocol::AskForApproval` values for downstream execution logic.

## Detailed Behavior
- Derives `ValueEnum` so Clap accepts kebab-case variants (`untrusted`, `on-failure`, `on-request`, `never`) and surfaces them in help output automatically.
- Documents each variant inline to clarify how Codex will request user approval, matching the semantics in `AskForApproval`.
- Implements `From<ApprovalModeCliArg> for AskForApproval` with a straightforward match to bridge user input to core policy handling.
- Marked as `Clone + Copy + Debug` to simplify reuse in CLI argument structures and logging.

## Broader Context
- Ensures all binaries expose consistent naming and behavior for approval modes, reducing duplication and the risk of diverging help text.
- Downstream command execution modules rely on `AskForApproval` semantics; keeping this conversion centralized prevents accidental drift if new variants are introduced.
- Context can't yet be determined for enterprise or experimental approval modes; new variants should extend this enum when the policy is ready for general use.

## Technical Debt
- None observed; the enum mirrors `AskForApproval` exactly and provides clear documentation.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./approval_presets.rs.spec.md
