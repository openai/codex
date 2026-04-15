# Codex WASM Harness Prototype

This crate is the first browser-facing seam for a Codex harness prototype.

It exposes one browser-facing layer:

- `BrowserCodex`: a `wasm_bindgen` adapter that starts a real
  `codex-core::CodexThread`, calls `CodexThread::submit(Op::UserTurn { ... })`,
  and streams real Codex protocol events back to JavaScript.

The demo page passes the user's API key into the WASM facade so Rust can make
browser `fetch` requests to the Responses API through the wasm-compatible
Codex client stack.

The API key field is for local prototype use only: it stores the key in browser
`localStorage` and sends it directly from the page. A production browser
integration should use a proxy or an ephemeral-token flow instead of persisting
long-lived API keys in the page origin.

The remaining work is to replace the current browser-only host shims with more
complete browser implementations for persistence, richer tools, and other
host-heavy services.

## Library Boundary

The intended downstream integration point is the crate itself. Downstream
webapps can:

- depend on `codex-wasm-harness` from a Git branch or local path;
- construct `BrowserCodex` from JavaScript or wrap the crate with their own
  browser bindings;
- supply their own browser/runtime implementations for host services such as
  code execution, event rendering, or persistence; and
- keep app-specific browser glue out of the Codex repo.

## Current Limitations

This is a minimal browser port of the Codex turn loop, not full desktop Codex.

- It uses the real `CodexThread` turn path, but many host-heavy services are
  still compiled into degraded wasm implementations.
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
