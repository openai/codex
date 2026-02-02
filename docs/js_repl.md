# JavaScript REPL (`js_repl`)

`js_repl` runs JavaScript in a persistent Node-backed kernel with top-level `await`.

## Feature gate

`js_repl` is disabled by default and only appears when:

```toml
[features]
js_repl = true
```

`js_repl_tools_only` can be enabled to force direct model tool calls through `js_repl`:

```toml
[features]
js_repl = true
js_repl_tools_only = true
```

When enabled, direct model tool calls are restricted to `js_repl` and `js_repl_reset`; other tools remain available via `await codex.tool(...)` inside js_repl.

`js_repl_polling` can be enabled to allow async/polled execution:

```toml
[features]
js_repl = true
js_repl_polling = true
```

When enabled, `js_repl` accepts `poll=true` in the first-line pragma and returns an `exec_id`. Use `js_repl_poll` with that `exec_id` until the response `status` becomes `completed` or `error`.

## Node runtime

`js_repl` requires a Node version that meets or exceeds `codex-rs/node-version.txt`.

Runtime resolution order:

1. `CODEX_JS_REPL_NODE_PATH` environment variable
2. `js_repl_node_path` in config/profile
3. `node` discovered on `PATH`

You can configure an explicit runtime path:

```toml
js_repl_node_path = "/absolute/path/to/node"
```

## Usage

- `js_repl` is a freeform tool: send raw JavaScript source text.
- Optional first-line pragma:
  - `// codex-js-repl: timeout_ms=15000 reset=true`
  - `// codex-js-repl: poll=true timeout_ms=15000`
- Top-level bindings persist across calls.
- Use `js_repl_reset` to clear the kernel state.

### Polling flow

1. Submit with `js_repl` and `poll=true` pragma.
2. Read `exec_id` from the JSON response.
3. Call `js_repl_poll` with `{"exec_id":"...","yield_time_ms":1000}`.
4. Repeat until `status` is `completed` or `error`.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.state`: mutable object persisted for the current kernel session.
- `codex.tmpDir`: per-session scratch directory path.
- `codex.sh(command, opts?)`: runs a shell command through Codex execution policy and returns `{ stdout, stderr, exitCode }`.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside js_repl.
- `codex.emitImage(pathOrBytes, { mime?, caption?, name? })`: emits an image artifact in tool output.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses a JSON-line transport over stdio.
