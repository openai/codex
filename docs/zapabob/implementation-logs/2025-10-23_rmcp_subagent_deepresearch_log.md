# 2025-10-23 rmcp subagent + deep-research restoration log

## Summary
- Updated `AgentRuntime::filter_codex_mcp_tools` to recognise `codex-subagent`, `codex-deep-research`, and fully-qualified MCP tool names.
- Expanded `build_codex_mcp_tools_description` so agents see guidance for subagent, deep research, supervisor, custom command, hook, and auto-orchestrate tools.
- Refreshed unit coverage to assert the new filtering behaviour and the presence of the extended tool descriptions.

## Notes
- Hyphenated Codex MCP tool names now survive filtering, restoring subagent and deep research flows when running through rmcp.
