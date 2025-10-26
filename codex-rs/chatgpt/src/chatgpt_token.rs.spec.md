## Overview
`chatgpt_token` loads ChatGPT access tokens from Codex auth storage and caches them for reuse by HTTP helpers.

## Detailed Behavior
- Maintains a global `LazyLock<RwLock<Option<TokenData>>>` (`CHATGPT_TOKEN`) to store the most recent token.
- `get_chatgpt_token_data` clones the cached `TokenData` if present; `set_chatgpt_token_data` swaps in new data under a write lock.
- `init_chatgpt_token_from_auth(codex_home)` reads `auth.json` via `CodexAuth::from_codex_home`, fetches the async `TokenData`, and caches it. Missing auth leaves the cache empty but does not error.

## Broader Context
- Called by `chatgpt_client` before each request to guarantee token availability. Other consumers can use the getters to reuse the cached token.
- Token persistence relies on `codex-core` auth workflows; this module does not handle refresh logic itself.

## Technical Debt
- Cache invalidation is manual; token refresh events must call `set_chatgpt_token_data` explicitly or restart the process to pick up new credentials.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Introduce refresh/expiry handling so stale tokens trigger re-authentication rather than silent HTTP failures.
related_specs:
  - ../mod.spec.md
  - ./chatgpt_client.rs.spec.md
