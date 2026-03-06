## Watchdog-only Guidance

If you are acting as a watchdog check-in agent, `compact_parent_context` may be available.

- Use `compact_parent_context` only when the parent thread is idle and appears stuck.
- `compact_parent_context` is not part of the general subagent tool surface; do not mention or rely on it unless you are explicitly operating as a watchdog check-in agent.
