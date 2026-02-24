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

## Module resolution

`js_repl` resolves **bare** specifiers (for example `await import("pkg")`) using an ordered
search path. Path-style specifiers (`./`, `../`, absolute paths, `file:` URLs) are rejected.

Module resolution proceeds in the following order:

1. `CODEX_JS_REPL_NODE_MODULE_DIRS` (PATH-delimited list)
2. `js_repl_node_module_dirs` in config/profile (array of absolute paths)
3. Thread working directory (cwd, always included as the last fallback)

For `CODEX_JS_REPL_NODE_MODULE_DIRS` and `js_repl_node_module_dirs`, module resolution is attempted in the order provided with earlier entries taking precedence.

## Usage

- `js_repl` is a freeform tool: send raw JavaScript source text.
- Optional first-line pragma:
  - `// codex-js-repl: timeout_ms=15000`
- Top-level bindings persist across calls.
- Top-level static import declarations (for example `import x from "pkg"`) are currently unsupported; use dynamic imports with `await import("pkg")`.
- Use `js_repl_reset` to clear the kernel state.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.tmpDir`: per-session scratch directory path.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside `js_repl` (including shell tools like `shell` / `shell_command` when available).
- To share generated images with the model, write a file under `codex.tmpDir`, call `await codex.tool("view_image", { path: "/absolute/path" })`, then delete the file.

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
