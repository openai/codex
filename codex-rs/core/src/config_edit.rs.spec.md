## Overview
`core::config_edit` provides utilities for mutating `config.toml` while preserving formatting and profile structure. It powers CLI commands that persist overrides (model selection, reasoning effort, nested keys) without clobbering comments or reformatting the entire file.

## Detailed Behavior
- Public helpers:
  - `persist_overrides` writes a list of dotted key segments/values to the active profile (or root when no profile is active).
  - `persist_non_null_overrides` skips entries with `None` values, acting as a no-op when all overrides are unset.
  - `persist_overrides_and_clear_if_none` removes keys when the provided value is `None`.
- Internals:
  - `persist_overrides_with_behavior` handles shared logic: reads/creates `config.toml`, resolves the effective profile (explicit override or existing `profile` key), applies overrides via `apply_toml_edit_override_segments`, optionally removes keys with `remove_toml_edit_segments`, and writes a temp file before atomically persisting to disk.
  - Overrides are specified as explicit segment arrays to avoid ambiguity with profiles containing dots/spaces; helper functions build nested tables as needed and convert scalars into tables when hierarchy changes.
- Tests verify:
  - Values land at the correct scope (top-level vs `[profiles.<name>]`).
  - Profiles with dots/spaces remain quoted properly.
  - Nested tables are created lazily, replacing scalar nodes when necessary.
  - Comments and spacing survive round-trips.

## Broader Context
- `config.rs::persist_model_selection` and CLI commands rely on these helpers to update configuration safely. They complement `Config` loading by ensuring user edits do not break TOML structure or remove managed sections.
- Using `toml_edit` allows fine-grained edits without reserialising the whole document, preserving user comments—a key usability requirement.

## Technical Debt
- None noted in this module; future refactors mentioned in `config.rs` could consolidate persistence logic here.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./config.rs.spec.md
  - ./config_profile.rs.spec.md
