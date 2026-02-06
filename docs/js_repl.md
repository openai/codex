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
  - `// codex-js-repl: timeout_ms=15000`
- Top-level bindings persist across calls.
- Top-level static import declarations (for example `import x from "pkg"`) are currently unsupported; use dynamic imports with `await import("pkg")`.
- Use `js_repl_reset` to clear the kernel state.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.state`: mutable object persisted for the current kernel session.
- `codex.tmpDir`: per-session scratch directory path.
- `codex.sh(command, opts?)`: runs a shell command through Codex execution policy and returns `{ stdout, stderr, exitCode }`.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside js_repl.
- `codex.emitImage(pathOrBytes, { mime?, caption?, name? })`: emits an image artifact in tool output.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses a JSON-line transport over stdio.

## Vendored parser asset (`meriyah.umd.min.js`)

The kernel embeds a vendored Meriyah bundle at:

- `codex-rs/core/src/tools/js_repl/meriyah.umd.min.js`

Current source is `meriyah@7.0.0` from npm (`dist/meriyah.umd.min.js`).
Licensing is tracked in:

- `third_party/meriyah/LICENSE`
- `NOTICE`

### How this file was sourced

From a clean temp directory:

```sh
tmp="$(mktemp -d)"
cd "$tmp"
npm pack meriyah@7.0.0
tar -xzf meriyah-7.0.0.tgz
cp package/dist/meriyah.umd.min.js /path/to/repo/codex-rs/core/src/tools/js_repl/meriyah.umd.min.js
cp package/LICENSE.md /path/to/repo/third_party/meriyah/LICENSE
```

### How to update to a newer version

1. Replace `7.0.0` in the commands above with the target version.
2. Copy the new `dist/meriyah.umd.min.js` into `codex-rs/core/src/tools/js_repl/meriyah.umd.min.js`.
3. Copy the package license into `third_party/meriyah/LICENSE`.
4. Update the version string in the header comment at the top of `meriyah.umd.min.js`.
5. Update `NOTICE` if the upstream copyright notice changed.
6. Run the relevant `js_repl` tests.
