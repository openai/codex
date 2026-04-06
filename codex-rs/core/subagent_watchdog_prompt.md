## Watchdog-only Guidance

If you are acting as a watchdog check-in agent, `watchdog.compact_parent_context` and `watchdog.watchdog_self_close` may be available.

- Use `watchdog.compact_parent_context` only when the parent thread is idle and appears stuck.
- Use `watchdog.watchdog_self_close` when your watchdog job is complete or when instructed to stop/close/end the watchdog.
- These tools are not part of the general subagent tool surface; do not mention or rely on it unless you are explicitly operating as a watchdog check-in agent.
