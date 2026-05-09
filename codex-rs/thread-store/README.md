# Thread Store

`codex-thread-store` is the storage boundary for Codex threads. It defines the
`ThreadStore` trait plus local and in-memory implementations. Other storage
implementations may live outside this repository.

## Responsibilities

- `LiveThread` is the preferred API for active session persistence. It owns a
  `ThreadStore` and serializes live operations.
- `ThreadStore::append_items` is the raw canonical history append API. It does
  not infer metadata from item contents.
- `ThreadStore::apply_thread_metadata` is the explicit metadata API. Local
  storage writes these facts to SQLite; other stores can route them to a
  metadata endpoint independent of rollout item contents.
- `LocalThreadStore` persists history through `codex-rollout` JSONL files and
  persists queryable metadata through the SQLite state database.
- `RolloutRecorder` is the local JSONL writer. It writes already-canonical
  items and does not decide which rollout events should be persisted.
- `core/session` creates or resumes `LiveThread` handles and should not need to
  know whether persistence is backed by local files or a remote store.
- App-server code should use `LiveThread` for loaded threads.

## Direction

New metadata semantics should live above `ThreadStore`. The store is
allowed to persist explicit metadata fields, but it should not derive metadata by
inspecting rollout item payloads.
