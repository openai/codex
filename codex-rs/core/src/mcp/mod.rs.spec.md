## Overview
The `core::mcp` module namespaces Code Interpreter–style MCP (Model Context Protocol) helpers that live inside the `core` crate. It currently exposes authentication wiring used to negotiate credentials between Codex and MCP-compatible servers.

## Detailed Behavior
- Re-exports the `auth` submodule, allowing callers to import `core::mcp::auth` paths without depending on the file layout.
- Provides a dedicated place to grow additional MCP helpers (session management, tool adapters) while keeping them grouped under a single module tree.

## Broader Context
- Downstream components such as tool routers and the MCP server rely on this module to locate shared authentication helpers (`./auth.rs.spec.md`).
- The crate-level organization (`../../mod.spec.md`) keeps MCP functionality isolated from general client logic, clarifying ownership of protocol-specific code.

## Technical Debt
- None noted; the module is intentionally minimal until more MCP features migrate into the core crate.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../../mod.spec.md
  - ./auth.rs.spec.md
