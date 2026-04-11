# Codex WASM Harness Prototype

This crate is the first browser-facing seam for a Codex harness prototype.

It does not yet call `codex-core::run_turn` or `RegularTask::run`. Instead, it
establishes the intended browser API shape:

- submit a prompt from JavaScript;
- stream Codex-shaped turn events back to the page; and
- resolve after a `turn_complete` event.

The sampler is currently implemented in Rust/WASM. The demo page uses a
deterministic local response by default, or passes the user's API key into the
WASM facade so Rust can make a direct browser `fetch` request to the Responses
API.

The API key field is for local prototype use only: it stores the key in browser
`localStorage` and sends it directly from the page. A production browser
integration should use a proxy or an ephemeral-token flow instead of persisting
long-lived API keys in the page origin.

The next step is to replace the callback boundary with a real model transport
and then wire the facade to the Codex turn loop after host services are
injectable.

The `real-core` feature is an explicit compile probe for depending on
`codex-core`. It currently does not build for `wasm32-unknown-unknown`; the
first blocker is native Tokio/Mio networking pulled through the host-heavy
`codex-core` dependency graph.

## Current Limitations

This is a boundary prototype, not a port of `codex-core` yet.

- It does not construct a `Session` or `TurnContext`.
- It does not call `RegularTask::run` or `run_turn`.
- It does not expose native Codex tools.
- It emits Codex-shaped events, but not the full protocol event set.

The immediate implementation value is that the browser API and demo page can be
iterated independently while the host-heavy Codex services are moved behind
browser-compatible traits.

## Build Sketch

```sh
rustup target add wasm32-unknown-unknown
codex-rs/wasm-harness/scripts/build-browser-demo.sh
```

Then serve `codex-rs/wasm-harness/examples` and open `/browser/index.html`.
