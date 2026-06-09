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

## Testing Plugin Analytics

The `plugin-analytics-smoke` command exercises `plugin/installed`, plugin
enable/disable config writes, and a structured plugin mention through one
app-server connection. Analytics are captured to a local JSONL file and are
not sent to the analytics backend. The model turn uses a loopback Responses
API server.

The selected plugin must already be installed remotely, and the active Codex
profile must be authenticated.

```bash
# Build a debug Codex binary; analytics capture is unavailable in release builds.
cargo build -p codex-cli --bin codex

cargo run -p codex-app-server-test-client -- \
  --codex-bin ./target/debug/codex \
  plugin-analytics-smoke \
  --plugin-id linear@openai-curated-remote
```

Use `--capture-file /tmp/plugin-analytics.jsonl` to select the output path.
The command validates one `codex_plugin_disabled`, `codex_plugin_enabled`, and
`codex_plugin_used` event with the expected local plugin identity and capability
metadata. It prints the events and leaves the JSONL file in place for
inspection. It does not install or uninstall plugins and does not modify the
profile's persistent config.

## Watching Raw Inbound Traffic

Initialize a connection, then print every inbound JSON-RPC message until you stop it with
`Ctrl+C`:

```bash
cargo run -p codex-app-server-test-client -- watch
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
