## Overview
`oauth.rs` manages storage and retrieval of MCP OAuth tokens. It abstracts over OS keyrings and file-based fallbacks, serializes token responses, and exposes helpers used by the MCP client and CLI flows.

## Detailed Behavior
- Data model:
  - `StoredOAuthTokens` captures server metadata and the serialized `OAuthTokenResponse` (`WrappedOAuthTokenResponse`) along with the client id.
  - `OAuthCredentialsStoreMode` lets callers choose between `Auto`, `File`, or `Keyring` persistence.
- Keyring integration:
  - `KeyringStore` trait abstracts keyring operations; `DefaultKeyringStore` implements it using the `keyring` crate.
  - `load_oauth_tokens_from_keyring` / `save` / `delete` operate on hashed service/URL identifiers (`compute_store_key`).
- File fallback:
  - `load_oauth_tokens_from_file`, `save_oauth_tokens_to_file`, `delete_oauth_tokens_from_file` store credentials in `CODEX_HOME/.credentials.json` using `find_codex_home`.
- Public helpers:
  - `load_oauth_tokens`, `has_oauth_tokens`, `save_oauth_tokens`, `delete_oauth_tokens` choose the appropriate backend based on `OAuthCredentialsStoreMode::Auto`.
- Token lifecycle:
  - `OAuthPersistor` wraps an async `AuthorizationManager`, tracks refresh state, and persists tokens only when updated (`persist_if_needed`).
  - Tokens are hashed with SHA-256 for keyring keys; file storage keeps a map keyed by the hashed identifier.
- Handles keyring failures gracefully, logging warnings and falling back to file-based storage.

## Broader Context
- Used by `RmcpClient` during OAuth transport initialization, by `perform_oauth_login` after completing the browser flow, and by auth status checks to determine whether credentials are present.

## Technical Debt
- TODO in file suggests moving `find_codex_home` to a shared crate to eliminate the circular dependency; tracked elsewhere.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Relocate `find_codex_home` into a shared lower-level crate to avoid duplication.
related_specs:
  - ../mod.spec.md
  - ./perform_oauth_login.rs.spec.md
  - ./auth_status.rs.spec.md
