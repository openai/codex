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
search path. Local file imports are also supported for relative paths, absolute paths, and
`file://` URLs that point to ESM `.js` / `.mjs` files.

Module resolution proceeds in the following order:

1. `CODEX_JS_REPL_NODE_MODULE_DIRS` (PATH-delimited list)
2. `js_repl_node_module_dirs` in config/profile (array of absolute paths)
3. Thread working directory (cwd, always included as the last fallback)

For `CODEX_JS_REPL_NODE_MODULE_DIRS` and `js_repl_node_module_dirs`, module resolution is attempted in the order provided with earlier entries taking precedence.

Bare package imports always use this REPL-wide search path, even when they originate from an
imported local file. They are not resolved relative to the imported file's location.

## Usage

- `js_repl` is a freeform tool: send raw JavaScript source text.
- Optional first-line pragma:
  - `// codex-js-repl: timeout_ms=15000`
- Top-level bindings persist across calls.
- If a cell throws, prior bindings remain available, lexical bindings whose initialization completed before the throw stay available in later calls, and hoisted `var` / `function` bindings persist only when execution clearly reached their declaration or a supported write site.
- Supported hoisted-`var` failed-cell cases are direct top-level identifier writes and updates before the declaration (for example `x = 1`, `x += 1`, `x++`, `x &&= 1`) and non-empty top-level `for...in` / `for...of` loops.
- Intentionally unsupported failed-cell cases include hoisted function reads before the declaration, aliasing or direct-IIFE-based inference, writes in nested blocks or other nested statement structure, nested writes inside already-instrumented assignment RHS expressions, destructuring-assignment recovery for hoisted `var`, partial `var` destructuring recovery, pre-declaration `undefined` reads, and empty top-level `for...in` / `for...of` loop vars.
- Top-level static import declarations (for example `import x from "pkg"`) are currently unsupported; use dynamic imports with `await import("pkg")`.
- Imported local files must be ESM `.js` / `.mjs` files and run in the same REPL VM context as the calling cell.
- Static imports inside imported local files may only target other local `.js` / `.mjs` files via relative paths, absolute paths, or `file://` URLs. Bare package and builtin imports from local files must stay dynamic via `await import(...)`.
- `import.meta.resolve()` returns importable strings such as `file://...`, bare package names, and `node:fs`; the returned value can be passed back to `await import(...)`.
- Local file modules reload between execs, so a later `await import("./file.js")` picks up edits and fixed failures. Top-level bindings you already created still persist until `js_repl_reset`.
- Use `js_repl_reset` to clear the kernel state.

## Helper APIs inside the kernel

`js_repl` exposes these globals:

- `codex.cwd`: REPL working directory path.
- `codex.homeDir`: effective home directory path from the kernel environment.
- `codex.tmpDir`: per-session scratch directory path.
- `codex.tool(name, args?)`: executes a normal Codex tool call from inside `js_repl` (including shell tools like `shell` / `shell_command` when available).
- `codex.emitImage(imageLike)`: explicitly adds one image to the outer `js_repl` function output each time you call it.
- `codex.tool(...)` and `codex.emitImage(...)` keep stable helper identities across cells. Saved references and persisted objects can reuse them in later cells, but async callbacks that fire after a cell finishes still fail because no exec is active.
- Imported local files run in the same VM context, so they can also access `codex.*`, the captured `console`, and Node-like `import.meta` helpers.
- Each `codex.tool(...)` call emits a bounded summary at `info` level from the `codex_core::tools::js_repl` logger. At `trace` level, the same path also logs the exact raw response object or error string seen by JavaScript.
- Nested `codex.tool(...)` outputs stay inside JavaScript unless you emit them explicitly.
- `codex.emitImage(...)` accepts a data URL, a single `input_image` item, an object like `{ bytes, mimeType }`, or a raw tool response object that contains exactly one image and no text. Call it multiple times if you want to emit multiple images.
- `codex.emitImage(...)` rejects mixed text-and-image content.
- Request full-resolution image processing with `detail: "original"` only when the `view_image` tool schema includes a `detail` argument. The same availability applies to `codex.emitImage(...)`: if `view_image.detail` is present, you may also pass `detail: "original"` there. Use this when high-fidelity image perception or precise localization is needed, especially for CUA agents.
- Raw MCP image blocks can request the same behavior by returning `_meta: { "codex/imageDetail": "original" }` on the image content item.
- Example of sharing an in-memory Playwright screenshot: `await codex.emitImage({ bytes: await page.screenshot({ type: "jpeg", quality: 85 }), mimeType: "image/jpeg", detail: "original" })`.
- Example of sharing a local image tool result: `await codex.emitImage(codex.tool("view_image", { path: "/absolute/path", detail: "original" }))`.
- When encoding an image to send with `codex.emitImage(...)` or `view_image`, prefer JPEG at about 85 quality when lossy compression is acceptable; use PNG when transparency or lossless detail matters. Smaller uploads are faster and less likely to hit size limits.

Avoid writing directly to `process.stdout` / `process.stderr` / `process.stdin`; the kernel uses a JSON-line transport over stdio.

## Debug logging

Nested `codex.tool(...)` diagnostics are emitted through normal `tracing` output instead of rollout history.

- `info` level logs a bounded summary.
- `trace` level also logs the exact serialized response object or error string seen by JavaScript.

For `codex app-server`, these logs are written to the server process `stderr`.

Examples:

```sh
RUST_LOG=codex_core::tools::js_repl=info \
LOG_FORMAT=json \
codex app-server \
2> /tmp/codex-app-server.log
```

```sh
RUST_LOG=codex_core::tools::js_repl=trace \
LOG_FORMAT=json \
codex app-server \
2> /tmp/codex-app-server.log
```

In both cases, inspect `/tmp/codex-app-server.log` or whatever sink captures the process `stderr`.

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
