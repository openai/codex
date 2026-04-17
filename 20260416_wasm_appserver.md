# WASM Embedded App-Server Design

## TL;DR

The current browser harness wraps a single user submission by creating a thread, calling `thread.submit(Op::UserTurn { ... })`, and draining `thread.next_event()` until the turn completes. That path proved that the core harness can run in the browser, but it leaves the browser with a custom control surface that diverges from native app-server behavior and currently resets session state after every turn.

This document proposes switching the browser integration to an embedded in-process app-server runtime. The browser would keep a long-lived app-server instance in wasm, communicate with it through the existing app-server request and event model, and reuse the native app-server boundary instead of maintaining a browser-specific harness API.

## Objective

Use the existing app-server boundary as the primary control plane for the browser runtime so that:

- browser and native clients speak the same conceptual protocol
- multi-turn behavior matches native behavior more closely
- debugging and event inspection reuse existing app-server semantics
- future features such as thread lifecycle, steering, approvals, and richer tooling do not require a second browser-specific orchestration layer

## Background

### Current Browser Design

The current browser runtime is centered on `BrowserCodex` in `codex-rs/wasm-harness/src/browser.rs`.

Its main flow is:

1. `BrowserCodex.submit_turn(prompt, on_event)` creates a fresh `LocalSet`.
2. It either reuses or creates a `BrowserSession`.
3. It submits a turn directly to core with:

   ```rust
   session.thread.submit(Op::UserTurn { ... }).await
   ```

4. It loops on `session.thread.next_event().await` and forwards events to JS.
5. When the turn finishes, it clears `self.session`.

This is a thin wrapper around the core thread API. It is effectively a `UserSubmit`-style interface: the browser gives the harness a prompt, the harness starts one turn, then pushes turn events back to JS.

The relevant behavior today is:

- `submit_turn()` scopes execution to a per-turn `LocalSet`
- the browser wrapper owns a `BrowserSession { config, thread, session_configured }`
- the wrapper emits raw core events directly to JS
- the wrapper explicitly drops the session after each turn

### Why The Current Design Is Not Enough

The current design was useful as a proof of feasibility, but it creates several product and maintenance problems.

First, it is not actually aligned with the native client boundary. Native clients talk to app-server through requests, notifications, and streamed server events. The browser currently bypasses that layer and talks directly to `CodexThread`.

Second, it currently loses continuity across turns. Because `submit_turn()` uses a fresh `LocalSet` and clears `self.session` after each submission, the browser prototype starts a new harness session for every turn. That is why follow-up prompts appear to lack memory of earlier prompts.

Third, it forces the browser to maintain a separate orchestration contract. Any feature added at the app-server boundary has to be re-exposed or re-invented in the browser wrapper.

Fourth, it weakens debugging. Native app-server already has a request model, notification model, server-request model, and thread/turn lifecycle semantics. The browser wrapper currently exposes only a narrower direct-thread view.

## Why Switch To An Embedded App-Server

The app-server boundary already solves the problems the browser needs:

- request/response for typed client operations
- notifications for fire-and-forget client messages
- streamed server notifications for turn progress
- server requests for approvals and similar interactive flows
- explicit thread lifecycle APIs
- a stable place to add future browser features without widening the direct core API

The repository already includes an in-process embedding path in `codex-rs/app-server/src/in_process.rs`. That runtime preserves app-server semantics while replacing stdio/websocket transport with in-memory channels.

That makes it a much better browser boundary than `BrowserCodex.submit_turn(...)`.

## Goals

- Preserve the existing app-server request and event model in the browser.
- Keep a long-lived runtime alive across multiple browser turns.
- Support multi-turn conversations without rebuilding browser-specific thread state each turn.
- Reuse existing app-server features such as `thread/start`, `turn/start`, `turn/interrupt`, and server-driven approval requests.
- Keep the wasm integration transport-local and in-process. The browser should not need to run a real socket server to use app-server semantics.

## Non-Goals

- This document does not require full native parity for browser tools, filesystem access, or sandboxing.
- This document does not solve browser persistence by itself. State DB and rollout persistence still need separate wasm-capable implementations.
- This document does not require reusing `run_main_with_transport(...)` directly. The proposal reuses the app-server boundary, not that exact native entrypoint.

## Proposal

### High-Level Design

Replace the current direct-thread browser wrapper with a wasm-facing wrapper around an embedded in-process app-server runtime.

This refactor should be a replacement, not an addition. We should remove the existing `BrowserCodex` implementation as part of the cleanup and move the current prototype onto the new app-server-based path.

The new stack would look like:

```text
JS UI
  -> wasm wrapper
  -> in-process app-server runtime
  -> MessageProcessor
  -> ThreadManager / Codex core
```

Instead of calling `thread.submit(Op::UserTurn { ... })` directly, the browser would:

- start one in-process app-server runtime for the browser session
- send app-server client requests into that runtime
- consume app-server server notifications and server requests from that runtime

### Browser Boundary

The wasm wrapper should expose an API shaped like the app-server protocol rather than the current `submit_turn(prompt, on_event)` helper.

A minimal JS-facing API is:

- `start(options) -> handle`
- `request(request) -> Promise<response>`
- `notify(notification) -> Promise<void>` or `void`
- `nextEvent() -> Promise<event | null>`
- `respondToServerRequest(requestId, result) -> Promise<void>`
- `failServerRequest(requestId, error) -> Promise<void>`
- `shutdown() -> Promise<void>`

This mirrors the capabilities already present on `InProcessClientHandle`:

- `request(...)`
- `notify(...)`
- `next_event()`
- `respond_to_server_request(...)`
- `fail_server_request(...)`
- `shutdown()`

### Runtime Lifecycle

The browser should create one long-lived in-process app-server runtime and keep it alive until the browser session is reset or closed.

Expected lifecycle:

1. Browser constructs wasm wrapper.
2. Wrapper builds `InProcessStartArgs`.
3. Wrapper calls `codex_app_server::in_process::start(...)`.
4. Wrapper keeps the returned handle alive across turns.
5. JS sends requests such as `thread/start` and `turn/start`.
6. JS drains streamed server events with `nextEvent()`.
7. On shutdown or reset, wrapper calls `shutdown()`.

This is the key change from the current design. The runtime is session-scoped, not turn-scoped.

### Message Flow

For a typical first turn:

1. JS calls `request(thread/start { ... })`.
2. The app-server returns a thread id.
3. JS calls `request(turn/start { threadId, input, ... })`.
4. The app-server returns an in-progress turn response.
5. JS repeatedly calls `nextEvent()` or receives pushed events.
6. The runtime emits `turn/started`, item deltas, tool events, and `turn/completed`.

For later turns:

1. JS reuses the existing thread id.
2. JS calls `request(turn/start { threadId, input, ... })` again.
3. The same app-server runtime and the same underlying thread/session continue processing.

This aligns browser behavior with native behavior and removes the current per-turn session reset.

### Event Model

The browser should consume app-server events, not raw core `EventMsg` values.

That gives the browser:

- a stable protocol-shaped event stream
- server notifications for turn lifecycle and content updates
- server requests for approvals and other interactive flows
- lag/backpressure signals already defined by the in-process embedding

It also gives us a cleaner debugging surface because the browser can log:

- outgoing client requests
- client notifications
- incoming server notifications
- incoming server requests
- responses to server requests

### Why Use `in_process` Instead Of `run_main_with_transport`

`run_main_with_transport(...)` is not the right wasm entrypoint.

It hardcodes:

- stdio or websocket transport startup
- native signal handling
- native logging and DB startup assumptions

By contrast, `in_process.rs` already does the important part we want:

- run `MessageProcessor`
- keep the app-server request/notification/event contract
- replace transport with in-memory channels

So the design should reuse app-server semantics through `in_process`, not try to reuse the native transport bootstrap unchanged.

## Detailed Design

### 1. Replace `BrowserCodex` With A WASM App-Server Wrapper

Add a new browser-facing wrapper that owns:

- the long-lived in-process app-server handle
- browser-specific runtime services such as the code executor bridge
- browser session options needed to build config and initialize params

This wrapper replaces `BrowserCodex` as the main orchestration entrypoint.

As part of this refactor we should:

- remove the existing direct-thread `BrowserCodex` implementation
- remove `BrowserSession` as a browser-specific wrapper around `CodexThread`
- update the current browser prototype to construct and use the new app-server-based wrapper instead

We do not want to maintain two browser orchestration paths. The prototype should become the first consumer of the new design.

### 2. Request Serialization

The wrapper should accept JSON or `JsValue` payloads that correspond to app-server requests and notifications.

At the wasm boundary:

- JS passes a request object
- wasm deserializes into `ClientRequest` or `ClientNotification`
- `in_process` handles the request
- wasm serializes responses and events back to JS

This keeps the browser API close to the existing protocol and avoids introducing a second custom Rust-to-JS command language.

### 3. Thread Ownership

The browser should treat thread ids as app-server resources, not as direct `CodexThread` handles.

That means:

- creating threads through `thread/start`
- reading state through `thread/read` and related APIs
- starting turns through `turn/start`
- steering or interrupting turns through existing turn APIs

This is an important layering choice. The browser should stop owning direct thread runtime objects.

### 4. Approval And Server Request Handling

The browser must support app-server initiated requests back to the client.

Examples include:

- tool approval requests
- user input requests
- future browser-specific interactive flows

When `nextEvent()` yields a `ServerRequest`, JS must either:

- answer with `respondToServerRequest(...)`
- or reject with `failServerRequest(...)`

This is a capability the current direct-thread wrapper does not model cleanly.

### 5. Debugging

The browser wrapper should log the app-server boundary directly.

Recommended browser-side logging:

- every outgoing request with method and id
- every request result
- every incoming server notification
- every incoming server request
- every reply to a server request
- backpressure or lag markers

This is a better debugging surface than ad hoc logging around direct `submit()` and `next_event()` calls because it reflects the real control plane used by native clients.

## Storage And Persistence

This proposal improves the control plane, but it does not by itself provide browser persistence.

There are two separate persistence problems:

1. Core state DB
   Core currently initializes session storage internally. On `wasm32`, the current state DB bridge is stubbed and returns `None`.

2. Rollout persistence
   The current wasm rollout recorder is also stubbed. Recording is effectively a no-op and history reload is unavailable.

As a result, the embedded app-server design gives us:

- live multi-turn continuity within a running browser session

But it does not yet give us:

- browser reload/resume
- durable thread metadata
- durable rollout history

Those require follow-on work below the `in_process` layer.

## Migration Plan

### Milestone 1: Replace The Prototype Runtime With `in_process`

Build the wasm wrapper on top of `codex_app_server::in_process` and migrate the existing prototype to use it.

Scope:

- long-lived runtime
- request/notification/event API exposed to JS
- thread and turn flow through app-server methods
- browser code executor still supplied through existing wasm/runtime hooks
- remove the direct `BrowserCodex` / `BrowserSession` path from `wasm-harness`

Success criteria:

- multi-turn browser session works without resetting runtime state between turns
- browser logs show app-server request and event traffic
- browser no longer calls `thread.submit(Op::UserTurn { ... })` directly
- the existing browser prototype runs on the app-server-based implementation
- there is only one supported browser runtime path in the codebase

### Milestone 2: Align With `codex-app-server-client`

Decide whether the wasm wrapper should sit directly on `in_process` or on a thinner variant of `codex-app-server-client`.

The client facade is appealing because it already wraps:

- in-process runtime startup
- event forwarding
- server-request resolution helpers
- convergence with the remote app-server client shape

This may reduce custom browser-side orchestration code.

### Milestone 3: Browser Persistence

Add real wasm-backed persistence for:

- rollout history
- thread metadata
- any state DB-backed browser features we want to preserve across reloads

This likely requires explicit storage seams below the app-server wrapper.

## Alternatives Considered

### Keep The Current `BrowserCodex` Model And Preserve Session State

We could keep the current wrapper and only stop clearing `self.session`.

That would likely fix the immediate memory bug, but it would not fix the larger architectural problem:

- the browser would still use a custom direct-thread boundary
- app-server APIs would still need browser-specific re-exposure
- debugging would still happen at a less stable abstraction level

This is a tactical fix, not the design we want to converge on.

### Reuse `run_main_with_transport(...)` Directly

We could try to add a custom wasm transport and plug it into `run_main_with_transport(...)`.

This is not attractive because the function currently bundles:

- transport startup
- native shutdown handling
- logging and telemetry bootstrap
- native transport assumptions

`in_process` is already a better split for embedding.

## Risks

- The browser protocol surface becomes closer to app-server, which may require slightly more client-side plumbing than the current `submit_turn(prompt)` helper.
- Some app-server internals still assume native runtime pieces and may need additional injection points for wasm.
- Storage is still unresolved. This design fixes control-plane drift first, not persistence.
- If we expose raw JSON-RPC too literally to JS, the browser API may become awkward. We should keep the protocol shape while still providing small ergonomic helpers.

## Open Questions

1. Should the wasm wrapper expose raw JSON-RPC payloads, typed helper methods, or both?
2. Should the browser wrap `in_process` directly, or should it reuse a slimmer `codex-app-server-client` facade?
3. Which app-server methods do we want to support in browser v1 beyond `thread/start` and `turn/start`?
4. Do we want server events delivered by pull (`nextEvent`) only, or also by JS callback subscription?
5. What is the right storage abstraction for browser-backed rollout and state DB persistence?

## Recommended Decisions For Implementation Start

To begin implementation, we should make the following decisions explicit.

- Browser API surface
  Recommendation:
  Expose a thin typed wrapper over the app-server protocol rather than raw JSON-RPC only. The JS API should provide ergonomic helpers such as `startThread`, `startTurn`, `interruptTurn`, `nextEvent`, and `respondToServerRequest`, while staying close to app-server request and event types under the hood.

- `in_process` vs `codex-app-server-client`
  Recommendation:
  Start directly on `codex_app_server::in_process`. It is the simpler runtime dependency and gives us direct control in wasm. We should borrow the client facade's worker and event-forwarding patterns where useful, but avoid adding a second abstraction layer unless the wasm wrapper clearly grows into it.

- Session lifecycle
  Recommendation:
  Create one long-lived embedded app-server runtime per browser session and keep it alive until explicit reset or shutdown. Runtime lifetime should be browser-session scoped, not turn scoped. Reset should happen only on explicit session teardown, configuration changes that require rebuild, or API key changes.

- Event delivery model
  Recommendation:
  Use callback subscription as the primary browser-facing event model, while optionally retaining a polling escape hatch for tests or simple consumers. Internally we can still drain `next_event()`, but most browser UI code is simpler if events are pushed into JS callbacks.

- Server request handling
  Recommendation:
  Treat server requests as first-class and require explicit client handling. The wrapper should surface every `ServerRequest` to JS and require the browser to answer or reject it. For unsupported request types in browser v1, reject them explicitly with a clear error rather than letting turns hang.

- v1 app-server method set
  Recommendation:
  Support a narrow but complete thread and turn slice in browser v1: `thread/start`, `thread/read`, `turn/start`, `turn/interrupt`, and `turn/steer`. That is enough for a real multi-turn conversational product and debugging workflow without taking on the full app-server surface immediately.

## Recommendation

Move the browser runtime to an embedded in-process app-server boundary.

That keeps the browser on the same architectural path as native clients, fixes the current per-turn session model, improves debugging, and creates a better long-term seam for browser-specific runtime and storage work.
