# 2025-10-23 SemVer Sync & Release Build Improvements Log

## Overview
- Aligned all workspace/package metadata with the upstream OpenAI/codex semantic version (`0.48.0`) by stripping the `-zapabob.1` suffix.
- Propagated the version change across Rust workspace manifests, npm package metadata, global installer messaging, and user-agent strings to ensure consistent branding.
- Introduced a shared `resolve_runtime_budget` helper for CLI commands so runtime token budgets inherit the canonical integer type and clamping logic without duplication.
- Brought the `research_cmd` Gemini integration back in line with the upstream API surface (using existing constructors) while retaining informative logs for MCP fallbacks.
- Added the missing `codex-stdio-to-uds` dependency reference so the CLI links its stdio tunnel helper during release builds.
- Normalised the DeepResearch web search user-agent string to the upstream format.

## Details
- `codex-rs/Cargo.toml`: Set `[workspace.package] version = "0.48.0"`.
- `codex-cli/package.json`: Bumped `version` to `"0.48.0"` for the npm artefact.
- `scripts/install/global-install.ps1`: Report the installed version dynamically instead of the hard-coded `0.47.0-alpha.1`.
- `codex-rs/cli/src/lib.rs`: Added `resolve_runtime_budget` utility used by all CLI subcommands that instantiate `AgentRuntime`.
- `{agent_create_cmd.rs, delegate_cmd.rs, parallel_delegate_cmd.rs}`: Switched to the helper and kept token budgets as `i64`, matching upstream expectations.
- `codex-rs/cli/Cargo.toml`: Declared `codex_stdio_to_uds` workspace dependency to restore linkage.
- `codex-rs/cli/src/research_cmd.rs`: Replaced unsupported Gemini constructors with `new`/`with_mcp_client` pattern while logging MCP intention; maintained provider fallback order.
- `codex-rs/deep-research/src/web_search_provider.rs`: Updated the HTTP user-agent to `Mozilla/5.0 Codex-DeepResearch/0.48.0`.

## Validation
- `cargo build --release` (workspace root) — ✅ (after dependency + helper fixes).
- `scripts/install/global-install.ps1` — ✅ produces globally installed CLI reporting `codex-cli 0.48.0`.
