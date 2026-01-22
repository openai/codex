# External Events for Codex Sessions (Spec)

## Summary

Codex sessions today only learn about “what changed” when Codex itself runs a command/tool
(tests, git, web search, etc.) or when the user types new input.
This spec adds an **External Events**
channel: background systems (CI/builds, deploys, code review, long-running scripts, other agents)
can publish events into the _currently running_ Codex session so Codex can:

- Show timely notifications in the UI (TUI/IDE).
- Optionally incorporate event information into the agent’s context.
- Optionally steer an in-flight turn when appropriate.
- Optionally trigger follow-on “workflows” (e.g., release after build completes).

The design prioritizes **safety (prompt-injection resistance)**, **simplicity (local-first)**, and
**portability (macOS/Linux/Windows)**.

---

## Hackathon MVP (2 hours)

If the goal is “make Codex aware of outside happenings in the current session”,
the smallest useful
thing is a **local event inbox** that Codex can tail and immediately enqueue into the thread’s
context.

Implementation checklist: `docs/hackathon-external-events-plan.md`.

**MVP constraints**:

- One transport: **per-thread file inbox** (`*.jsonl`), append-only.
- Explicit destination: every event targets a specific `thread_id`.
- Mid-turn behavior preference: if Steer is enabled, steer immediately; otherwise queue and apply
  on the next model call.
- Persisted transcript: emit a distinct, persisted history item so events are visible on resume/replay.
- Minimal schema: `{ schema_version, event_id, time_unix_ms, type, severity, title, summary,
  payload? }`.
- Security posture: local filesystem permissions only; treat all event text as untrusted data.

This MVP is enough to:

- Make external events part of the thread context without user copy/paste.
- Persist external events into the transcript so they show up in the TUI and on resume/replay.
- Queue events that arrive mid-turn and automatically fold them into the next model call.
- Enable “Codex-to-Codex” messaging by having one agent append events into another agent’s
  per-thread inbox (local or via `ssh`).

### Worked example 1: background test run reports failure into the thread

Setup:

1. You have Codex running in thread `thr_123`.
2. A separate terminal runs tests in the same repo.
3. A tiny wrapper writes a `build.status` event into
   `~/.codex/sessions/thr_123/external_events.inbox.jsonl` when the command finishes.

Wrapper sketch (write directly):

```bash
#!/usr/bin/env bash
set -euo pipefail

thread_id="thr_123"
inbox="$HOME/.codex/sessions/$thread_id/external_events.inbox.jsonl"

start_ms="$(python -c 'import time; print(int(time.time()*1000))')"
printf '%s\n' \
  '{"schema_version":1,"event_id":"evt_test_started","time_unix_ms":'"$start_ms"',\
"type":"build.status","severity":"info","title":"tests started","summary":"cargo test -p foo"}' \
  >>"$inbox"

if cargo test -p foo; then
  sev="info"
  title="tests passed"
  summary="cargo test -p foo succeeded"
else
  sev="error"
  title="tests failed"
  summary="cargo test -p foo failed (see terminal for logs)"
fi

end_ms="$(python -c 'import time; print(int(time.time()*1000))')"
printf '%s\n' \
  '{"schema_version":1,"event_id":"evt_test_done","time_unix_ms":'"$end_ms"',\
"type":"build.status","severity":"'"$sev"'","title":"'"$title"'","summary":"'"$summary"'"}' \
  >>"$inbox"
```

Result:

- If Steer is enabled and the agent is mid-turn, Codex can append a compact “tests failed”
  context
  to the in-flight turn immediately.
- Otherwise the failure is queued and automatically prepended to the next model call, so the next
  assistant response can proactively pivot to fixing the failure.

### Worked example 2: Codex-to-Codex “worker” sends findings to the main thread

Goal: run a second Codex instance (local or remote) with a narrow prompt (the “worker”),
then feed
its findings into the “main” Codex thread as soon as they’re produced.

Flow:

1. Main Codex is running thread `thr_main`.
2. Worker Codex runs with a task like “scan CI logs and summarize the root cause; output JSON”.
3. A small forwarder takes the worker’s output and appends an `agent.message` event to the main
   thread inbox.

Minimal event line:

```jsonc
{
  "schema_version": 1,
  "event_id": "evt_worker_1",
  "time_unix_ms": 1730831111000,
  "type": "agent.message",
  "severity": "info",
  "title": "worker: likely root cause",
  "summary": "Windows failure caused by path separator; fix normalize_path() in codex-rs/…",
  "payload": {
    "from": { "name": "codex-worker-1" },
    "refs": [{ "kind": "path", "value": "codex-rs/tui/src/…" }]
  }
}
```

Result:

- If Steer is enabled, the main agent can be steered mid-turn with “FYI from worker” and adjust
  its plan without waiting for the current turn to finish.
- If Steer is not enabled, the message is queued and becomes visible to the model on the very next
  call, which still avoids human copy/paste and supports async multi-agent collaboration.

### Worked example 3: cross-repo coordination (CLI repo ↔ docs website repo)

Goal: run two Codex instances in two different repos and let them coordinate asynchronously:

- Repo A: Codex CLI / product code (`thr_code`)
- Repo B: developer docs website (`thr_docs`)

Flow:

1. `thr_code` makes an API/behavior change (flags, CLI output, config keys).
2. A small hook produces a compact “docs-impact summary” and appends an event into `thr_docs`:
   `~/.codex/sessions/thr_docs/external_events.inbox.jsonl`.
3. `thr_docs` sees the event on the next model call (or immediately steers if Steer is enabled)
   and updates the relevant docs pages.

Practical “docs-impact summary” sources (keep them short):

- `jj diff --from 'trunk()' --to '@' --no-pager --git` (or `git diff`) + a tiny diffstat
- list of changed config keys / flags / commands
- a single “what changed + why + how to use it” paragraph

Example event:

```jsonc
{
  "schema_version": 1,
  "event_id": "evt_docs_sync_1",
  "time_unix_ms": 1730831111000,
  "type": "repo.change",
  "severity": "info",
  "title": "CLI change needs docs update",
  "summary": "Added --foo; changed default config.bar from X to Y",
  "payload": {
    "from_repo": "codex-cli",
    "refs": [
      { "kind": "path", "value": "codex-rs/cli/src/…" },
      { "kind": "path", "value": "docs/config.md" }
    ]
  }
}
```

Why this works well:

- It keeps the docs agent aligned with code changes without human copy/paste across repos.
- The message is scoped and structured, so it’s useful context without flooding the model.
- It generalizes across machines: `thr_code` can `ssh` append into the remote `thr_docs` inbox.

The rest of this document is a larger “full spec / roadmap” that generalizes transport, routing,
durability, policy, and app-server APIs.

---

## CLI helper (hackathon scope)

For demos (and for simple automation), you can send an event without manually crafting JSONL:

```bash
codex events send \
  --thread "$THREAD_ID" \
  --type agent.message \
  --severity info \
  --title "docs agent: FYI" \
  --summary "CLI flag --foo was added; docs likely need an update" \
  --payload-json '{"from_repo":"codex-cli"}'
```

To inspect an inbox:

```bash
codex events show --thread "$THREAD_ID" --last 20
```

---

## Goals

- **Ingest external events** while Codex is running, including mid-turn.
- **Route events to the right session/thread**, with sane defaults.
- Optionally **present events to the user** (TUI/IDE surfaces can come later).
- **Make events available to the agent** in a structured, non-instructional way.
- Support **multiple transports** (local IPC first; network/queues as optional adapters).
- Provide **policy hooks** to decide: steer (if enabled) vs. queue.
- Persist events so they’re not lost if the UI is briefly disconnected.

## Non-goals

- A full general-purpose workflow engine (this spec defines minimal hooks; workflow orchestration
  can live outside Codex).
- Guaranteeing “true realtime” model re-conditioning mid-token (we support practical strategies:
  queue or steer).
- Accepting arbitrary unauthenticated network traffic by default (local-only is the default
  posture).

---

## Terminology

- **Instance**: One running Codex process (TUI, IDE extension, or app-server client) with live
  state.
- **Thread**: A persisted conversation (a “session” in the conversational sense).
- **Turn**: One user→agent interaction cycle within a thread.
- **Focused thread**: The thread currently selected in an instance’s UI (active tab/focus).
  This is
  instance-local and is not used for routing by default in this spec.
- **External Event**: A structured message produced outside Codex tool execution.
- **Producer**: The system publishing events (CI, build script, PR bot, etc.).
- **Ingress**: How events reach Codex (socket, HTTP, queue adapter, file drop).
- **Router**: Determines which thread/turn receives the event and what to do with it.
- **Sink**: Where the event goes (UI notification, thread history item, automation).

---

## Event model

### Envelope (v1)

All external events use a versioned envelope so the format can evolve without breaking integrations.

```jsonc
{
  "schema_version": 1,
  "event_id": "evt_01HZY…", // unique per producer (or globally unique)
  "time_unix_ms": 1730831111000,
  "type": "build.completed", // dot-separated, stable
  "severity": "info", // debug|info|warning|error|critical

  "source": {
    "name": "buildkite",
    "instance": "org/repo", // optional
    "run_id": "build-1234", // optional
    "url": "https://…", // optional
    "labels": { "branch": "main" }, // optional
  },

  "routing": {
    "thread_id": "thr_123", // required for deterministic routing
    "turn_id": "turn_456", // optional; for mid-turn steering correlation
    "correlation_id": "release-1", // optional; ties multiple events together
  },

  "title": "Build finished",
  "summary": "Linux x86_64 passed; Windows failed",

  "payload": {
    // producer-defined structured data; MUST be valid JSON
  },

  "artifacts": [
    {
      "kind": "log",
      "title": "stderr",
      "ref": { "type": "url", "url": "https://…" },
      // alternative: { "type":"path", "path":"/tmp/build.log" }
    },
  ],

  "suggested_actions": [
    {
      "action_id": "act_1",
      "title": "Open build logs",
      "kind": "open_url",
      "args": { "url": "https://…" },
    },
    {
      "action_id": "act_2",
      "title": "Ask Codex to investigate failure",
      "kind": "start_turn",
      "args": { "prompt": "Investigate the Windows failure; propose a fix." },
    },
  ],

  "trust": {
    "origin": "local", // local|network|queue
    "authenticated": true,
    "provenance": "token", // token|mtls|none|…
    "treat_as_instruction": false, // MUST default false; see Security section
  },
}
```

### Size limits and attachments

- The envelope should be kept small (recommended soft limit: **≤ 64 KiB**).
- Large logs should be referenced via `artifacts` (URL/path) rather than embedded.
- The UI MAY fetch artifacts on demand; the agent SHOULD receive only summaries unless explicitly
  requested.

---

## Ingress / transport options

This spec supports multiple ingestion paths. Implementations can start with one (recommended: local
IPC) and add others as needed.

### Option A (recommended): local IPC endpoint (UDS / named pipe)

**What**: When a session is running, it creates a local-only endpoint:

- macOS/Linux: Unix domain socket (UDS) at `~/.codex/sessions/<thread_id>/events.sock`
- Windows: named pipe `\\\\.\\pipe\\codex\\<thread_id>\\events`

**Protocol**: newline-delimited JSON (`application/jsonl`), one event per line. The connection is
full-duplex; Codex replies with a single-line acknowledgment per accepted/rejected event so
producers can fail fast.

Request (one per line):

```jsonc
{
  "token": "codex_evt_tok_…",
  "event": {
    /* External Event envelope */
  },
}
```

Response (one per line):

```jsonc
{
  "ok": true,
  "event_id": "evt_01HZY…",
  "delivered": { "thread_id": "thr_123", "mode": "queue_for_next_turn" },
}
```

Errors set `ok:false` and include a stable `code`:

- `unauthorized` (bad/missing token)
- `invalid_event` (schema/version/type issues)
- `unknown_thread`
- `rate_limited`
- `duplicate_event` (see idempotency)

**Auth**:

- Endpoint path is not sufficient security; require an **ephemeral bearer token** generated at
  session start.
- Token is stored in a file with `0600` permissions (or platform equivalent):
  `~/.codex/sessions/<thread_id>/external_events.json`

**Pros**: local-first, fast, no port conflicts, works offline, minimal dependencies.

**Cons**: producers must be on the same machine.

### Option B: local HTTP listener (loopback)

**What**: Session binds to `127.0.0.1:<random_port>` (or configurable) and accepts:

- `POST /v1/events` (single event JSON)
- `POST /v1/events:batch` (array of events)

**Auth**: `Authorization: Bearer <token>` (same token file as Option A).

**Responses**:

- `202 Accepted` with `{ ok: true, delivered: … }`
- `400 Bad Request` with `{ ok: false, code: "invalid_event", message: "…" }`
- `401 Unauthorized` with `{ ok: false, code: "unauthorized" }`
- `404 Not Found` with `{ ok: false, code: "unknown_thread" }`
- `429 Too Many Requests` with `{ ok: false, code: "rate_limited" }`
- `409 Conflict` with `{ ok: false, code: "duplicate_event" }`

**Pros**: easy to integrate with webhooks via local forwarders (ngrok, SSH tunnel, etc.).

**Cons**: port management; security posture must remain loopback-only by default.

### Option C: file inbox (drop folder)

**What**: Session watches `~/.codex/sessions/<thread_id>/inbox/` for `*.json` or `*.jsonl`.

**Pros**: simplest producer story (write a file); very robust.

**Cons**: latency; file watcher complexity on some platforms; weaker dedupe unless event_id is
consistent.

### Option D: queue-backed adapter (Redis/NATS/RabbitMQ/MSMQ/etc.)

**What**: A separate adapter process subscribes to a queue and forwards to Option A/B.

- Redis: pub/sub or streams
- NATS: subjects
- RabbitMQ: routing keys
- MSMQ (Windows): queue names

**Pros**: cross-machine, many producers, durable buffering, enterprise-friendly.

**Cons**: operational overhead; authentication/ACL complexity; harder to “discover” the right
session.

**Spec stance**: Codex core does not need to embed all queue clients. Prefer adapters that normalize
into the envelope and forward to local ingress.

---

## Session discovery & addressing

External producers need a way to target a specific thread deterministically.

### Discovery file (per thread)

When a thread is started/resumed, Codex writes:

`~/.codex/sessions/<thread_id>/external_events.json`

```jsonc
{
  "thread_id": "thr_123",
  "created_unix_ms": 1730831111000,
  "ipc": {
    "type": "uds",
    "path": "/Users/me/.codex/sessions/thr_123/events.sock",
  },
  "http": { "url": "http://127.0.0.1:43117/v1/events" },
  "token": "codex_evt_tok_…",
  "capabilities": {
    "notify": true,
    "queue_for_next_turn": true,
    "turn_steer": true,
  },
}
```

Producers should always set `routing.thread_id`. Codex should not guess based on UI focus because a
single machine may have multiple running instances and each instance may have multiple threads
loaded.

### Convenience CLI

Add a small helper so scripts don’t need to parse paths:

- `codex events list` → show active threads with endpoints
- `codex events send --thread <THREAD_ID> --type build.completed --title "CI" --summary "…" \\
  --payload-json '{"job":"linux"}'`

This CLI is also the right place to enforce “explicit routing” (require `--thread`).

---

## Persistence and retention

To avoid losing events during UI reconnects and to support “queue for next turn”, Codex should
persist a small event log per thread:

- `~/.codex/sessions/<thread_id>/external_events.log.jsonl` (append-only)
- Optional index/state file (e.g., `external_events_state.json`) for read/ack status and last-seen
  offsets.

Retention defaults:

- cap by count (e.g., last 1,000 events per thread) and/or age (e.g., 7 days)
- always keep unread events until acknowledged

---

## Routing and delivery modes

On receipt, Codex routes the event and chooses a delivery strategy.

### Routing rules

1. If `routing.thread_id` is present and loaded, deliver to that thread.
2. If `routing.thread_id` is present but not loaded, persist it for that thread so it appears on
   resume.
3. If `routing.thread_id` is missing, reject with `invalid_event` (or a more specific
   `missing_thread_id`) rather than guessing based on UI focus.

### Delivery modes

Delivery mode is chosen by policy (config + event fields):

1. **Append-to-thread (context)**: persist an `ExternalEventItem` in thread history so future turns
   can use it as context.
2. **Queue-for-next-model-call**: store in a per-thread pending list; the next model call (user turn
   or auto-turn) begins with a compacted summary of queued events.
3. **Auto-turn**: if no turn is running, start a turn immediately that incorporates the event(s);
   if a turn is running, start the auto-turn immediately after `turn/completed`.
4. **Steer-in-flight** (optional capability): inject a minimal “FYI” summary into an in-progress
   turn (requires explicit mid-turn input support).
5. **Interrupt** (future): stop an in-flight turn and restart with new context. This is
   intentionally deferred because it discards in-flight work.

**Default policy (safe)**:

- `info/warning`: append-to-thread + (steer if enabled, else queue-for-next-model-call)
- `error/critical`: append-to-thread + (steer if enabled, else queue-for-next-model-call)

### Notification vs. “prompt injection”

This feature can be wired up in two fundamentally different ways:

- **Context-first** (recommended default): events are appended/queued into the thread so the model
  sees them on the next model call; no UI is required.
- **UI-first** (optional): events are primarily surfaced in the UI, and only become model context
  when the user opts in (start turn / include event).

Because external events are not authored by the user, automatic injection MUST be opt-in and must
preserve the “events are data” rule described in Security.

### Idempotency and dedupe

- `(source.name, event_id)` should be treated as the primary idempotency key.
- If an identical key is received again within a retention window, Codex should:
  - return `duplicate_event` to producers that want strict behavior, or
  - accept-but-drop (configurable) while still returning `ok:true` to reduce producer complexity.

---

## Agent integration (making Codex “respond”)

### Representing events in the conversation

Introduce a new persisted item type:

- `TurnItem::ExternalEvent(ExternalEventItem)` (protocol-level)

Minimal `ExternalEventItem` fields:

- `id`, `time_unix_ms`, `type`, `severity`
- `title`, `summary`
- `payload` (JSON), `artifacts` (refs)
- `source`, `correlation_id`
- `trust` (see Security)

**Important**: the item is _not_ a user instruction. It should be treated like tool output:
informative, structured, potentially untrusted.

### When to invoke the model automatically

There are three patterns to make Codex respond:

1. **User-driven** (default): UI shows “Ask Codex to handle this” action; user triggers a new
   turn.
2. **Auto-prompt** (opt-in): rules can start a turn automatically for certain event types (e.g.,
   “build failed”).
3. **In-flight steer** (optional): for long-running turns, inject event summaries to allow the
   agent
   to adjust without restarting.

Auto-prompt should be gated by config and respect approval policy (e.g., it can draft a plan/message
but not run shell commands without the usual approvals).

### Event compaction

To avoid context bloat, queued events should be compacted:

- group by `correlation_id` / `type` / `source.run_id`
- keep only the last N events per group
- produce a concise summary line per group
- include deep links to full event details in the UI

---

## UI / UX requirements

This section is optional and can be implemented later. The core value of external events is
asynchronous context for the model.

### TUI/IDE surfaces

- Unread badge count per thread.
- `/events` (or equivalent UI panel) lists:
  - time, severity color, type, title
  - keybindings / command palette actions from `suggested_actions`

### Mid-turn behavior

If an event arrives while Codex is generating:

- append/queue it immediately for the next model call
- optionally offer steer-in-flight if supported by the runtime

---

## Security and prompt-injection resistance

External events are a high-risk injection surface. This spec enforces:

1. **Events are data, not instructions**:
   - `trust.treat_as_instruction` MUST default to `false`.
   - Agent prompts should explicitly tell the model to treat external events as informational.
2. **Authentication by default**:
   - local IPC + token file with strict permissions.
   - loopback HTTP requires bearer token.
3. **UI labeling**:
   - show source + trust indicators (“local authenticated”, “network unauthenticated”).
4. **Action gating**:
   - suggested actions are UI affordances; executing them should still go through normal approvals
     (shell commands, network, file writes, etc.).
5. **Sanitization**:
   - strip/escape terminal control sequences in event text.
   - cap sizes and truncate with “view details” links.

### Safe defaults for automation

- Auto-turn and steer policies MUST be **opt-in**.
- Even when enabled, auto-turn SHOULD require `trust.authenticated=true` and a source allow-list
  (e.g., `source.name in ["buildkite", "github_actions"]`).
- Unauthenticated network-origin events SHOULD be treated as context-only (append/queue) and should
  not trigger auto-turns unless the user explicitly escalates trust.

---

## App-server protocol extension (optional but recommended)

For integrations that use `codex app-server`, add:

- `externalEvent/publish` → client pushes an event into a thread (or global inbox)
- `externalEvent/list` → list recent events for a thread
- `externalEvent/ack` → mark read/handled
- Notification: `externalEvent/received` → emitted when an event is accepted and routed

This allows IDEs (or other clients) to be the bridge between remote systems and the local Codex
instance.

---

## Configuration (sketch)

In `config.toml`:

```toml
[external_events]
enabled = true
default_delivery = "queue_for_next_turn" # or "notify_only"

[[external_events.sources]]
type = "uds"
enabled = true

[[external_events.sources]]
type = "http"
enabled = false
bind = "127.0.0.1:0" # 0 = random port

[[external_events.rules]]
match_type = "build.failed"
min_severity = "error"
delivery = "append_to_thread"
prefer_steer = true

[[external_events.rules]]
match_type = "build.completed"
delivery = "notify_only"
```

---

## Example flows

### 1. CI build completion → release follow-up

1. Producer posts `build.completed` with artifacts + `suggested_actions`.
2. Codex appends the event to the thread and queues it for the next model call.
3. Codex starts an auto-turn (if configured) or the user starts a turn: “Release v1.2.3 using the
   successful build artifacts; run the release checklist.”

### 2. Build failure mid-turn → steer or queue

1. Codex is implementing a fix.
2. Producer posts `build.failed` with failing test names and log URL.
3. If Steer is enabled, Codex immediately steers the in-flight turn with a compact failure summary.
4. If Steer is not enabled, Codex queues the failure and incorporates it on the next model call.

---

## Open questions

- Should `routing.thread_id` always be required, or should Codex accept unrouted events into a
  global inbox (never into model context) for purely informational notifications?
- Should events be persisted into the thread history by default, or only queued?
- What is the minimal “steer-in-flight” capability surface (new `turn/steer` vs. reuse of
  existing
  primitives)?
- How should remote adapters authenticate to the local session (mTLS vs bearer token + tunnel)?

---

## Appendix: hackathon-scope multi-agent (Codex-to-Codex) via file inbox

This section is intentionally narrow: it describes a concrete, achievable shape that enables
multiple Codex instances (possibly on different machines) to “talk” without implementing queues,
webhooks, or a workflow engine.

### Communication method

Use a single append-only JSONL file as the ingress point:

- `~/.codex/sessions/<thread_id>/external_events.inbox.jsonl`

The Codex UI tails the inbox file for any thread it has loaded and treats each line as one event.

This avoids “focus” ambiguity: producers choose the recipient explicitly by choosing the target
`<thread_id>` file to append to.

### Minimal event types

Define a small set of event types for the hackathon:

- `agent.message` (a message from another Codex instance or a bot)
- `build.status` (started / progress / completed / failed)

Suggested `agent.message` payload:

```jsonc
{
  "from": { "name": "codex-worker-1", "thread_id": "thr_worker" },
  "text": "I found the failure: it's a Windows path separator bug in …",
  "context_refs": [{ "kind": "path", "value": "codex-rs/tui/src/…" }]
}
```

### How one Codex instance sends to another

No special protocol is required for the hackathon. A sender can append one JSON line:

- locally: write to the other instance’s inbox file
- remotely: `ssh` into the target machine and append to the inbox file there

The receiving UI:

- enqueues the event into the target thread’s context
- optionally lists the message in an “External events” panel
- offers a single keybinding/command: “Start a turn with this” (copies a compacted summary into
  a
  new user prompt)

### What this does not do (on purpose)

- No automatic tool execution on the receiving side.
- No mid-turn prompt injection.
- No “workflow chaining” (build triggers release) beyond suggested actions that the user clicks.
- No “focus-based routing”: every send targets a specific thread.
