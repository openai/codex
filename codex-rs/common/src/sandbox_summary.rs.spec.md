## Overview
`common::sandbox_summary` exposes `summarize_sandbox_policy`, a helper that distills a `codex_core::protocol::SandboxPolicy` into a concise string. User interfaces use the summary to display the current sandbox posture alongside configuration data or preset choices without reimplementing formatting rules.

## Detailed Behavior
- Returns fixed strings for simple policies: `"danger-full-access"` for unrestricted execution and `"read-only"` for the most restrictive preset.
- For `SandboxPolicy::WorkspaceWrite`, starts with the label `"workspace-write"` and appends a bracketed, comma-separated list of writable locations. The list always includes `workdir` and conditionally includes `/tmp`, `$TMPDIR`, and each entry from `writable_roots`, formatting paths with `to_string_lossy` to avoid panics on non-UTF-8 data.
- Appends `" (network access enabled)"` whenever the policy grants network access, keeping the summary readable while calling out the elevated capability.
- Builds the summary eagerly using owned `String` values to avoid lifetimes that would complicate UI consumption.

## Broader Context
- Ensures consistent wording across CLI summaries, preset pickers, and any future dashboards. Consumers that need localized or structured output should wrap this helper rather than diverge from the shared messaging.
- The list of implied writable locations mirrors the runtime enforcement in `codex-core` and related sandbox crates; any changes to those policies require updating the helper to keep the summaries accurate.
- Context can't yet be determined for platform-specific nuances (e.g., additional writable directories on Windows) until sandbox policy generation modules document their behavior.

## Technical Debt
- None observed; function mirrors the corresponding sandbox variants and formats information predictably.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./approval_presets.rs.spec.md
