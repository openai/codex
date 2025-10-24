## Overview
`common::approval_presets` defines `ApprovalPreset`, a lightweight data structure that pairs approval and sandbox policies, and exposes `builtin_approval_presets` to return the curated defaults. UI layers use these presets to power quick-pick menus that keep policy combinations consistent across the product.

## Detailed Behavior
- `ApprovalPreset` stores an identifier, display label, description, and the paired `AskForApproval` and `SandboxPolicy` values. All fields are `'static` or copy types to keep the preset list `const`.
- `builtin_approval_presets` returns a `Vec<ApprovalPreset>` containing three built-in options:
  - `read-only`: combines on-request approval with the read-only sandbox.
  - `auto`: keeps approval on-request but expands the sandbox to workspace-write with default writable roots.
  - `full-access`: disables approval prompts and grants the danger-full-access sandbox for unrestricted operation.
- The function constructs the vector inline, relying on `SandboxPolicy::new_workspace_write_policy()` for the default workspace-write configuration to ensure parity with sandbox initialization elsewhere in the codebase.
- Descriptions are written in user-facing tone and reused by both CLI and TUI surfaces; any updates should account for translation consistency across interfaces.

## Broader Context
- Acts as the single source of truth for approval/sandbox combinations surfaced in onboarding flows and quick switches. Downstream specs for CLI/TUI should reference these presets rather than duplicating text.
- Ties directly into `codex-core`’s policy evaluation: changing preset defaults affects runtime authorization behavior, so reviewing changes with policy owners is essential.
- Context can't yet be determined for enterprise-specific presets or dynamic policy loading; this module will need extension if those requirements emerge.

## Technical Debt
- None observed; presets map cleanly to existing policies and rely on `codex-core` constructors.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./sandbox_summary.rs.spec.md
  - ./approval_mode_cli_arg.rs.spec.md
