# Rust/codex-rs

In the `codex-rs` folder where the Rust code lives:

- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`. You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.
 - Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR`. You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.

## Native MCP-Based Agent Orchestration

We leverage the Model Context Protocol (MCP) crates (`mcp-client`, `mcp-server`, `mcp-types`) as a process boundary and plugin interface for agents and tools. Each agent is a standalone executable speaking JSON-RPC over stdio. The orchestrator lives natively in Rust under `codex-rs`:
 - Agents are described by YAML specs under `agents/`, specifying name, model, system prompt templates, allowed tools, and cost budgets.
 - On startup, the orchestrator reads all agent specs, spawns each via `McpClient::new_stdio_client`, and wires them together using Tokio channels.
 - System prompts are rendered from templates that enumerate each agent’s sandbox environment and available tools.
 - Testing uses in-memory duplex streams to validate `list_tools`, `call_tool`, streaming notifications (`codex/event`), batch requests, and patch application via MCP.

## Conflict Detection (apply-patch)

To ensure robust patch application, we will:
1. Pre-validate overlapping hunks before applying them by mapping each hunk’s context to absolute line ranges. If two hunks’ ranges overlap, return a dedicated `Conflict` error variant in `ApplyPatchError`.
2. Continue to derive unified diffs for disjoint hunks and provide the resulting content via `unified_diff_from_chunks`.
3. Optionally fallback to a three-way merge (using an external diff3 library or VCS base) when conflicts are detected.
4. Expose automated or LLM-driven conflict resolution hooks (e.g. `tools/resolve_conflict`) that accept conflicting hunks, project context, and produce merged patches.
