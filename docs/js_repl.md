# JavaScript REPL (`js_repl`)

`js_repl` runs JavaScript in a persistent Node-backed kernel with top-level `await`.

## Feature gate

`js_repl` is disabled by default and only appears when:

```toml
[features]
js_repl = true
```

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
- Top-level bindings persist across calls.
- Use `js_repl_reset` to clear the kernel state.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.state`: mutable object persisted for the current kernel session.
- `codex.tmpDir`: per-session scratch directory path.
- `codex.sh(command, opts?)`: runs a shell command through Codex execution policy and returns `{ stdout, stderr, exitCode }`.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside js_repl.
- `codex.emitImage(pathOrBytes, { mime?, caption?, name? })`: emits an image artifact in tool output.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses a JSON-line transport over stdio.
