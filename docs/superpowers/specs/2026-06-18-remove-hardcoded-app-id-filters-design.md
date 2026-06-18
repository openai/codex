# Remove Hardcoded App ID Filters

## Status

Approved on 2026-06-18.

## Problem

Codex carries the same originator-dependent connector ID denylist in two crates. One copy filters connector directory and accessibility results; the other filters live and cached Codex Apps MCP tools. The opaque IDs have no in-code ownership or expiration information, and the duplicate lists can drift.

## Scope

Remove the seven-ID denylist and all plumbing used only to apply it.

Preserve:

- server-driven directory visibility, including `visibility == "HIDDEN"`;
- plugin discoverability and accessible/enabled-state filtering;
- normal MCP tool configuration filters;
- consequential-tool approval message templates keyed by connector ID.

## Design

### Connector listings

Delete the denylist constants and `filter_disallowed_connectors`. Connector directory and accessible-connector results will no longer pass through an ID policy. Simplify `filter_tool_suggest_discoverable_connectors` by removing its originator argument while retaining its checks for accessibility and plugin-backed discoverability.

Update connector consumers to use the merged or cached connector results directly. This removes originator-specific behavior from connector listing without changing the remaining merge, sort, visibility, accessibility, or enabled-state logic.

### Codex Apps MCP tools

Delete the duplicate denylist and `is_connector_id_allowed` helper from `codex-utils-plugins`. Remove `filter_disallowed_codex_apps_tools` and its live-list and disk-cache call sites from `codex-mcp`.

Cached and freshly listed tools will therefore have identical allow-all-by-connector-ID behavior. Existing MCP tool-name, configured-tool, approval, and capability filtering remains unchanged.

Remove dependencies that become unused as a direct result of deleting the policy and refresh the Cargo and Bazel lockfiles when required.

## Testing

Use regression tests that start with formerly blocked connector IDs and prove they survive:

- connector discovery/list processing;
- live Codex Apps MCP tool conversion;
- Codex Apps MCP disk-cache write/read behavior.

Run focused tests for every changed crate, formatting, scoped lint fixes, dependency lock checks when applicable, and the repository-required broader test suite with user approval if shared crates are changed.

## Risk and Rollback

The intended behavior change is that formerly denied apps and tools can appear when returned by the service. Service visibility and authorization remain authoritative. The change is reversible by reverting the commit; no stored data or API schema migration is involved.
