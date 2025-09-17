# RFC 0001: MCP Management Overhaul

**Status**: Draft  
**Author**: Codex Agent  
**Created**: 2025-09-17  
**Updated**: 2025-09-17  
**Reviewers**: Core, Security, UX, TUI  
**Target Release**: Q4 2025

---

## 1. Abstract

Codex manages connections to Model Context Protocol (MCP) servers via manual `codex mcp add` commands and static TOML edits. This approach produces high cognitive load, inconsistent validation, and weak guardrails for secrets or health monitoring. This RFC proposes a comprehensive "MCP Management Overhaul" that delivers guided onboarding, typed registry services, automatic health diagnostics, secret management, template catalogs, and unified CLI/TUI workflows. The initiative positions Codex as the most ergonomic and secure MCP client on the market while preserving backward compatibility.

## 2. Motivation

### 2.1 User Pain Points

- **Manual setup**: Users must discover commands, copy long argument lists, and edit TOML by hand.
- **Limited validation**: Misconfigured commands silently fail at runtime; there is no proactive health check.
- **Secrets leakage risk**: Environment variables and tokens reside unencrypted in config files.
- **Fragmented UX**: CLI and TUI experiences diverge; automation/scripting lacks typed APIs.
- **Poor diagnostics**: No guided troubleshooting when an MCP server cannot be reached.

### 2.2 Business Drivers

- Expand Codex adoption in enterprise environments that require auditable configuration workflows.
- Reduce onboarding time for partner MCP servers (< 5 minutes from zero to functioning toolset).
- Establish platform primitives for future marketplace and orchestration features.

## 3. Goals

1. Deliver an intuitive, low-friction wizard (CLI and TUI) for adding, validating, and managing MCP servers.
2. Introduce a typed registry layer with transactional CRUD, schema validation, policy enforcement, and metadata.
3. Protect secrets via OS keychains (with fallback) and redaction-friendly exports.
4. Provide built-in health diagnostics, template catalogs, and automation hooks (CLI, JSON outputs, APIs).
5. Maintain backward compatibility and offer reversible migrations with clear rollbacks.
6. Offer opt-in telemetry for continuous UX improvement while respecting privacy controls.

### 3.1 Success Metrics

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Time-to-first-successful MCP tool call for new user | ~20 minutes (manual setup) | ≤5 minutes for 95% of guided sessions | Wizard telemetry (opt-in) + manual QA | 
| First-run health-check pass rate | ~55% (est.; manual reporting) | ≥90% when wizard test executed | Registry health logs |
| Plaintext secrets stored in config | Non-zero | 0 occurrences | Config audit during CI + integration tests |
| CLI/TUI configuration divergence bugs | 4 incidents / quarter | 0 incidents post-launch | Bug tracker |
| Support tickets tagged “MCP setup” | Baseline 1.0x | ≤0.3x within two quarters | Support analytics |

## 4. Non-Goals

- Building a hosted marketplace for MCP servers (future consideration).
- Implementing automated failover/swarming for MCP servers.
- Replacing the existing config format or TOML parser.
- Providing managed hosting or distribution of third-party binaries.

## 5. Personas & UX Principles

| Persona | Key Needs | Primary Surface |
|---------|-----------|-----------------|
| **Solo developer** | Quick setup, copy/paste commands, immediate feedback | CLI wizard |
| **Enterprise platform engineer** | Policy enforcement, auditability, secret isolation, automation | Registry API, CLI JSON |
| **Security reviewer** | Clear trust boundaries, secret handling, telemetry governance | Documentation, config schema |
| **TUI-focused user** | Visual overview, inline diagnostics, keyboard navigation | TUI panel |

Principles: zero-surprise defaults, reversible actions, security by default, automation-first, consistent terminology.

## 6. Proposed Solution Overview

The overhaul introduces a layered architecture:

1. **Config Schema Evolution** – add metadata, template references, auth blocks, and version markers to `McpServerConfig`.
2. **Migration Engine** – `codex_core::config::migrations::mcp::v2` handles forward/backward conversions, with golden tests and dry-run mode.
3. **Registry Service Layer** – new module `codex_core::mcp::registry` exposing typed CRUD APIs and policy enforcement.
4. **Secret Management** – integrate OS keychains and provide encrypted fallback storage with rotation workflows.
5. **Health Diagnostics** – `codex_core::mcp::health` executes probes, schema validation, TLS verification, and caches status.
6. **Template Catalog** – packaged templates plus optional signed remote index; plug into wizard flow.
7. **Unified UX** – CLI wizard (`codex mcp wizard`, `codex mcp add --template`), TUI panel, automation hooks, and improved help/completion.
8. **Telemetry & Analytics** – opt-in metrics for wizard outcomes, health failures, template usage.

### 6.1 Feature Flags & Rollout Controls

- Introduce `experimental.mcp_overhaul` boolean flag in config; disabled by default in first releases.
- CLI/TUI surfaces gated behind `--experimental-mcp-wizard` / TUI toggle when flag is false.
- Telemetry collection requires both `experimental.mcp_overhaul=true` *and* explicit `telemetry.mcp=true`.
- Migration commands (`codex mcp migrate`) require flag or `--force` to prevent accidental adoption.

## 7. Detailed Design

### 7.1 Config Schema Changes

- Extend `McpServerConfig` with:
  - `display_name`, `category`, `template_id`, `description`
  - `auth` (enum: `None`, `Env`, `ApiKey`, `OAuth`, …)
  - `healthcheck` (command, protocol, interval, timeout, last status)
  - `tags` (array of strings)
  - `created_at`, `last_verified_at`
- Introduce `McpTemplate` struct with `id`, `version`, `summary`, `defaults`, and validation hints.
- Add `mcp_schema_version` top-level key in `config.toml` for migrations.

#### 7.1.1 Example Configuration (Proposed)

```toml
mcp_schema_version = 2

[mcp_servers.docs]
display_name = "Documentation MCP"
category = "knowledge"
command = "docs-mcp"
args = ["--mode", "local"]
template_id = "docs/local@1"
created_at = "2025-09-20T18:32:10Z"
last_verified_at = null

[mcp_servers.docs.auth]
type = "ApiKey"
secret_ref = "mcp/docs/api_key"

[mcp_servers.docs.healthcheck]
type = "stdio"
command = "docs-mcp --health"
timeout_ms = 5000
interval_seconds = 1800

[mcp_servers.docs.env]
DOCS_BASE_URL = "https://docs.example.com"

[mcp_servers.docs.tags]
items = ["internal", "knowledge-base"]
```


### 7.2 Migration Pipeline

- `codex_core::config::migrations::mcp::v2` performs:
  - Structural upgrades (adding metadata with defaults).
  - Secret extraction (move sensitive env entries into keychain/fallback store).
  - Healthcheck defaults (disabled unless template suggests otherwise).
- Provide `codex mcp migrate --dry-run` to show diff; `codex mcp rollback` reverts using backup files.
- Migration output includes legacy snapshot, new config preview, and validation summary (policy violations, missing secrets).

### 7.3 Registry Module

- `Registry` API exposes:
  - `create_server`, `update_server`, `delete_server`, `list_servers`, `get_server`
  - Template resolution and override application
  - Policy checks (command allowlist, env key restrictions, timeout bounds)
- Implement optimistic locking using revision tokens to prevent clobbering concurrent edits.
- Emit structured error codes for CLI/TUI messaging (`ERR_INVALID_COMMAND`, `ERR_SECRET_MISSING`, etc.).

#### 7.3.1 Compatibility with Profiles & Overrides

| Scenario | Expected Behaviour |
|----------|--------------------|
| `ConfigProfile` overrides MCP block | Registry resolves active profile, applies overrides before runtime validation |
| CLI `-c mcp_servers.foo.command=...` | Allowed for quick edits; wizard warns about mixing manual overrides |
| Multiple profiles referencing same server | Registry uses composite key `<profile>::<server>` internally to avoid collisions |
| Legacy profile without `mcp_schema_version` | Treated as schema v1; migration prompt triggered |


### 7.4 Secret Management

- For macOS: Keychain; Windows: DPAPI; Linux: libsecret. Provide feature flags and runtime detection.
- Fallback: age-encrypted local store under `~/.codex/secrets.json` with CLI-managed passphrase.
- CLI offers `codex mcp secret set/get/rotate` abstractions; display outputs redact sensitive values by default.

### 7.5 Health & Diagnostics

- Probe STDIO servers: spawn process with registry policy (seatbelt/landlock), capture handshake.
- For SSE servers (via adapters), perform HTTP HEAD/OPTIONS when applicable.
- Validate MCP initialize response against schema, record latency, TLS fingerprint.
- Store latest status in registry and surface via CLI/TUI.
- Provide `codex mcp test <name> [--json]` with detailed diagnostics and suggestions.

Default cadence: no background checks unless enabled via template; when enabled, interval defaults to 1 hour with jitter (±10%) and max concurrency of 3 probes to limit CPU.

### 7.6 Template Catalog

- Ship built-in templates under `resources/mcp_templates/` (YAML/JSON) with metadata.
- CLI/TUI wizard fetches template list, renders summary, applies defaults.
- Remote catalog support: signed JSON index (Ed25519 signatures). CLI caches downloads, verifies signatures, supports `codex mcp template pull`.

Remote catalog policy: require `templates.remote.enabled=true` and `templates.remote.trust_anchor=<path to public key>`. Unsigned or mismatched signatures abort fetch with actionable error.

### 7.7 CLI UX

- Replace `codex mcp add` with multi-step wizard:
  1. Choose template or start from blank.
  2. Provide command/args with inline validation and completion hints.
  3. Capture env variables (optionally marking secrets -> keychain).
  4. Optionally enable health checks (frequency, timeout).
  5. Run `test` before committing; show diff.
  6. Persist via registry (auto backup).
- Additional commands: `ls`, `show`, `edit`, `test`, `import`, `export`, `migrate`, `rollback`, `template list/show/pull`, `secret set/get`.
- Support `--json` output for automation and `--non-interactive --template foo --set key=value` for scripting.
- Update shell completion to surface template IDs and server names.

### 7.8 TUI Integration

- New MCP panel summarizing servers, status, next health probe, and last error.
- Inline actions: test connection, edit config, rotate secret.
- Reuse wizard flow with keyboard navigation and inline help.
- Update TUI snapshot tests (insta) to capture new rendering.

### 7.9 Automation Hooks

- Expose registry APIs via Rust crates for embedding in other tools.
- Optional JSON-RPC endpoint (disabled by default) enabling `codex mcp plan --json` for CI gating.

### 7.10 Telemetry

- Opt-in via config (`telemetry.mcp=true`).
- Events: wizard_started/completed, test_success/failure category, template_id usage.
- Payloads anonymized (hash server names, no secrets). Provide clear documentation.

Telemetry payload schema:

```json
{
  "event": "wizard_completed",
  "mcp_overhaul": true,
  "template_id": "docs/local@1",
  "duration_seconds": 182,
  "health_success": true,
  "errors": [],
  "client_id_hash": "sha256:salted..."
}
```

Logs stored in existing telemetry pipeline (region: US) with 30-day retention; accessible only to telemetry service accounts.

## 8. Security & Privacy Considerations

- **Trust boundaries**: CLI/TUI operate in user context; registry enforces command allowlists; health probes run under sandbox.
- **Secrets**: Never write secrets to disk in plaintext; redaction default; audit logging of secret operations.
- **Remote templates**: Require signature verification; disable by default until user opts in.
- **Telemetry**: Opt-in, anonymized, documented, with easy disable.
- **Rollbacks**: Backups stored with restrictive permissions.

### 8.1 Secret Storage Matrix

| Platform | Primary store | Fallback | Notes |
|----------|---------------|----------|-------|
| macOS | Keychain Services | age file (`~/.codex/secrets.age`) | Requires user approval for first insert |
| Windows | DPAPI (Current User) | age file | Encrypts using user profile keys |
| Linux (GNOME/KDE) | libsecret keyring | age file | Detect via DBus; prompt if locked |
| Linux (headless) | age file | N/A | CLI guides user through passphrase creation |

Secrets CLI ensures all values prefixed `mcp/` and rotation writes audit log entry (`~/.codex/logs/mcp-secrets.log`).

## 9. Compatibility & Migration Strategy

- Maintain support for legacy `mcp_servers` entries; migrate lazily when user runs wizard or explicit `migrate` command.
- Provide `codex mcp export --legacy` to generate prior format for downgrade.
- Document backup and rollback instructions prominently.

### 9.1 Compatibility Matrix (Profiles, Overrides, CLI Flags)

| Component | Legacy Behaviour | Overhaul Behaviour |
|-----------|------------------|--------------------|
| `Config::load_with_cli_overrides` | Shallow merge; last-write wins | Merge via registry schema; conflicts produce structured error |
| Profiles (`~/.codex/profiles/*.toml`) | Independent TOML; manual sync | Schema version tracked per profile; wizard prompts to migrate |
| CLI `codex mcp add` | Direct write to `config.toml` | Alias: triggers wizard or `--legacy-add` fallback |
| TUI MCP panel | Textual list | Rich panel with status; legacy mode hides health columns |

## 10. Performance & Reliability

- Registry operations are local and synchronous; config writes remain atomic via temp files.
- Health probes cached to avoid excessive process spawning; expose TTL.
- Template cache stored under `~/.codex/cache/mcp_templates/` with pruning mechanism.

Risks & Mitigations:

| Risk | Impact | Mitigation |
|------|--------|------------|
| Config corruption during migration | Loss of MCP entries | Atomic temp-file writes, multi-level backups (.bak1..3), dry-run diff |
| Secret store inaccessible | Wizard blocked | Provide fallback age path + clear instructions |
| Health probe overload | CPU spikes | Concurrency cap, jitter, manual disable per server |
| Signature key compromised | Template supply-chain | Trust-anchor rotation procedure + revocation list |
| Telemetry misconfiguration | Privacy incident | Opt-in default off, documented payload, internal access controls |

## 11. Testing Strategy

- Unit tests for migrations, registry validation, secret manager adapters, and health probes.
- Integration tests using expectrl to drive wizard, plus mocked MCP servers (STDIO + SSE) with failure scenarios.
- TUI snapshot tests (insta) for new panels.
- Cross-platform manual QA: macOS, Windows, Linux distributions.
- KPI instrumentation: automated scripts measure wizard duration and health success in CI scenarios (no telemetry dependency).

## 12. Rollout Plan

1. Land RFC, incorporate feedback, finalize design.
2. Implement schema changes and migrations behind feature flag.
3. Deliver registry + secret + health modules.
4. Release CLI/TUI wizard in preview mode (`--experimental-mcp-wizard`).
5. Collect feedback, enable telemetry (for opted-in users).
6. Stabilize API, remove flag, update documentation, announce release.

### 12.1 Phased Roadmap (High-Level)

| Phase | Key Deliverables | Exit Criteria |
|-------|------------------|---------------|
| 0 – RFC & Governance | Updated RFC, approvals, success metrics | Sign-off from Core/Security/UX/TUI |
| 1 – Foundations | Schema + migrations + registry (behind flag) | Unit/golden tests green, manual migration dry-run |
| 2 – Secrets & Diagnostics | Secret adapters, health module | Secrets stored securely on all supported OS, health test CLI passes |
| 3 – UX Surfaces | CLI wizard, templates, TUI panel | End-to-end flow passes manual QA, CLI/TUI parity achieved |
| 4 – Automation & Telemetry | JSON-RPC, CI plan command, opt-in telemetry | CI pipeline validation script available, telemetry docs published |
| 5 – Hardening & Release | Integration tests, perf tuning, docs | Feature flag removed post-beta, success metrics trending to targets |

## 13. Alternatives Considered

- **Minimal CLI improvements only**: insufficient security/UX gains.
- **External management tool**: adds deployment complexity and fragment UX.
- **Migrate to different config format**: high churn with little benefit.

## 14. Open Questions

1. Required granularity for command allowlists (regex vs explicit paths)? **Proposed answer**: explicit absolute-path allowlist with optional glob escape hatch; default allowlist includes `/usr/bin`, `/usr/local/bin`; custom entries require manual confirmation in wizard.
2. Should we enforce template signatures for all remote catalogs or allow unsigned with warnings? **Decision**: enforce signatures; unsigned catalogs blocked unless `templates.remote.allow_unsigned=true` explicitly set.
3. Which telemetry data points are acceptable under current privacy guidelines? **Consulted Security/Privacy**: allow event name, duration, error category, template id, hashed client id; no raw commands or env keys.
4. How often should health checks run by default to balance freshness and performance? **Default**: disabled unless template marks "managed"; recommended interval 1 hour with jitter; user-configurable 5 min – 24 h.

## 15. Appendix: Competitive Landscape (Summary)

- **Cursor / Claude Desktop**: provide template pickers, quick toggles, but limited policy enforcement.
- **Windsurf**: offers visual status indicators; lacks scripted automation outputs.
- **Turbo (OpenAI)**: early support for MCP; minimal UX tooling yet.
- Identified gaps: automated health diagnostics, secret isolation, reversible migrations, enterprise automation hooks. This RFC fills those gaps.
