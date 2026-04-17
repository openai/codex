# Codex WASM Harness Prototype

This crate is the first browser-facing seam for a Codex harness prototype.

It exposes one browser-facing layer:

- `BrowserAppServer`: a `wasm_bindgen` adapter that exposes an app-server-shaped
  browser boundary. It keeps a live `codex-core::CodexThread` behind a
  request/event interface, accepts app-server request envelopes such as
  `thread/start` and `turn/start`, and streams notifications plus raw core
  events back to JavaScript.

The demo page passes the user's API key into the WASM facade so Rust can make
browser `fetch` requests to the Responses API through the wasm-compatible
Codex client stack.

The API key field is for local prototype use only: it stores the key in browser
`localStorage` and sends it directly from the page. A production browser
integration should use a proxy or an ephemeral-token flow instead of persisting
long-lived API keys in the page origin.

This replaces the earlier `BrowserCodex.submit_turn(...)` prototype. The
browser example now uses the same high-level control plane as native Codex:
create or reuse a thread, start turns against it, and listen for async events.
The remaining work is to swap the direct core bridge under this wrapper for a
deeper reuse of the in-process app-server runtime once the storage/runtime
dependencies are wasm-compatible.

## Library Boundary

The intended downstream integration point is the crate itself. Downstream
webapps can:

- depend on `codex-wasm-harness` from a Git branch or local path;
- construct `BrowserAppServer` from JavaScript or wrap the crate with their own
  browser bindings;
- provide session-scoped prompt inputs such as `cwd`, developer instructions,
  and user/project-doc instructions;
- supply their own browser/runtime implementations for host services such as
  code execution, event rendering, or persistence; and
- keep app-specific browser glue out of the Codex repo.

The intended boundary is:

- the application resolves instruction sources such as `AGENTS.md`, AppKernel
  policy, approved OPFS layout, and approved GitHub URL prefixes;
- the harness accepts those resolved strings and injects them into the Codex
  session as base, developer, or user instructions.

Example:

```js
const app = new BrowserAppServer(apiKey);
app.setSessionOptions({
  cwd: "/workspace",
  instructions: {
    developer: "Code runs inside AppKernel. OPFS and network APIs are available.",
    user: "# AGENTS.md instructions for /workspace\n\n<INSTRUCTIONS>\n...\n</INSTRUCTIONS>",
  },
});
app.setEventHandler((event) => console.log(event));

const thread = await app.request({
  method: "thread/start",
  id: 1,
  params: {},
});

await app.request({
  method: "turn/start",
  id: 2,
  params: {
    threadId: thread.thread.id,
    input: [{ type: "text", text: "Say hello." }],
  },
});
```

## Current Limitations

This is a minimal browser port of the Codex app-server boundary, not full
desktop Codex.

- It keeps real Codex threads alive across turns, but many host-heavy services
  are still compiled into degraded wasm implementations.
- It currently relies on a single injected browser code executor for code mode.
- Native shell, PTY, MCP, plugin runtime, and filesystem-backed persistence are
  not available in the browser prototype.

The immediate implementation value is that downstream browser work can now
build on the real Codex harness boundary instead of a custom TypeScript or Rust
turn loop.

## Build Sketch

```sh
rustup target add wasm32-unknown-unknown
codex-rs/wasm-harness/scripts/build-browser-demo.sh
```

Then serve `codex-rs/wasm-harness/examples` and open `/browser/index.html`.
