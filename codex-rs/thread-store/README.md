# Thread Store

`codex-thread-store` is the storage boundary for Codex threads. It defines the
`ThreadStore` trait plus local and in-memory implementations. Other storage
implementations may live outside this repository.

## Responsibilities

- `ThreadStore::append_items` is the raw canonical history append API. It does
  not infer metadata from item contents.
- `ThreadStore::update_thread_metadata` is the only thread metadata write API.
  It accepts a single literal metadata patch shape, regardless of whether the
  caller is applying a user/API mutation or facts derived above the store from
  appended history.
- `LiveThread` is the preferred API for active session persistence. It owns a
  per-thread metadata sync helper, applies the rollout persistence policy,
  appends canonical history, and then sends metadata patches through
  `ThreadStore::update_thread_metadata`.
- `ThreadManager` routes metadata mutations for loaded and cold threads through
  one entrypoint. Loaded threads use their `LiveThread`; cold threads go
  directly to the store.
- `LocalThreadStore` persists history through `codex-rollout` JSONL files and
  persists queryable metadata through the SQLite state database when available.
  Local explicit metadata mutations also maintain JSONL/name-index compatibility
  so reading old or SQLite-less local storage keeps working.
- `RolloutRecorder` is the local JSONL writer. It writes already-canonical
  items for `ThreadStore::append_items`; it no longer decides metadata updates
  for live thread-store appends.
- `core/session` creates or resumes `LiveThread` handles and does not need to
  know whether persistence is backed by local files or another store.

## Direction

New metadata observation semantics should live above `ThreadStore`. Stores
persist explicit metadata fields, but raw history appends remain history-only.
