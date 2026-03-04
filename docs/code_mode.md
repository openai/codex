# Code Mode (`code_mode`)

`code_mode` runs JavaScript in a Node-backed `node:vm` context.

## Feature gate

`code_mode` is disabled by default and only appears when:

```toml
[features]
code_mode = true
```

Unlike `js_repl`, enabling `code_mode` does **not** disable direct model tool calls.

## Node runtime

`code_mode` uses the same Node runtime resolution as `js_repl`:

1. `CODEX_JS_REPL_NODE_PATH` environment variable
2. `js_repl_node_path` in config/profile
3. `node` discovered on `PATH`

## Usage

- `code_mode` is a freeform tool: send raw JavaScript source text.
- It exposes async wrappers for other tools through `await tools[name](args)` and identifier globals for valid tool names. Nested tool calls resolve to arrays of content items.
- Function tools require JSON object arguments. Freeform tools require raw strings.
- `add_content(value)` is synchronous. It accepts a content item or an array of content items, so `add_content(await exec_command(...))` returns the same content items a direct tool call would expose.
- Only content passed to `add_content(value)` is surfaced back to the model.
- The tool description lists which nested tools are available in the current session.
- `code_mode` cannot invoke itself recursively.

## Notes

- Because `code_mode` uses `node:vm`, it is lighter than the persistent `js_repl` kernel but does not keep top-level bindings between calls.
