# Codex Code Spec Index

This index tracks the Markdown “spec” files that live next to the Rust sources in `codex-rs`. Every entry links back to the canonical spec so you can review behaviour, invariants, and technical debt before diving into the code itself.

## How to Read These Specs
- Specs are colocated with the source (`*.rs.spec.md`) and cover real behaviour, not future work.
- Directory overviews use `mod.spec.md`; file-level specs sit beside their Rust modules.
- Each spec ends with a small YAML footer summarising local tech debt and related specs.
- When adding or changing APIs, update the relevant spec and make sure this index links to it.

## Coverage by Phase
| Phase | Scope | Status | Notes |
| --- | --- | --- | --- |
| Phase 0 – Workspace Foundations | Shared types (`common`, `protocol`, `mcp-types`, `codex-backend-openapi-models`, `protocol-ts`) | ✅ Complete | All foundational crates have linked specs. |
| Phase 1 – Core Orchestration & Execution | Core runtime (`core`, execution, configuration/auth) | ⚠️ In Progress | Major modules specced; integration/tests & auth stragglers remain. |
| Phase 2 – Interfaces & Services | CLI, TUI, service entrypoints | ⚠️ In Progress | TUI fully covered; app-server/mcp-server deeper internals still pending. |
| Phase 3 – Tools & External Integrations | Tooling crates, clients, telemetry, IPC | ✅ Complete | Specs cover file search, git tooling, rmcp client, feedback/otel, etc. |
| Phase 4 – Utilities, Tests, Scripts | Workspace helpers, harnesses, scripts | ✅ Complete | Utilities, test harnesses, and script specs are in place. |

### Phase 0 – Workspace Foundations (✅)
- [`codex-rs/common/mod.spec.md`](../../codex-rs/common/mod.spec.md) – shared config/CLI helpers.
- [`codex-rs/protocol/mod.spec.md`](../../codex-rs/protocol/mod.spec.md) – protocol enums and payloads.
- [`codex-rs/mcp-types/mod.spec.md`](../../codex-rs/mcp-types/mod.spec.md) – MCP transport models.
- [`codex-rs/codex-backend-openapi-models/mod.spec.md`](../../codex-rs/codex-backend-openapi-models/mod.spec.md) – generated backend schemas & regeneration flow.
- [`codex-rs/protocol-ts/mod.spec.md`](../../codex-rs/protocol-ts/mod.spec.md) – TypeScript bindings parity.

### Phase 1 – Core Orchestration & Execution (⚠️)
Covered highlights:
- [`codex-rs/core/mod.spec.md`](../../codex-rs/core/mod.spec.md), [`codex-rs/core/src/codex.rs.spec.md`](../../codex-rs/core/src/codex.rs.spec.md), [`codex-rs/core/src/exec.rs.spec.md`](../../codex-rs/core/src/exec.rs.spec.md) – dispatcher, session handling, and execution pipeline.
- [`codex-rs/core/src/tools/mod.spec.md`](../../codex-rs/core/src/tools/mod.spec.md) – tool orchestration, registry, router, sandboxing, and parallel execution.
- [`codex-rs/core/src/config.rs.spec.md`](../../codex-rs/core/src/config.rs.spec.md) – configuration persistence, profiles, feature flags.
- [`codex-rs/core/src/command_safety/mod.spec.md`](../../codex-rs/core/src/command_safety/mod.spec.md) – command allowlist/denylist, rollout policy integration.
- [`codex-rs/core/tests/suite/prompt_caching.rs.spec.md`](../../codex-rs/core/tests/suite/prompt_caching.rs.spec.md) – end-to-end coverage for prompt cache reuse and environment-context emission.
- [`codex-rs/core/tests/suite/rmcp_client.rs.spec.md`](../../codex-rs/core/tests/suite/rmcp_client.rs.spec.md) – RMCP stdio/HTTP auth flow integration tests.

Outstanding (needs specs):
- `core/tests/**` integration suites (seatbelt, approvals, unified exec, etc.).

### Phase 2 – Interfaces & Services (⚠️)
Covered:
- [`codex-rs/cli/src/main.rs.spec.md`](../../codex-rs/cli/src/main.rs.spec.md), [`codex-rs/cli/src/login.rs.spec.md`](../../codex-rs/cli/src/login.rs.spec.md) – CLI entrypoints and login flow.
- [`codex-rs/tui/mod.spec.md`](../../codex-rs/tui/mod.spec.md) with full module coverage (see Phase 4 summary below).
- [`codex-rs/app-server/src/lib.rs.spec.md`](../../codex-rs/app-server/src/lib.rs.spec.md), [`codex-rs/app-server/src/message_processor.rs.spec.md`](../../codex-rs/app-server/src/message_processor.rs.spec.md) – REST service wiring.
- [`codex-rs/mcp-server/src/message_processor.rs.spec.md`](../../codex-rs/mcp-server/src/message_processor.rs.spec.md) – MCP server message handling.

Outstanding:
- App-server and MCP-server internal suites (`app-server/tests/suite/*.rs`, `mcp-server/tests/**`).
- `responses-api-proxy` deeper routes/tests.

### Phase 3 – Tools & External Integrations (✅)
- [`codex-rs/apply-patch/mod.spec.md`](../../codex-rs/apply-patch/mod.spec.md) – patch apply engine & CLI.
- [`codex-rs/file-search/mod.spec.md`](../../codex-rs/file-search/mod.spec.md) and [`codex-rs/tui/src/file_search.rs.spec.md`](../../codex-rs/tui/src/file_search.rs.spec.md) – search orchestration.
- [`codex-rs/rmcp-client/mod.spec.md`](../../codex-rs/rmcp-client/mod.spec.md) – RMCP transport, OAuth, helper binaries.
- [`codex-rs/backend-client/mod.spec.md`](../../codex-rs/backend-client/mod.spec.md) – backend HTTP client surface.
- [`codex-rs/feedback/mod.spec.md`](../../codex-rs/feedback/mod.spec.md) & [`codex-rs/otel/mod.spec.md`](../../codex-rs/otel/mod.spec.md) – telemetry plumbing.
- [`codex-rs/ansi-escape/mod.spec.md`](../../codex-rs/ansi-escape/mod.spec.md), [`codex-rs/stdio-to-uds/mod.spec.md`](../../codex-rs/stdio-to-uds/mod.spec.md) – IPC/utilities.

### Phase 4 – Utilities, Tests, Scripts (✅)
- Utilities: [`codex-rs/utils/string/mod.spec.md`](../../codex-rs/utils/string/mod.spec.md), [`codex-rs/utils/tokenizer/mod.spec.md`](../../codex-rs/utils/tokenizer/mod.spec.md), [`codex-rs/utils/pty/mod.spec.md`](../../codex-rs/utils/pty/mod.spec.md).
- Test harnesses: [`codex-rs/app-server/tests/common/mod.spec.md`](../../codex-rs/app-server/tests/common/mod.spec.md), [`codex-rs/core/tests/common/mod.spec.md`](../../codex-rs/core/tests/common/mod.spec.md), [`codex-rs/tui/src/chatwidget/tests.rs.spec.md`](../../codex-rs/tui/src/chatwidget/tests.rs.spec.md).
- TUI utilities & widgets: [`codex-rs/tui/src/custom_terminal.rs.spec.md`](../../codex-rs/tui/src/custom_terminal.rs.spec.md), [`codex-rs/tui/src/diff_render.rs.spec.md`](../../codex-rs/tui/src/diff_render.rs.spec.md), [`codex-rs/tui/src/status/card.rs.spec.md`](../../codex-rs/tui/src/status/card.rs.spec.md), [`codex-rs/tui/src/update_prompt.rs.spec.md`](../../codex-rs/tui/src/update_prompt.rs.spec.md).
- Scripts: [`codex-rs/scripts/create_github_release.spec.md`](../../codex-rs/scripts/create_github_release.spec.md).

## Outstanding Coverage Gaps
Remaining files without specs (per latest `spec_coverage.py` run):
- Core integration suites (`codex-rs/core/tests/**`) and generated openapi models (`codex-backend-openapi-models/src/models/*.rs`).
- Service test suites (`codex-rs/app-server/tests/suite/**`, `codex-rs/mcp-server/tests/**`).
- CLI/TUI integration tests under `codex-rs/cli/tests/**` and `codex-rs/tui/tests/**`.
- Exec policy tests (`codex-rs/execpolicy/tests/**`).

These align with the Phase 1/2 straggler work noted in `plan/codex-rs-documentation-plan.md`. When adding specs for those paths, update this table and migrate the items out of the gap list.

## Next Steps
1. Prioritise Phase 1/2 test and auth/state modules (see gaps list) so core behaviour is documented end-to-end.
2. Build tooling to audit uncovered paths (e.g., ignore generated directories, type-only modules).
3. Once Phase 1/2 are complete, retire `progress_summary.md` and use this index plus the plan for ongoing tracking.
