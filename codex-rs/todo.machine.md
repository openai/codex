# MCP Management Overhaul – Phase 3 (Wizard & UX)

## Objective
Implement feature-flagged MCP wizard and template-driven workflows across CLI/TUI while maintaining automation parity and health/status visibility.

## Preconditions
- Schema extensions, migrations, and CLI migrate command (completed).
- `experimental.mcp_overhaul` gate wired into config loading (completed).

## Phased Tasks
1. **Templates & Registry Integration**
   - Define built-in templates under `resources/mcp_templates/*.json` with schema validation helpers (`McpTemplate` parsing).
   - Extend `codex_core::mcp::registry` (new module) to expose typed CRUD operations, template resolution (apply defaults, prompts for missing fields), and policy checks (command allowlist stub, env key validation warnings).
   - Implement in-memory health status cache placeholder to surface last probe result.

2. **CLI Wizard**
   - Add `codex mcp wizard` command (feature-gated) with stepper: select template / custom → fill command/env/auth → optional health settings → preview diff → dry-run test call (stub).
   - Support non-interactive mode: `codex mcp add --template foo --set key=value --set env.KEY=VALUE`.
   - Reuse registry APIs for create/update; ensure rollback-on-error semantics.
   - Update list/get outputs to include health summary when flag enabled.

3. **TUI Panel**
   - Create MCP management panel (behind flag) listing servers, status, last check, actions (test, edit, remove).
   - Integrate wizard flow via modal/prompt system; add snapshot tests (`insta`).

4. **Health Probe Stub**
   - Wire placeholder `codex mcp test <name>` to call registry health (currently stubbed) so UX flows end-to-end.
   - Provide JSON output for automation.

5. **Automation Hooks**
   - Add `codex mcp plan --json` to emit registry state + validation warnings (feature-gated).

6. **Documentation & Guardrails**
   - Update `docs/config.md` with experimental flag instructions and wizard preview.
   - Clearly note feature flag in CLI help (`--help`).

## Guardrails
- All new surfaces check `config.experimental_mcp_overhaul`; non-enabled state prints actionable guidance.
- Wizard commands must respect `--json` (no interactive prompts) with helpful error when run without flag.
- No default telemetry until Phase 4.

## Validation
- Unit tests for template parsing, registry safeguards, CLI wizard step logic.
- TUI snapshots for panel and wizard modals.
- Manual QA script covering CLI wizard (Happy path/validation failure) and TUI panel navigation.
