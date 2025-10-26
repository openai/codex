## Overview
`auth_fixtures` fabricates ChatGPT authentication artifacts for app-server integration tests, allowing suites to simulate authenticated users without hitting real services.

## Detailed Behavior
- `ChatGptAuthFixture` builder configures access tokens, refresh tokens, account IDs, optional plan type/email claims, and custom `last_refresh` timestamps.
- `ChatGptIdTokenClaims` captures JWT claims relevant to Codex (email and plan type) and offers fluent setters.
- `encode_id_token` constructs a JWT-like token using base64url encoding with a dummy signature, producing an input compatible with `codex_core::token_data::parse_id_token`.
- `write_chatgpt_auth(codex_home, fixture)` serializes the fixture into `AuthDotJson`, writes it to the appropriate `auth.json`, and ensures timestamps default to `Utc::now()` when unspecified.

## Broader Context
- Consumed by login and app-server tests that need to bootstrap a Codex home directory with valid-looking ChatGPT credentials before exercising APIs.
- Complements the ChatGPT CLI specs, which rely on the same auth format.

## Technical Debt
- Fixture writing always populates tokens even if tests want to simulate empty auth; helper could expose toggles for missing token scenarios.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./mcp_process.rs.spec.md
