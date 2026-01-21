# OAuth Rotation & Multi-Account Auth (Design Doc)

## Summary
Add multi-account ChatGPT OAuth storage and automatic credential rotation for OpenAI requests. Persist multiple OAuth accounts in a versioned auth store, expose CLI workflows to list/remove accounts, and add a rotation strategy that handles rate limits and auth failures without disrupting API-key usage.

## Goals
- Store multiple ChatGPT OAuth accounts safely and deterministically.
- Rotate OAuth credentials automatically on 429/401/403 and certain failures.
- Keep API key login independent (API key stays even when OAuth accounts change).
- Provide CLI commands to list accounts and remove one or all accounts.
- Preserve compatibility with legacy `auth.json` format.

## Non-goals
- UI/TUI account switching or selection UI (CLI only for now).
- Cross-provider rotation (non-OpenAI providers remain unchanged).
- Automatic removal of accounts without explicit user action.

## Current Behavior (Baseline)
- `auth.json` stores a single OAuth token set plus optional `OPENAI_API_KEY`.
- Login overwrites the entire file.
- Requests use one ChatGPT account; failures lead to a local refresh/retry loop.

## Proposed Changes

### 1) Versioned Auth Store (V2)
Introduce a versioned auth store schema and migrate legacy auth data on load.

**Auth store structure (conceptual):**
```
{
  "version": 2,
  "OPENAI_API_KEY": "sk-...",
  "providers": {
    "openai": {
      "type": "oauth",
      "active": { "default": "<record-id>" },
      "order": { "default": ["<record-id>", "..."] },
      "records": [
        {
          "id": "<record-id>",
          "namespace": "default",
          "label": "user@example.com",
          "tokens": { ... },
          "last_refresh": "...",
          "created_at": "...",
          "updated_at": "...",
          "health": {
            "cooldown_until": "...",
            "last_status_code": 429,
            "last_error_at": "...",
            "success_count": 3,
            "failure_count": 1
          }
        }
      ]
    }
  }
}
```

**Migration rules:**
- If `auth.json` lacks `version`, parse legacy shape and lift tokens into a single OAuth record.
- Preserve `OPENAI_API_KEY`.
- Seed `active` + `order` for the default namespace.

### 2) Multi-account Auth API
Expose a minimal API to manage records:
- `add_oauth_account(...) -> record_id`
- `list_oauth_accounts(...) -> Vec<OAuthAccountSummary>`
- `remove_oauth_account(...) -> bool`
- `remove_all_oauth_accounts(...) -> bool`
- `set_openai_api_key(...)` (decoupled from OAuth storage)

These are storage-level primitives used by CLI and rotation.

### 3) CLI UX
Add account visibility and removal without breaking existing commands.

**New/updated commands:**
- `codex login accounts` (alias `list`): list stored ChatGPT accounts (id, email/label, last refresh, cooldown, active marker).
- `codex logout --account <id|email|label>`: remove a single OAuth account.
- `codex logout --all-accounts`: remove all OAuth accounts but keep API key.
- `codex logout` (no flags): keep existing behavior (remove all credentials).

### 4) OAuth Rotation Algorithm
Rotate only for OpenAI providers + ChatGPT auth when multiple accounts exist.

**Candidate selection:**
- Use `order[namespace]`.
- Skip records whose `cooldown_until` is in the future.
- If all are cooled down, fall back to the next candidate (best effort).

**Failure handling:**
- **429**: set cooldown based on `Retry-After` (seconds or HTTP-date); move account to back; try next.
- **401/403**: refresh once; if refresh succeeds, retry same account once; on refresh failure or repeat failure, cooldown + rotate.
- **Other HTTP errors**: mark failure, rotate.
- **Network/timeout/build**: retry same account up to `network_retry_attempts`; do not rotate on network errors.

**Success handling:**
- Clear cooldown; increment success count.

### 5) Config
New optional config section:
```
[oauth_rotation]
rate_limit_cooldown_ms = 30000   # default 30s
auth_failure_cooldown_ms = 300000 # default 5m
network_retry_attempts = 1
max_attempts = <candidate_count> # default = all candidates
```

### 6) Observability
Persist per-account health in auth store:
- `cooldown_until`
- `last_status_code`
- `last_error_at`
- `success_count` / `failure_count`

## Implementation Plan
1. **Auth storage V2**
   - Add `AuthStore`, `OAuthProvider`, `OAuthRecord`, `OAuthHealth`.
   - Implement migration from legacy `AuthDotJson`.
   - Update read/write paths for file/keyring/auto stores.
2. **Auth management APIs**
   - Add `add/list/remove` functions.
   - Keep `set_openai_api_key` independent of OAuth records.
3. **AuthManager extensions**
   - Provide `oauth_snapshot`, `auth_for_record`, `oauth_record_outcome`, `oauth_move_to_back`, `refresh_record`.
4. **Client rotation**
   - Implement `OAuthRotationPlan` in `ModelClient`.
   - Wire into Responses/Websocket/Compact flows (ChatGPT auth only).
5. **CLI updates**
   - Add `codex login accounts`.
   - Extend `codex logout` with `--account` and `--all-accounts`.
6. **Config & schema**
   - Add `OAuthRotationConfig` to config structs.
   - Regenerate `config.schema.json`.
7. **Tests**
   - Rotation integration tests (429, 401 refresh, refresh failure, non-auth errors, Retry-After date).
   - Auth storage regression tests (migration + add/remove/list).
   - Login server E2E test updated for new storage format.

## Test Plan (Detailed)
- **OAuth rotation:**
  - 429 → cooldown + rotate to next account.
  - 401 → refresh → retry same account.
  - 401 + refresh failure → rotate.
  - Non-auth HTTP error (e.g. 402) → rotate.
  - Retry-After HTTP-date parsing → cooldown near date.
  - Network error → retry same account (no rotation).
- **Storage:**
  - Legacy auth.json migration to V2.
  - add/list/remove all account order & active selection.
- **CLI (manual):**
  - `codex login accounts` shows stored list.
  - `codex logout --account <id>` removes one account only.
  - `codex logout --all-accounts` removes OAuth accounts but keeps API key.
  - `codex logout` removes all credentials.

## Risks & Mitigations
- **Stale concurrent writes:** file lock with best-effort updates for health/order.
- **Ambiguous account selection:** `--account` accepts id/email/label; require id if ambiguous.
- **Over-rotation on transient network errors:** network retries are confined to same account.
