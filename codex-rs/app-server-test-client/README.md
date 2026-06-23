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

`send-message` and `send-message-v2` handle `request_user_input` server requests interactively.
When Codex asks a question, choose a numbered option (or `o` for a free-form answer when offered)
and the client will send the response and continue streaming the same turn.

## Testing Plugin Analytics

The `plugin-analytics-smoke` command exercises `plugin/installed`, plugin
enable/disable config writes, and a structured plugin mention through one
app-server connection. Analytics are captured to a local JSONL file and are
not sent to the analytics backend. The model turn uses a loopback Responses
API server.

The selected plugin must already be installed and enabled remotely, and the
active Codex profile must be authenticated. On a fresh local cache, the command
retries ephemeral turns while the installed remote bundle finishes syncing.

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
`codex_plugin_used` event with the expected local and remote plugin identities
and capability metadata. Each event includes the local ID in `plugin_id` and the
backend ID in `remote_plugin_id`. The enabled and disabled events come from
successful writes to the temporary config; the command does not mutate the
remote enabled state. It prints the events and leaves the JSONL file in place
for inspection. It does not install or uninstall plugins and does not modify
the profile's persistent config.

### Testing remote install and uninstall analytics

`plugin-analytics-mutation-smoke` is a manually invoked live smoke test. It
contacts the configured remote plugin API and temporarily changes the active
account's installed-plugin state. It is not run by `cargo test`, `just test`,
or CI.

Choose a remote plugin that is available to the active account and is not
currently installed. The command refuses to run when the plugin is already
installed, installs it, validates `codex_plugin_installed`, uninstalls it, and
validates `codex_plugin_uninstalled`, and verifies that the original
uninstalled state was restored.

The mutation events include the local Codex ID in `plugin_id` and the backend ID
in `remote_plugin_id`.

`--remote-plugin-id` takes the backend ID, such as `plugins~Plugin_...`, not the
local `<plugin>@<marketplace>` ID.

```bash
cargo run -p codex-app-server-test-client -- \
  --codex-bin ./target/debug/codex \
  plugin-analytics-mutation-smoke \
  --remote-plugin-id <REMOTE_PLUGIN_ID> \
  --confirm-account-mutation \
  --capture-file /tmp/plugin-mutation-analytics.jsonl
```

Analytics use the normal queue, reduction, batching, and serialization path,
but the debug capture destination suppresses analytics network delivery. The
command prints one of these final states:

- `PASS`: the install and uninstall events validated and the plugin is uninstalled.
- `FAIL-CLEAN`: validation failed, but the original uninstalled state was
  restored.
- `FAIL-LOCAL-CACHE`: the backend is uninstalled, but local cleanup reported
  an error.
- `FAIL-DIRTY`: cleanup failed and the plugin still appears installed.
- `FAIL-UNKNOWN`: the command could not verify the final installed state.

For a dirty or uncertain result, retry cleanup with:

```bash
cargo run -p codex-app-server-test-client -- \
  --codex-bin ./target/debug/codex \
  plugin-remote-uninstall \
  --remote-plugin-id <REMOTE_PLUGIN_ID> \
  --confirm-account-mutation
```

Cleanup does not require analytics capture or a debug Codex binary. When the
smoke uses global `--config` overrides, its printed recovery command preserves
them so cleanup targets the same backend and account.

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
