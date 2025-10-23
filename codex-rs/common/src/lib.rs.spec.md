## Overview
`common::lib` defines the public face of the `codex-common` crate. It exposes shared helpers for CLI configuration, sandbox summaries, fuzzy matching, and model approval presets. The module primarily re-exports feature-gated submodules so downstream crates (CLI, TUI, MCP server) can pull these utilities through a single interface.

## Detailed Behavior
- Conditionally compiles CLI-centric helpers behind the `cli` feature, including `ApprovalModeCliArg`, `SandboxModeCliArg`, configuration override plumbing, and environment formatting utilities.
- Exposes elapsed-time tracking when the `elapsed` feature is enabled, ensuring lightweight builds can omit the dependency.
- Re-exports sandbox summary generation (feature gate `sandbox_summary`) so callers can request policy text without knowing the underlying module path.
- Publishes configuration summary construction via `create_config_summary_entries`, along with always-on helpers for fuzzy matching and shared preset catalogs (`model_presets`, `approval_presets`).
- The file itself owns no business logic; it stitches together submodules declared in `common/src`. Each referenced file will receive its own `*.spec.md` (e.g., `config_summary.rs.spec.md`, `model_presets.rs.spec.md`) as the documentation effort progresses.

## Broader Context
- Downstream consumers rely on the feature flags declared here to tailor binary footprints. Coordinating feature usage across `codex-cli`, `codex-tui`, and `codex-mcp-server` will be covered in their respective specs once written.
- Shared presets and summaries influence user-facing configuration flows; this spec will link to the crate-level overview once `../mod.spec.md` is authored.
- Context can't yet be determined for how `sandbox_summary` interacts with seatbelt policies until the dedicated spec captures the policy format and regeneration steps.

## Technical Debt
- Commented note recommends renaming the `AskForApproval` preset to `EscalationPolicy`; the rename remains outstanding and should be tracked when editing the preset exports.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Rename the `AskForApproval` preset to `EscalationPolicy` to match the inline guidance.
related_specs:
  - ../mod.spec.md
  - ./config_summary.rs.spec.md
  - ./approval_presets.rs.spec.md
  - ./model_presets.rs.spec.md
  - ./sandbox_summary.rs.spec.md
  - ./sandbox_mode_cli_arg.rs.spec.md
  - ./approval_mode_cli_arg.rs.spec.md
  - ./format_env_display.rs.spec.md
  - ./elapsed.rs.spec.md
