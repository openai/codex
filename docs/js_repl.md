# JavaScript REPL (`js_repl`)

`js_repl` runs JavaScript in a persistent Node-backed kernel with top-level `await`.

## Feature gates

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

When enabled, direct model tool calls are restricted to `js_repl` and `js_repl_reset` (and `js_repl_poll` / `js_repl_cancel` if polling is enabled). Other tools remain available via `await codex.tool(...)` inside `js_repl`.

`js_repl_polling` can be enabled to allow async/polled execution:

```toml
[features]
js_repl = true
js_repl_polling = true
```

When enabled, `js_repl` accepts `poll=true` in the first-line pragma and returns an `exec_id`. Use `js_repl_poll` with that `exec_id` until `status` becomes `completed` or `error`. Use `js_repl_cancel` with that `exec_id` to stop a long-running execution.

## Node runtime selection

`js_repl` requires a Node version that meets or exceeds `codex-rs/node-version.txt`.

Runtime resolution order:

1. `CODEX_JS_REPL_NODE_PATH` environment variable
2. `js_repl_node_path` in config/profile
3. Bundled runtime under `vendor/<target>/node/node(.exe)` relative to the Codex executable
4. `node` discovered on `PATH`

You can configure an explicit runtime path:

```toml
js_repl_node_path = "/absolute/path/to/node"
```

## Usage

- `js_repl` is a freeform tool: send raw JavaScript source text.
- Optional first-line pragma:
  - `// codex-js-repl: timeout_ms=15000`
  - `// codex-js-repl: poll=true timeout_ms=15000`
- Top-level bindings persist across calls.
- Top-level static import declarations (for example `import x from "pkg"`) are currently unsupported; use dynamic imports with `await import("pkg")`.
- Use `js_repl_reset` to clear kernel state.

### Polling flow

1. Submit with `js_repl` and `poll=true`.
2. Read `exec_id` from the response.
3. Call `js_repl_poll` with `{"exec_id":"...","yield_time_ms":1000}`.
4. Repeat until `status` is `completed` or `error`.
5. Optional: call `js_repl_cancel` with `{"exec_id":"..."}` to cancel a running execution.

## Kernel helper APIs

`js_repl` exposes these globals:

- `codex.state`: mutable object persisted for the current kernel session.
- `codex.tmpDir`: per-session scratch directory path.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside `js_repl` (including shell tools like `shell` / `shell_command` when available).
- To share generated images with the model, write a file under `codex.tmpDir`, call `await codex.tool("view_image", { path: "/absolute/path" })`, then delete the file.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses JSON lines on stdio for host/kernel protocol.

## Process isolation and environment

The Node process is launched with a constrained environment derived from Codex policy and then scrubbed of pre-existing JS/package-manager settings.

Before launch, host code removes inherited keys matching:

- `NODE_*`
- `NPM_CONFIG_*`
- `YARN_*`
- `PNPM_*`
- `COREPACK_*`

Then `js_repl` sets explicit values under a dedicated home (`$CODEX_HOME/js_repl`), including:

- `CODEX_JS_REPL_HOME`
- `CODEX_JS_REPL_VENDOR_NODE_MODULES` (`$CODEX_HOME/js_repl/codex_node_modules/node_modules`)
- `CODEX_JS_REPL_USER_NODE_MODULES` (`$CODEX_HOME/js_repl/node_modules`)
- `CODEX_JS_TMP_DIR`
- `NODE_PATH` (vendor + user module roots)
- `NODE_REPL_HISTORY`
- redirected `HOME`/`USERPROFILE` and package-manager cache/config roots (`npm`, `yarn`, `pnpm`, `corepack`, `XDG_*`)

## Module resolution order

For bare specifiers (for example `import("lodash")`), `js_repl` resolves in this order:

1. Node built-ins (`node:fs`, `fs`, etc.)
2. Vendored modules under `$CODEX_HOME/js_repl/codex_node_modules/node_modules`
3. User modules under `$CODEX_HOME/js_repl/node_modules`

If the bare specifier is not found in those roots, the import fails.

Relative/absolute/file imports still resolve from `process.cwd()`, but bare imports are kept inside the js_repl vendor/user roots.

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
