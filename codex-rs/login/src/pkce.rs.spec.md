## Overview
`pkce` generates verifier/challenge pairs used by both browser and device-code login flows to satisfy OAuth PKCE requirements.

## Detailed Behavior
- `generate_pkce()` produces 64 random bytes, encodes them with URL-safe base64 (no padding) to form a valid PKCE verifier (43–128 chars).
- Computes the SHA-256 digest of the verifier and URL-safe encodes it to produce the S256 code challenge.
- Returns a `PkceCodes` struct bundling the verifier and challenge so callers can persist the pair through redirect/authorization steps.

## Broader Context
- Used by `server::run_login_server` and `device_code_auth::run_device_code_login` when constructing OAuth authorization URLs and subsequent token exchanges.

## Technical Debt
- None; implementation follows RFC 7636 guidance and relies on `rand::rng()` and `sha2::Sha256`.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./server.rs.spec.md
  - ./device_code_auth.rs.spec.md
