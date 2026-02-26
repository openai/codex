# App Server Test Client
Quickstart for running and hitting `codex app-server`.

## Quickstart

Run from `<reporoot>/codex-rs`.

```bash
# 1) Build debug codex binary
cargo build -p codex-cli --bin codex

# 2) Start websocket app-server in background
cargo run -p codex-app-server-test-client -- \
  --codex-bin ./target/debug/codex \
  serve --listen ws://127.0.0.1:4222 --kill

# 3) Call app-server (defaults to ws://127.0.0.1:4222)
cargo run -p codex-app-server-test-client -- model-list
```

## Testing Thread Rejoin Behavior

Build and start an app server using commands above. The app-server log is written to `/tmp/codex-app-server-test-client/app-server.log`

### 1) Get a thread id

Create at least one thread, then list threads:

```bash
cargo run -p codex-app-server-test-client -- send-message-v2 "seed thread for rejoin test"
cargo run -p codex-app-server-test-client -- thread-list --limit 5
```

Copy a thread id from the `thread-list` output.

### 2) Rejoin while a turn is in progress (two terminals)

Terminal A:

```bash
cargo run --bin codex-app-server-test-client -- \
  resume-message-v2 <THREAD_ID> "respond with thorough docs on the rust core"
```

Terminal B (while Terminal A is still streaming):

```bash
cargo run --bin codex-app-server-test-client -- thread-resume <THREAD_ID>
```

## Live Elicitation Timeout Pause Harness

This harness starts or connects to a websocket `codex app-server`, prompts a
real model to run the repo-local
`scripts/live_elicitation_hold.sh` helper, and verifies that the turn survives a
15 second unified-exec command even though `exec_command` normally yields after
10 seconds.

The helper script:

- reads the auto-injected `CODEX_THREAD_ID`
- calls `thread/increment_elicitation` for that thread
- sleeps for 15 seconds
- calls `thread/decrement_elicitation`

The harness also fails if the turn starts new items before the helper script
prints its final `[elicitation-hold] done` marker.

Run it from `<reporoot>/codex-rs`:

```bash
cargo build -p codex-cli --bin codex
cargo build -p codex-app-server-test-client

cargo run -p codex-app-server-test-client -- \
  --codex-bin ./target/debug/codex \
  live-elicitation-timeout-pause \
  --model gpt-5 \
  --workspace ..
```

Notes:

- Pass `--url ws://host:port` instead of `--codex-bin` to reuse an already
  running websocket app-server.
- The harness uses the current `codex-app-server-test-client` binary as the
  callback client inside the helper script.
- For ad hoc debugging, the helper RPCs are also exposed directly:

```bash
cargo run -p codex-app-server-test-client -- \
  --url ws://127.0.0.1:4222 \
  thread-increment-elicitation <THREAD_ID>

cargo run -p codex-app-server-test-client -- \
  --url ws://127.0.0.1:4222 \
  thread-decrement-elicitation <THREAD_ID>
```
