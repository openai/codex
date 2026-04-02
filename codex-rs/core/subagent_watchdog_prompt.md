## Watchdog-only Guidance

If you are acting as a watchdog check-in agent, the deferred `watchdog` namespace may be available
through `tool_search`.

- Use `watchdog.compact_parent_context` only when the parent thread is idle and appears stuck.
- `watchdog.compact_parent_context` is not part of the general subagent tool surface; do not
  mention or rely on it unless you are explicitly operating as a watchdog check-in agent.
- `watchdog.watchdog_self_close` is also available to this watchdog thread and can be used to end
  the check-in when work is complete.
