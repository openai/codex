## Overview
`codex_tool` exercises the MCP server’s Codex tool integration end to end, covering shell approvals, patch approvals, configuration enforcement, and workspace restrictions.

## Detailed Behavior
- Spawns `codex-mcp-server` via the shared `McpProcess` harness and interacts with it over JSON-RPC, while WireMock streams SSE responses from a mock ChatGPT backend.
- Key scenarios include:
  - `test_shell_command_approval_triggers_elicitation`: untrusted shell commands trigger exec approval elicitations; approving runs the command and confirms side effects (file creation), while rejecting returns denied errors.
  - `shell_command_auto_approved_runs_without_elicitation`: trusted commands bypass approval.
  - `test_patch_apply_triggers_elicitation`: proposed file changes generate patch approval elicitations that reference affected paths; approvals propagate to the mock backend.
  - `test_patch_apply_with_grant_root`: verifies grant-root hints in elicitations when Codex provides `grant_root`.
  - `test_base_instructions_forwarded`: ensures base instructions in tool calls are forwarded to the backend.
  - Additional helpers assert Codex-specific metadata (event IDs, call IDs) round-trip correctly.
- Utility functions handle config generation, expected elicitation construction, and setup/teardown of mock servers and temp directories.

## Broader Context
- Provides regression coverage for the approval flows detailed in the MCP server specs, ensuring the JSON-RPC contract and SSE-to-elicit conversions remain stable.

## Technical Debt
- Tests duplicate some helper logic found in app-server suites (e.g., SSE builders). Consolidating MCP-specific fixtures could reduce duplication.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract shared MCP approval fixture helpers to reduce duplication across suites and keep elicitation expectations centralized.
related_specs:
  - ../mod.spec.md
  - ../../mod.spec.md
  - ../../src/codex_tool.rs.spec.md
