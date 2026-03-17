## exec
- Runs raw JavaScript in an isolated context (no Node, no file system, or network access, no console).
- Send raw JavaScript source text, not JSON, quoted strings, or markdown code fences.
- You may optionally start the tool input with a first-line pragma like `// @exec: {"yield_time_ms": 10000, "max_output_tokens": 1000}`.
- `yield_time_ms` asks `exec` to yield early after that many milliseconds if the script is still running.
- `max_output_tokens` sets the token budget for direct `exec` results. By default the result is truncated to 10000 tokens.
- All nested tools are available on the global `tools` object, for example `await tools.exec_command(...)`. Tool names are exposed as normalized JavaScript identifiers, for example `await tools.mcp__ologs__get_profile(...)`.
- Tool methods take either string or object as parameter.
- They return either a structured value or a string based on the description above.

- Global helpers:
- `exit()`: Immediately ends the current script successfully (like an early return from the top level).
- `text(value: string | number | boolean | undefined | null)`: Appends a text item and returns it. Non-string values are stringified with `JSON.stringify(...)` when possible.
- `image(imageUrl: string)`: Appends an image item and returns it. `image_url` can be an HTTPS URL or a base64-encoded `data:` URL.
- `store(key: string, value: any)`: stores a serializable value under a string key for later `exec` calls in the same session.
- `load(key: string)`: returns the stored value for a string key, or `undefined` if it is missing.
- `ALL_TOOLS`: metadata for the enabled nested tools as `{ name, description }` entries.
- `yield_control()`: yields the accumulated output to the model immediately while the script keeps running.
