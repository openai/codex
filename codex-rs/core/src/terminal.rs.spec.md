## Overview
`core::terminal` detects the user’s terminal emulator and exposes a sanitized identifier that Codex can send in HTTP headers (e.g., User-Agent suffixes). It caches detection results so repeated lookups are cheap.

## Detailed Behavior
- `TERMINAL` is a `OnceLock<String>` used by `user_agent()` to memoize the detected terminal name for the current process.
- `is_valid_header_value_char` and `sanitize_header_value` ensure the identifier contains only header-safe characters (`[A-Za-z0-9._/-]`), replacing anything else with underscores.
- `detect_terminal` inspects environment variables in priority order:
  - Prefers `TERM_PROGRAM` and `TERM_PROGRAM_VERSION` (common on macOS).
  - Falls back to emulator-specific hints (WezTerm, kitty, Alacritty, Konsole, GNOME Terminal, VTE, Windows Terminal).
  - Defaults to `TERM` or `unknown` when no specialized environment variables are present.
- Returns values like `WezTerm/<version>` or `TerminalName` as appropriate, always sanitized before caching.

## Broader Context
- Contributes to the client user agent composition (`./client.rs.spec.md`, `./user_notification.rs.spec.md`) so backend logging can attribute requests to specific terminals.
- Helps downstream telemetry (`./otel_init.rs.spec.md`) distinguish terminal sessions without leaking unsanitized environment data.

## Technical Debt
- None observed; detection logic is easily extensible for additional terminals.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./client.rs.spec.md
  - ./user_notification.rs.spec.md
  - ./otel_init.rs.spec.md
