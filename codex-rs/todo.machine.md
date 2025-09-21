# MCP Management Overhaul – Phase 3 (Wizard & UX)

## Objective
Ship a feature-flagged MCP wizard and dashboard that meet these success criteria:
- ≥95% of guided CLI/TUI sessions reach a valid config within 5 minutes (measured via telemetry or manual timing).
- Health check command returns a structured result for every configured server.
- CLI/TUI/JSON outputs stay in sync (no divergence bugs in manual QA).
- No plaintext secrets written to disk during wizard flows.

## Preconditions
- Schema extensions, migrations, and CLI migrate command ✅
- `experimental.mcp_overhaul` flag wired into config ✅

## Deliverables
1. **Templates & Registry (Success when…)**
   - `resources/mcp_templates/*.json` exist with schema validation.
   - `codex_core::mcp::registry` supports create/update/delete/list using templates.
   - Policy hooks (command allowlist stub + env warning) execute during registry ops.

2. **CLI Wizard (Success when…)**
   - `codex mcp wizard` (flagged) walks through template selection, validation, preview, final apply.
   - `codex mcp add --template … --set …` works headless and writes identical config.
   - `codex mcp list/get` show health summary when flag on.
   - Non-interactive wizard: `codex mcp wizard --name foo --command bar --apply` persists entry via registry.
   - Interactive wizard after confirmation writes entry and re-runs summary on success.
   - `--json` path returns machine summary without side effects.

3. **TUI Panel (Success when…)**
   - New panel lists servers + status.
   - Wizard modal mirrors CLI flow; snapshot tests updated.

4. **Health Probe Stub (Success when…)**
   - `codex mcp test <name> [--json]` returns cached/placeholder status without panic.

5. **Automation Hooks (Success when…)**
   - `codex mcp plan --json` emits validation summary suitable for CI.

6. **Documentation & Guardrails (Success when…)**
   - `docs/config.md` updated with flag instructions + wizard quickstart.
   - CLI help (`--help`) references experimental gate.
   - Running wizard/test without flag yields explicit guidance.

## Validation Checklist
- Unit tests: template parsing, registry validation, wizard step transitions.
- CLI integration tests: `codex mcp wizard --json`, apply path writes config.
- TUI snapshot: panel, wizard flows.
- Manual QA: CLI happy path + failure, TUI flow, automation commands.
- Telemetry/mock timing: confirm ≤5 min setup goal (manual timing if telemetry absent).
- Secrets audit: ensure wizard never leaves secrets in plain config.

## Progress — 2025-09-17
- TUI manager modal + wizard flow implemented (`codex-rs/tui/src/mcp/*`).
- App events for open/apply/reload/remove integrated with `McpRegistry` in `tui/src/app.rs`.
- Snapshot test scaffolding for manager & wizard views added (pending `cargo insta accept`).
- Health status stub exposed via `codex_core::mcp::health` and surfaced in manager list.
