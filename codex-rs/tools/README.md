# codex-tools

`codex-tools` is the shared support crate for building, adapting, planning, and
executing model-visible tool sets outside `codex-core`.

Today this crate owns the host-facing tool models and helpers that no longer
need to live in `core/src/tools/spec.rs` or `core/src/client_common.rs`:

- aggregate host models such as `ToolSpec`, `ConfiguredToolSpec`,
  `LoadableToolSpec`, `ResponsesApiNamespace`, and
  `ResponsesApiNamespaceTool`
- host config and discovery models used while assembling tool sets, including
  `ToolsConfig`, discoverable-tool models, and request-plugin-install helpers
- host adapters such as schema sanitization, MCP/dynamic conversion, code-mode
  augmentation, and image-detail normalization
- shared executable-tool contracts such as `ToolExecutor`, `ToolCall`, and
  `ToolOutput`

That extraction is the first step in a longer migration. The goal is not to
move all of `core/src/tools` into this crate in one shot. Instead, the plan is
to peel off reusable pieces in reviewable increments while keeping
compatibility-sensitive orchestration in `codex-core` until the surrounding
boundaries are ready.

## Vision

Over time, this crate should hold host-side tool machinery that is shared by
multiple consumers, for example:

- host-visible aggregate tool models
- tool-set planning and discovery helpers
- MCP and dynamic-tool adaptation into Responses API shapes
- code-mode compatibility shims that do not depend on `codex-core`
- other narrowly scoped host utilities that multiple crates need

The corresponding non-goals are just as important:

- do not move `codex-core` orchestration here prematurely
- do not pull `Session` / `TurnContext` / approval flow / runtime execution
  logic into this crate unless those dependencies have first been split into
  stable shared interfaces
- do not turn this crate into a grab-bag for unrelated helper code

## Migration approach

The expected migration shape is:

1. Keep extension-owned executable-tool authoring in `codex-extension-api`.
2. Move host-side planning/adaptation helpers here when they no longer need to
   stay coupled to `codex-core`.
3. Leave compatibility-sensitive adapters in `codex-core` while downstream
   call sites are updated.
4. Only extract higher-level host infrastructure after the crate boundaries are
   clear and independently testable.

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
