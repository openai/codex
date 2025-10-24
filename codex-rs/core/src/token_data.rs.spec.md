## Overview
`core::token_data` parses and stores authentication tokens. It extracts useful claims from ChatGPT/OpenAI JWTs, tracks refresh/access tokens, and exposes plan metadata for UI and policy enforcement.

## Detailed Behavior
- `TokenData` stores:
  - `IdTokenInfo` (parsed JWT claims), access token, refresh token, and optional `account_id`.
  - Serde hooks serialize/deserialize `IdTokenInfo` via custom functions so the original raw JWT is preserved.
- `IdTokenInfo` captures email, ChatGPT plan type (via `PlanType`), workspace ID, and the raw JWT string. `get_chatgpt_plan_type` returns a user-friendly string.
- Parsing:
  - `parse_id_token` ensures the JWT has three segments, decodes the payload via base64 URL-safe, and deserializes into `IdClaims`/`AuthClaims`. Missing claims are tolerated.
  - Errors (`IdTokenInfoError`) distinguish invalid formats, base64 issues, and JSON errors, aiding diagnostics.
- `PlanType` supports known plan enumerations (`Free`, `Plus`, etc.) and an `Unknown` string fallback.
- Tests verify parsing with/without optional fields and ensure pretty plan names (e.g., `PlanType::Known(KnownPlan::Pro)`) map to `Some("Pro")`.

## Broader Context
- `auth.rs` uses `TokenData` when refreshing tokens, enforcing login restrictions, and exposing account metadata. Plan types inform workspace restrictions and potential UI messaging.
- Keeping token parsing isolated simplifies future schema updates; new claims can be added without touching authentication flows.

## Technical Debt
- None noted; functionality is focused and well-tested.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./auth.rs.spec.md
