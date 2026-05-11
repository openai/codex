# codex-tool-api

`codex-tool-api` is the minimal extension-facing contract for contributed
function tools that can be injected into Codex without making `codex-core`
depend on the tool owner's crate.

Crates that define contributed tools should depend on this crate. It owns:

- the executable bundle contract: `ToolBundle`, `ToolExecutor`, `ToolCall`,
  and `ToolError`
- the one model-visible spec an extension may contribute directly:
  `FunctionToolSpec`

The contract is intentionally narrow: contributed tools receive a call id plus
raw JSON arguments and return a JSON value. If a feature needs richer host
integration, its extension is expected to do that wiring before exposing the
tool rather than widening this crate around the hardest native tools.

The intended dependency direction is:

```text
tool-owning extension crate --> codex-tool-api <-- codex-core
```

`codex-tools` has a different job. It remains the host-side owner of Responses
API tool models, schema parsing, namespaces, discovery, MCP/dynamic conversion,
code-mode shaping, and other aggregate host concerns. A crate that only wants
to contribute one ordinary function tool through an extension should not need
to depend on `codex-tools`.
