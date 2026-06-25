# Context

## Intake
- Source: `/home/dev-user/code/codex`, refreshed `origin/main` at `cef5444a80ac5a94d435ab780fba5d6f433c504f`.
- Target: `/home/dev-user/code/codex-dead-integrations-cleanup`, branch `cleanup/dead-integration-code`.
- Scope: remove demonstrably dead Rust structs, methods, fields, locals, and related suppressions in MCP, apps, connectors, plugins, and skills code.
- Preserve: public/protocol/serialized surfaces, platform-specific code, test helpers, and behavior unless usage evidence and compiler/linter validation establish deadness.

## Repo Map
- Rust workspace: `codex-rs/`.
- Integration crates: `codex-mcp`, `mcp-server`, `rmcp-client`, `connectors`, `plugin`, `core-plugins`, `skills`, `core-skills`.
- Cross-cutting consumers: `core`, `app-server`, `app-server-protocol`, `tui`, and `cli`.

## Key Paths
- `codex-rs/Cargo.toml`
- `codex-rs/core/src/`
- `codex-rs/app-server/src/`
- `codex-rs/app-server-protocol/src/`

## Constraints
- Keep the cleanup behavior-neutral.
- Require repository-wide reference searches before deleting public-looking items.
- Run formatting, targeted tests/checks, and workspace-level lint/check gates proportionate to the diff.

## Assumptions
- "Codex apps" includes app-related integration plumbing, not the whole app-server implementation.
- Generated code and intentionally retained compatibility surfaces are out of scope.

## Open Questions
- Exact candidate set will be compiler/search driven during Cycle 0.
