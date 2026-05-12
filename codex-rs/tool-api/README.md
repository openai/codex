# codex-tool-api

`codex-tool-api` is the minimal extension-facing contract for contributed
function tools that can be injected into Codex without making `codex-core`
depend on the tool owner's crate.

Crates that define contributed tools should depend on this crate. It owns:

- the shared definition envelope: `ToolDefinition`, `ToolExposure`, and
  `ToolName`
- the executable runtime contract: `ToolExecutor`, `ToolCall`, `ToolFuture`,
  and `ToolError`
- the one model-visible spec an extension may contribute directly:
  `FunctionToolSpec`

The contract is intentionally narrow: a definition keeps one tool's canonical
name, model metadata, exposure mode, and opaque runtime together. Hosts may use
that envelope with richer internal metadata, while contributed extension tools
use `FunctionToolSpec`. Contributed tools receive a call id plus raw JSON
arguments and return a JSON value. If a feature needs richer host integration,
its extension is expected to do that wiring before exposing the tool rather than
widening this crate around the hardest native tools. For ordinary flat function
tools, `ToolDefinition::from_function_spec(...)` derives the canonical tool
name from the contributed spec and keeps construction terse.

The intended dependency direction is:

```text
tool-owning extension crate --> codex-tool-api <-- codex-core
```

`codex-tools` has a different job. It remains the host-side owner of Responses
API tool models, schema parsing, namespaces, discovery, MCP/dynamic conversion,
code-mode shaping, and other aggregate host concerns. A crate that only wants
to contribute one ordinary function tool through an extension should not need
to depend on `codex-tools`.
