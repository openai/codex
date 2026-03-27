# codex-tools

`codex-tools` is intended to become the home for tool-related code that is
shared across multiple crates and does not need to stay coupled to
`codex-core`.

Today this crate is intentionally small. It currently owns the shared tool
schema primitives that no longer need to live in `core/src/tools/spec.rs`:

- `JsonSchema`
- `AdditionalProperties`
- `ToolDefinition`
- `parse_tool_input_schema()`
- `parse_dynamic_tool()`
- `parse_mcp_tool()`
- `mcp_call_tool_result_output_schema()`

That extraction is the first step in a longer migration. The goal is not to
move all of `core/src/tools` into this crate in one shot. Instead, the plan is
to peel off reusable pieces in reviewable increments while keeping
compatibility-sensitive orchestration in `codex-core` until the surrounding
boundaries are ready.

## Vision

Over time, this crate should hold tool-facing primitives that are shared by
multiple consumers, for example:

- schema and spec data models
- tool input/output parsing helpers
- tool metadata and compatibility shims that do not depend on `codex-core`
- other narrowly scoped utility code that multiple crates need

The corresponding non-goals are just as important:

- do not move `codex-core` orchestration here prematurely
- do not pull `Session` / `TurnContext` / approval flow / runtime execution
  logic into this crate unless those dependencies have first been split into
  stable shared interfaces
- do not turn this crate into a grab-bag for unrelated helper code

## Migration approach

The expected migration shape is:

1. Move low-coupling tool primitives here.
2. Switch non-core consumers to depend on `codex-tools` directly.
3. Leave compatibility-sensitive adapters in `codex-core` while downstream
   call sites are updated.
4. Only extract higher-level tool infrastructure after the crate boundaries are
   clear and independently testable.

That means it is normal for `codex-core` to temporarily re-export types or
helpers from `codex-tools` during the transition.

## Crate conventions

This crate should start with stricter structure than `core/src/tools` so it
stays easy to grow:

- `src/lib.rs` should remain exports-only.
- Business logic should live in named module files such as `foo.rs`.
- Unit tests for `foo.rs` should live in a sibling `foo_tests.rs`.
- The implementation file should wire tests with:

```rust
#[cfg(test)]
#[path = "foo_tests.rs"]
mod tests;
```

If this crate starts accumulating code that needs runtime state from
`codex-core`, that is a sign to revisit the extraction boundary before adding
more here.
