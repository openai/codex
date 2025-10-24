## Overview
`core::config_loader::macos` handles managed preferences on macOS. It reads a base64-encoded TOML string inserted via MDM profiles (at `com.openai.codex` / `config_toml_base64`), decodes it, and returns the resulting `TomlValue` overlay.

## Detailed Behavior
- `load_managed_admin_config_layer`:
  - When a base64 override string is provided (e.g., tests or CLI), trims it and either returns `None` (empty) or immediately decodes it via `parse_managed_preferences_base64`.
  - Otherwise, spawns a blocking task to call `load_managed_admin_config`, capturing errors and converting them into `io::Error`.
- `load_managed_admin_config` invokes `CFPreferencesCopyAppValue` (CoreFoundation) for the managed preferences key. If present, it treats the content as base64, trims whitespace, and delegates to `parse_managed_preferences_base64`.
- `parse_managed_preferences_base64`:
  - Decodes base64 bytes, ensures valid UTF-8, parses TOML, and verifies the root is a table.
  - Logs parsing failures and returns descriptive `io::Error`s so callers can surface configuration issues.
- On non-macOS platforms, `load_managed_admin_config_layer` is a stub returning `Ok(None)`.

## Broader Context
- Managed preferences act as the highest-priority config layer, enabling administrators to push enforced settings via device management. The decoded TOML is merged by `config_loader::apply_managed_layers`.
- The async boundary keeps CoreFoundation calls off the Tokio runtime threads.

## Technical Debt
- None noted; the implementation is platform specific and already guards against malformed payloads.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./mod.rs.spec.md
