# Saved Session / Fork Design (Closer to `codex resume`)

## Overview

This design keeps as close as possible to the existing `codex resume` behavior, but adds:

- In‑session slash command:
  - `/save <NAME>` – snapshot the current session so it can be resumed/forked later.
- CLI commands:
  - `codex resume ID|NAME` – existing behavior, extended so `<NAME>` aliases a saved session.
  - `codex fork ID|NAME` – new: start a new session as a copy of a saved session or existing id.
  - `codex session list` – list all **saved** sessions (by `<NAME>` and underlying id).

The underlying persistence remains the existing rollout files under `~/.codex/sessions/...`. “Saved sessions” are thin, named pointers to snapshots of those rollouts.

---

## Behavior Summary

- `/save <NAME>` (in-session)
  - Takes the **current state of the conversation** (as if you hit Ctrl‑C and then later did `codex resume <id>`).
  - Writes or updates a saved entry with user‑chosen `<NAME>`.
  - If `<NAME>` already exists, it is **overwritten** to point at the new snapshot (this matches “override the list of sessions” semantics).

- `codex resume ID|NAME` (CLI)
  - Behaves like `codex resume <ID>` today.
  - Additionally, if `ID|NAME` matches a saved `<NAME>`, it is resolved to the underlying saved snapshot and resumed from there.

- `codex fork ID|NAME` (CLI)
  - Like `codex resume`, but starts a **new session** with a **new conversation id**, using the same snapshot as the source.
  - `ID|NAME` can be either:
    - A saved name created via `/save`, or
    - A raw conversation id, in which case the fork is created from that rollout.

- `codex session list`
  - Lists all **saved** sessions:
    - `<NAME>`, underlying conversation id, created_at/saved_at, cwd, model, and rollout path.
  - The list is entirely driven by `/save`; unsaved transient sessions do not appear here.

---

## Data Model

We build entirely on top of the **existing session persistence** (rollout files under `~/.codex/sessions/...`) and add a **name field on the session itself** instead of a separate registry.

### Session name

Conceptually, a session has:

```rust
struct SessionMeta {
    id: ConversationId,   // existing UUID
    // ...
    name: Option<String>, // NEW: user-supplied `<NAME>`, if any
}
```

Concretely:

- We extend the existing `SessionMeta` (the struct written in the first JSONL record of each rollout) with an optional `name: Option<String>`.
- The app server’s `Thread` view can surface this name so the TUI has easy access to it.

**Storage**

- No new files are introduced.
- The session name is persisted **with the session rollout itself**:
  - Either directly in `SessionMeta.name`, or
  - Via a small additional rollout item that updates the name (implementation detail).
- Listing / resolving sessions uses the existing rollout listing mechanisms (`RolloutRecorder::list_conversations`, app‑server `ThreadList`) and simply reads the name field.

Notes:

- **Name uniqueness** is a logical constraint, not a separate registry:
  - Saving with the same `<NAME>` again just updates the name on the current session.
  - For resolution, we treat `<NAME>` as an alias for the “most recent” session with that name (older sessions with the same name effectively fall out of the saved list).
- Underlying rollout files remain in the existing `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` structure; we just annotate them with an optional name.

### Relationship to existing types

- We reuse:
  - `RolloutRecorder` for persistence.
  - `RolloutRecorder::list_conversations` / `find_conversation_path_by_id_str` for discovery.
  - `InitialHistory::Resumed` + `ConversationManager::resume_conversation_from_rollout` and `resume_conversation_with_history` for resuming.
  - The existing “resume by id” flow; `<NAME>` just resolves to the underlying conversation id/path using the stored name.

---

## `/save <NAME>`

### Semantics

- User issues `/save <NAME>` from within a Codex session (TUI).
- Codex:
  - Flushes the current rollout so all events up to this point are durably written (as if the process were about to exit).
  - Records a **saved snapshot** pointing to this rollout file and conversation id.
  - If a saved entry with the same `<NAME>` already exists, it is **replaced**:
    - The session list now reflects only the latest snapshot for that name.

This matches the mental model of “if I hit Ctrl‑C right now, I can later `codex resume <id>` from this point”, just with a human‑friendly name instead of a raw UUID.

### Implementation sketch

TUI:

- Recognize `/save <NAME>` as a local command (do not send it to the model).
- Call a new app‑server RPC, e.g. `session/save`:

```rust
struct SessionSaveParams {
    thread_id: String,   // current thread (maps to ConversationId)
    name: String,        // user-specified `<NAME>`
}
```

App server:

- Resolve `(ConversationId, CodexConversation)` from `thread_id`.
- Ask the underlying session to flush the rollout:
  - e.g. `session.flush_rollout().await` (or equivalent).
- Update the session metadata to set `SessionMeta.name = Some(<NAME>)`, and ensure that change is persisted to the rollout.
  - If another session already has the same name, the system will treat the **most recent** session with that name as canonical when resolving `<NAME>`.

No new persistence format is required beyond adding the `name` field to existing session metadata.

---

## `codex resume ID|NAME`

### Semantics

- `codex resume <ARG>` (CLI) keeps existing behavior, but with an extra resolution step:
  - If `<ARG>` matches a saved `<NAME>` in `saved_sessions.json`, resume from that saved session.
  - Otherwise, treat `<ARG>` as a raw conversation id and use the current resume behavior.

This means:

- Existing workflows (`codex resume <uuid>`, `codex resume --last`) continue to work.
- A user can type `codex resume codex-core` after doing `/save codex-core` in a session.

### Implementation sketch

CLI:

- Extend the existing `codex resume` subcommand resolution:

  1. Try to resolve `<ARG>` as a saved name:
     - Use rollout listing APIs (`RolloutRecorder::list_conversations` or app‑server `ThreadList`) to find sessions whose `SessionMeta.name == <ARG>`, pick the most recent one, and obtain `(conversation_id, rollout_path)`.
  2. If found, resume from that rollout path (see below).
  3. If not found, fall back to the existing id‑based lookup using `find_conversation_path_by_id_str`.

App server:

- The CLI already uses app‑server APIs to resume conversations; the new behavior only needs to:
  - Inject the correct `path` or `conversation_id` into the existing `ResumeConversationParams`, based on the resolution above.
  - Existing logic in `handle_resume_conversation` (which uses `RolloutRecorder::get_rollout_history` and `ConversationManager::resume_conversation_with_history`) remains unchanged.

---

## `codex fork ID|NAME`

### Semantics

- `codex fork <ARG>` (CLI) creates a **new** conversation whose initial state is copied from an existing one:
  - If `<ARG>` is a saved name, resolve it to the saved rollout path and conversation id.
  - If `<ARG>` is a raw id, locate its rollout using `find_conversation_path_by_id_str`.
  - In both cases, a new conversation is spawned with:
    - A **new conversation id**.
    - Initial history loaded from the source rollout.

This is the “explore once, fork many times” workflow driven from the CLI:

- Explore `codex-core` in TUI.
- `/save codex-core`.
- `codex fork codex-core` for “feature A”.
- `codex fork codex-core` again for “feature B`.

### Implementation sketch

CLI:

- Add a new subcommand:

```text
codex fork <ID|NAME>
```

- Resolution:
  1. Try `<ID|NAME>` as a saved name by scanning sessions whose `SessionMeta.name == <ID|NAME>` and picking the most recent one.
  2. If not found, treat it as a raw id and call `find_conversation_path_by_id_str`.
  3. If still not found, print a helpful error.

- Once a `rollout_path` is known, call an app‑server method (or use the existing resume machinery) that:
  - Loads `InitialHistory` via `RolloutRecorder::get_rollout_history(&rollout_path)`.
  - Spawns a new conversation with `ConversationManager::resume_conversation_with_history`.

App server:

- We can either:
  - Add a dedicated `session/fork` RPC that accepts `path` or `conversation_id`, or
  - Reuse the existing `ResumeConversationParams` and add a small wrapper on the server that:
    - Loads history from the given rollout.
    - Spawns a new conversation with that history (instead of attaching to an existing id).

In both cases, the “fork” operation is implemented entirely in terms of existing rollout + `InitialHistory` plumbing; the only new behavior is that it always uses a fresh conversation id.

---

## `codex session list`

### Semantics

`codex session list` prints the list of **saved** sessions, one per line:

- `<NAME>` (user‑chosen)
- Underlying conversation id
- `cwd`
- `model`
- `created_at` and/or `updated_at`

This gives users a quick way to discover what names they can use with `/resume`, `/fork`, or `codex resume`.

### Implementation sketch

CLI:

- Add a new subcommand, conceptually:

```text
codex session list
```

- The CLI loads configuration, locates `codex_home`, then:
  - Uses `RolloutRecorder::list_conversations` (or app‑server `ThreadList`) to iterate over sessions.
  - Filters down to those with a non‑empty `SessionMeta.name`.
  - Prints entries in a compact table format, grouping by name if multiple sessions share the same one (showing only the most recent for each name).

No separate registry file is needed; we derive the list directly from persisted session metadata.

---

## UX Notes & Edge Cases

- **Overwriting names**
  - `/save <NAME>` **always overwrites** any existing saved entry with that name.
  - This matches the “override the list of sessions” intent: the list reflects the latest saved snapshots for each name.

- **Continuing after `/save`**
  - After `/save`, the current conversation continues as normal.
  - The saved entry points at the rollout **as it existed at the time of save**:
    - On resume/fork we read the rollout file; because we flushed on save, the state is consistent with how the session looked at that moment.
    - Additional events recorded after `/save` will also end up in the same rollout file; conceptually this makes the “saved snapshot” move forward in time as you keep using the same session. If we want a truly frozen snapshot, we can later add an option to write to a dedicated copy of the rollout.

- **Name vs id collisions**
  - If a user picks a `<NAME>` that happens to look like a UUID, name resolution still prefers explicit saved entries:
    - First look up saved name.
    - If not found, treat as id.

- **Listing behavior**
  - `codex session list` only shows saved sessions, not all historical rollouts.
  - This keeps the list small and intentional; users explicitly control it via `/save`.

- **Deletion**
  - Initial version can omit deletion; names can be overwritten by re‑saving.
  - A future `codex session delete <NAME>` and/or `/delete-session <NAME>` command could remove entries from `saved_sessions.json`.

---

## Implementation Phasing

1. **Saved registry + resolution**
   - Implement `SavedSessionEntry` and `saved_sessions.json` read/write helpers.
   - Implement name → entry resolution and fallback to id → rollout path using existing `find_conversation_path_by_id_str`.

2. **App server RPCs + TUI slash command**
   - Add `session/save` RPC.
   - Wire `/save <NAME>` in TUI to `session/save`.

3. **CLI integration**
   - Extend `codex resume` to accept `<NAME>` as well as `<ID>`.
   - Add `codex fork <ID|NAME>` that uses the same resolution and history loading, but always spawns a new conversation.
   - Add `codex session list` to show saved sessions.

This keeps the design very close to today’s `codex resume` behavior while adding the ergonomics you described: explicit `/save <NAME>`, `/fork <NAME>`, `/resume <NAME>`, and a simple `codex session list` view. 
