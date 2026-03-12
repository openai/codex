## exec
- Runs raw JavaScript in an isolated context (no Node, no file system, or network access, no console).
- Send raw JavaScript source text, not JSON, quoted strings, or markdown code fences.
- You may optionally start the tool input with a first-line pragma like `// @exec: {"yield_time_ms": 10000, "max_output_tokens": 1000}`.
- `yield_time_ms` asks `exec` to yield early after that many milliseconds if the script is still running.
- `max_output_tokens` sets the token budget for direct `exec` results. By default the result is truncated to 10000 tokens.
- You have a set of tools provided to you. They are imported either from `tools.js` or `/mcp/server.js`
- Tool methods take either string or object as parameter.
- They return either a structured value or a string based on the description above.

- Surface text back to the model with `output_text(v: string | number | boolean | undefined | null)`. A string representation of the value is returned to the model. Manually serialize complex values.

- Methods available in `@openai/code_mode` module:
- `output_text(value: string | number | boolean | undefined | null)`: A string representation of the value is returned to the model. Manually serialize complex values.
- `output_image(imageUrl: string)`: An image is returned to the model. `image_url` can be an HTTPS URL or a base64-encoded `data:` URL.
- `store(key: string, value: any)`: stores a serializeable value under a string key for later `exec` calls in the same session.
- `load(key: string)`: returns the stored value for a string key, or `undefined` if it is missing.
- `yield_control()`: yields the accumulated output to the model immediately while the script keeps running.
