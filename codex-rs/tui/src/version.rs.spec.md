## Overview
Exposes the Codex CLI version compiled into the binary via Cargo metadata.

## Detailed Behavior
- `CODEX_CLI_VERSION` is a `&'static str` set to `env!("CARGO_PKG_VERSION")`, making the build’s package version available to runtime code (e.g., `/status` card, update prompts).

## Broader Context
- Used in status cards and UI banners to report the running version and compare it against latest releases.

## Technical Debt
- Depends on Cargo build metadata; if the build pipeline overrides or scrubs `CARGO_PKG_VERSION`, the constant will need an alternative source.

---
tech_debt:
  severity: low
  highest_priority_items:
    - None.
related_specs:
  - status/card.rs.spec.md
  - update_prompt.rs.spec.md
