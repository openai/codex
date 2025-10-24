## Overview
`core::default_client` constructs the shared `reqwest::Client` used for outbound HTTP requests (OpenAI, MCP, etc.). It standardizes headers (originator, User-Agent), supports user-agent suffix overrides, and disables proxies when running inside the seatbelt sandbox.

## Detailed Behavior
- User agent management:
  - `USER_AGENT_SUFFIX` (global `LazyLock<Mutex<_>>`) allows other modules to append context (e.g., MCP client identity) to the UA string.
  - `DEFAULT_ORIGINATOR` and `Originator` encapsulate an identifying header (`originator`) sent with every request; an internal env var (`CODEX_INTERNAL_ORIGINATOR_OVERRIDE`) or `set_default_originator` can override it before initialization.
  - `get_codex_user_agent` builds a UA string with originator, package version, OS/arch info, and terminal UA. `sanitize_user_agent` ensures the result is header-safe, falling back to defaults if needed.
- `create_client` sets default headers (`originator`, sanitized `User-Agent`) and disables proxies when the process is running under seatbelt (`CODEX_SANDBOX_ENV_VAR=seatbelt`). It returns a built client or a fallback `reqwest::Client::new()` if construction fails.
- Tests verify UA formatting and that the default client sends headers as expected.

## Broader Context
- All network calls (OpenAI API, MCP auth, token refresh) reuse this client, ensuring consistent telemetry and easier feature toggling (e.g., originator overrides). Sandbox detection prevents proxy usage in constrained environments.
- The user-agent suffix feature allows MCP clients to differentiate themselves without threading additional context through every request builder.

## Technical Debt
- None noted; global state is intentionally limited (originator once-set, suffix behind a mutex).

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./auth.rs.spec.md
  - ./model_provider_info.rs.spec.md
