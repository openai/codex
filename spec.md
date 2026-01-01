# Spec: Token Refresh Resilience Across Concurrent CLI Instances

## Problem statement

If multiple Codex CLI instances are running concurrently, it is possible for one (active) instance to refresh ChatGPT OAuth tokens and persist the rotated `access_token` + `refresh_token` to the shared credential store (`$CODEX_HOME/auth.json` or the OS keyring). An idle instance can later "wake up" with an in-memory copy of the _old_ refresh token; when it receives a 401 it attempts a refresh with the stale refresh token and fails permanently (the refresh token expires ~1 hour after first use, and/or becomes invalid once rotated).

Goal: make refresh behavior resilient to concurrent processes by consulting the shared credential store for updated tokens, while preventing accidental cross-account/workspace credential switching if the user logged out or logged back in to a different account in the meantime.

## Current implementation (as of this repo state)

### Where refresh happens

- API 401 handling happens in `codex-rs/core/src/client.rs` via `handle_unauthorized(...)`.
  - It refreshes once per request when `auth.mode == AuthMode::ChatGPT` by calling `AuthManager::refresh_token()`.

- `AuthManager::refresh_token()` (in `codex-rs/core/src/auth.rs`) does:
  1. `let auth = self.auth()` (cached snapshot)
  2. `auth.refresh_token().await`
  3. on success, `self.reload()` to pick up persisted changes

- `CodexAuth::refresh_token()` (in `codex-rs/core/src/auth.rs`) currently:
  - Reads refresh token from its in-memory `auth_dot_json` cache (`get_current_token_data()`).
  - Calls `try_refresh_token(refresh_token, client)` against `https://auth.openai.com/oauth/token`.
  - Persists updated tokens via `update_tokens(storage, ...)`.
  - Updates the in-memory `auth_dot_json` to the updated value.

Key detail: `AuthManager` is explicitly documented as not observing external modifications until `reload()` is called. Today, the refresh path uses in-memory tokens until the refresh succeeds.

### Credential storage and auth.json schema

- Storage backend abstraction is in `codex-rs/core/src/auth/storage.rs`.
- `cli_auth_credentials_store` supports:
  - `file`: `$CODEX_HOME/auth.json`
  - `keyring`: OS keyring
  - `auto`: keyring when possible, else file

`auth.json` schema is `AuthDotJson`:

```jsonc
{
  "OPENAI_API_KEY": "sk-...", // optional
  "tokens": {
    // optional; present for ChatGPT login
    "id_token": "jwt.header.payload.sig", // serialized as a string; parsed into IdTokenInfo
    "access_token": "jwt-or-opaque",
    "refresh_token": "opaque",
    "account_id": "...", // optional; seems to duplicate chatgpt_account_id claim
  },
  "last_refresh": "2025-01-01T00:00:00Z", // optional
}
```

Identity-related fields available for matching:

- `tokens.id_token` is parsed into `IdTokenInfo` in `codex-rs/core/src/token_data.rs`, which currently extracts:
  - `chatgpt_account_id` (optional; used elsewhere as the "workspace id")

`chatgpt_account_id` is not guaranteed to be present in every token. The login server treats it as optional in general (it logs when the expected `https://api.openai.com/auth` object is missing), and only requires it when `forced_chatgpt_workspace_id` is configured (see `ensure_workspace_allowed` in `codex-rs/login/src/server.rs`).

### Writing logic (file backend)

`FileAuthStorage::save()` writes via `OpenOptions::truncate(true).write(true).create(true)` and then `write_all + flush` to the final path.

This is not atomic: another process reading at the same time can observe a partially-written file and fail to parse JSON.

## Requirements

### Functional

- When a request fails with HTTP 401 and the CLI decides to refresh tokens:
  - It must be able to recover when the in-memory refresh token is stale but the credential store contains rotated tokens written by another instance (by consulting storage before attempting a network refresh with the in-memory token).

- The mechanism must work across all credential store modes (`file`, `keyring`, `auto`) by consulting the configured storage backend, not just `auth.json` directly.

- Token refresh behavior should remain "at most once per request" from the perspective of API retry logic (to avoid infinite refresh loops), but the refresh operation itself may do limited internal retries if it detects concurrent rotation.

### Safety / correctness

- Never silently refresh or retry a request using credentials from a _different_ identity than the one originally used for the request.
  - If the user logged out (credentials deleted), treat the situation as unauthenticated rather than attempting to refresh using stale in-memory tokens.
  - If the user logged in again to a different account/workspace, fail the in-flight request with a clear error and do not attempt to send the retried request under the new identity.

- "Same identity" must include _workspace_ and _account_ constraints. At minimum:
  - Same `chatgpt_account_id` (workspace id) as derived from the `id_token` claims.
  - If required identity fields are unavailable on either side, default to the safest behavior (do not switch).

### UX

- If recovery is possible (tokens rotated by another instance), the user should not see an error; the request should succeed after retry.
- If recovery is not possible due to identity change or logout, surface the existing "logged out"/unauthorized error and do not retry the request under any other identity.

### Observability

- Add structured logs at `debug`/`info` level to indicate when:
  - a refresh used tokens loaded from storage instead of the in-memory snapshot
  - a refresh was aborted due to identity mismatch
  - the storage contents were unreadable transiently (e.g., partial auth.json write)

## Proposed design

### Define a stable "token identity" matcher

Introduce an internal representation used only for comparison (names TBD in code):

- `TokenIdentity { chatgpt_account_id: Option<String> }`

Derivation:

- From in-memory auth: `CodexAuth::get_current_token_data()` -> `id_token.chatgpt_account_id`
- From stored auth: `storage.load()` -> `AuthDotJson.tokens.id_token.chatgpt_account_id`

Match rules:

- If both sides have `chatgpt_account_id`, they must match exactly.
- If either side lacks `chatgpt_account_id`, treat as "cannot safely match" (abort recovery path).

Rationale:

- Workspace id is already used for enforcement (`forced_chatgpt_workspace_id`) and is the most important scoping identifier.

Open question (see below): whether we should parse and compare a more stable subject (`sub`) claim as well.

### Refresh flow changes (high level)

Modify the ChatGPT refresh flow to consult shared storage first (before attempting any network refresh), and to recover from concurrent rotation.

1. **Capture the "expected identity" for the in-flight request**
   - The 401 handler already has access to the `auth` snapshot used for the request (`handle_unauthorized(..., auth: &Option<CodexAuth>)`).
   - Derive `expected_identity` from that snapshot before any reload.

2. **Sync from storage first (no network)**
   - Load current credentials from the configured storage backend (file/keyring/auto).
   - If storage is empty (user logged out): abort refresh and return "unauthenticated" / unauthorized.
   - If storage identity != expected identity: abort refresh and return the existing "logged out"/unauthorized error (do not retry under the new identity).
   - If storage identity matches, treat storage as authoritative:
     - If stored tokens differ from in-memory, update the in-memory `auth_dot_json` cache to the stored snapshot.
     - Retry the request using the (possibly updated) stored `access_token` before doing any network refresh.

3. **Only if still needed, perform network refresh using the stored refresh token**
   - If the retried request still receives a 401 (or other signal that the access token is unusable), call the refresh endpoint with the refresh token taken from the stored snapshot (not the stale in-memory one).
   - On success, persist and update in-memory cache as today.

4. **Handle concurrent rotation during refresh**
   - If refresh returns 401 with `refresh_token_reused` / `refresh_token_expired`:
     - Reload from storage once and re-check identity.
     - If the stored refresh token has changed since the one we attempted, try the refresh once more using the newer stored refresh token.
     - Otherwise, treat as a permanent refresh failure (as today).

5. **Preserve "at most once per request" behavior**
   - The request-level `refreshed` flag in `handle_unauthorized` remains the guard for API retries.
   - Internally we allow a bounded extra refresh attempt only when we can prove rotation occurred via storage changes.

### How we decide "newer" across instances

We do not need a total ordering of tokens to resolve concurrent instances safely. For rotating refresh tokens, the critical property is that once any instance refreshes successfully, the previous refresh token becomes unusable (reused/expired). Therefore:

- If identity matches and `stored.refresh_token != in_memory.refresh_token`, assume the stored refresh token is the current one and adopt it (storage-wins).
- `last_refresh` may be used as an additional heuristic (for logging and as a sanity check), but should not be the only source of truth because it is derived from local wall-clock time and may be missing.

### Handling partial auth.json writes (file backend)

To make "reload from storage" robust when using `file` storage:

- When `storage.load()` fails due to JSON parse errors, treat it as transient and retry a small number of times with a short delay (e.g., 2-3 attempts over ~50-150ms total).

This ensures that one process truncating + writing does not permanently break other processes trying to recover.

## Execution plan (no code yet)

1. Confirm the identity fields we should use:
   - Validate the expected behavior when `chatgpt_account_id` is missing (it is optional today).
   - Ensure the refresh recovery path is explicitly conditional on being able to prove identity matches (i.e., `chatgpt_account_id` present on both the request snapshot and the stored auth).

2. Design the internal API surface:
   - Decide whether the identity-aware refresh logic should live in:
     - `AuthManager::refresh_token(...)` (preferred: keeps the policy near the "single source of truth"), or
     - `CodexAuth::refresh_token(...)` (tighter to token refresh), or
     - `handle_unauthorized(...)` (closest to request retry loop).
   - Ensure we can pass `expected_identity` from the request path into the refresh path.

3. Implement "load from storage and compare identity" helper(s):
   - Load `AuthDotJson` from `AuthStorageBackend`.
   - Extract `TokenIdentity` from stored auth and from the request auth.
   - Provide a single comparison function with explicit "cannot match safely" behavior.

4. Implement refresh fallback logic:
   - Pre-refresh storage adoption.
   - Bounded "reload + retry refresh" on specific 401 reasons.

5. Add tests
   - Unit tests for identity matching behavior (presence/absence combinations).
   - Integration-style test simulating concurrent refresh:
     - Seed auth.json with token A.
     - Update storage to token B (same identity).
     - Ensure a refresh attempt uses token B rather than failing permanently.
   - Test for mismatch case:
     - In-memory identity A, storage identity B -> ensure refresh aborts and request is not retried under B.
   - If feasible, test transient parse error handling by writing partial JSON then completing it.

6. Documentation updates (after implementation is approved)
   - If behavior changes are user-visible (e.g., new error message on identity mismatch), update relevant docs under `docs/` (likely `docs/authentication.md` or `docs/config.md`).

## Decisions (resolved questions)

- Identity definition:
  - Is `chatgpt_account_id` always present in the `id_token` for ChatGPT login?
    Answer: No. It is modeled as optional (`Option<String>`) in `IdTokenInfo` and the login server logs when the expected `https://api.openai.com/auth` claims object is missing. When `forced_chatgpt_workspace_id` is set, login explicitly fails if `chatgpt_account_id` is absent; otherwise, it can be absent and Codex will still persist tokens. Therefore the concurrency-safe "adopt from storage" path must be conditional on having `chatgpt_account_id` available on both sides. If we want this to work even when the claim is missing, we'd need to add and validate an alternative stable identifier (e.g., parse `sub`) in a follow-up.

- Behavior on mismatch:
  - When an in-flight request sees a 401 but storage now belongs to a different identity, should we return a dedicated "credentials changed" error, or map to "logged out" (401) and let the user re-run?
    Answer: Map to the existing "logged out" error.

- Atomic writes:
  - Should we make `FileAuthStorage::save()` atomic (write temp file + rename) to prevent partial reads entirely, rather than adding read-retries?
    Answer: Do not attempt to implement atomic writes as part of this change. Use bounded retries on `storage.load()` parse errors to handle partial reads.

- Interaction with `AuthManager` design goal:
  - The current `AuthManager` explicitly avoids observing external modifications mid-run. Is it acceptable to treat token refresh as a special case where we _do_ consult storage, or should we instead introduce a more explicit "sync from storage" API that call sites opt into?
    Answer: Yes, we can update the semantics of the AuthManager. Update any comments to make the new rules clear.

- Scope:
  - Besides the core API client, do other components cache `TokenData` and perform refresh-like behavior (e.g., `codex-rs/chatgpt/src/chatgpt_token.rs`)? If so, should this spec cover those as well, or is the initial fix limited to the core request path?
    Answer: Do not expand the scope.
