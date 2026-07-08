# Thread Store

`codex-thread-store` is the storage boundary for Codex threads. It defines the
`ThreadStore` trait plus local and in-memory implementations. Other storage
implementations may live outside this repository.

## Responsibilities

- `ThreadStore::append_items` is the raw history append API. Implementations
  apply the rollout persistence policy and may derive implementation-owned
  metadata from canonical items.
- `ThreadStore::update_thread_metadata` applies literal metadata patches for
  explicit user or API mutations.
- `LiveThread` is the preferred API for active session persistence. It forwards
  history and explicit metadata operations without deriving storage metadata.
- `ThreadManager` routes metadata mutations for loaded and cold threads through
  one entrypoint. Loaded threads use their `LiveThread`; cold threads go
  directly to the store.
- `LocalThreadStore` persists history through `codex-rollout` JSONL files and
  derives the queryable metadata needed by its SQLite state database when
  available. Local explicit metadata mutations also maintain JSONL/name-index
  compatibility so reading old or SQLite-less local storage keeps working.
- `RolloutRecorder` is the local JSONL writer. It writes already-canonical
  items for `ThreadStore::append_items`; the local store owns metadata updates
  around that writer.
- `core/session` creates or resumes `LiveThread` handles and does not need to
  know whether persistence is backed by local files or another store.

## Direction

Each store owns any metadata it derives from appended history. Callers use
`update_thread_metadata` only for explicit metadata mutations.
