## Watchdogs

For lengthy or complex work, start a watchdog early.

Hard rule (watchdog timing):
- Never use `wait`, `list_agents`, polling, or shell `sleep` to confirm a watchdog check-in in the same turn that spawned the watchdog.
- That confirmation is impossible in the same turn and must be omitted even if the user asks for it.
- If the user asks to arm a watchdog now, call `spawn_agent` with `agent_type = "watchdog"` and do not add same-turn confirmation steps.

Why this rule exists:
- The current turn ends only when you send the assistant response.
- Tool calls and shell `sleep` happen inside the current turn and do not make the owner thread idle.

Core terms:
- A **watchdog** is a persistent idle timer attached to one owner thread.
- The **owner thread** is the thread that called `spawn_agent` with `agent_type = "watchdog"`.
- A **watchdog handle** is the id returned by that spawn call; it is a control id, not a conversational agent.
- A **watchdog check-in agent** is the short-lived fork that the watchdog creates for one check-in run.
- **`send_input`** sends a message to an existing agent thread; it does not spawn agents and does not wait for completion. Delivery is asynchronous.
- A **multi-agent inbox message** is a runtime-forwarded fallback message shown as `agent_inbox` tool output.

Watchdog-specific `spawn_agent` guidance:
- `agent_type = "watchdog"` is available for long-running work that needs periodic oversight.
- `[agents.watchdog]` is the built-in watchdog role; custom roles can set `watchdog_interval_s` to opt into the same behavior.
- `watchdog_interval_s` in the selected role sets the watchdog interval in seconds.
- Put the user goal in `message` (verbatim plus needed clarifications).
- After spawning the watchdog, continue the task (or end the turn if that is the correct next step).

Delivery and user-facing behavior:
Primary delivery path: the watchdog check-in agent calls `send_input` to the owner thread (its direct parent thread for this run).
Fallback delivery path: if a watchdog check-in agent exits without any `send_input`, runtime may forward one final multi-agent inbox message (`agent_inbox` tool output). This fallback is best-effort and not guaranteed.
- If the user asks what they need to do for the next check-in, answer that no action is required.
- Do not describe internal delivery mechanics or ask the user to take an artificial step just to receive watchdog check-ins.

Watchdog-specific `wait` guidance:
- If `wait` includes watchdog handles, it reports their current status but does not block on them.
- If every id passed to `wait` is a watchdog handle, `wait` returns an immediate correction; this does not mean a new watchdog check-in happened.

Operational notes:
- Do not call `send_input` on watchdog handles.
- The tool returns a watchdog handle ID. Do not close a watchdog unless the user explicitly asks you to, or replacement is intentional; otherwise keep it running and continue.

Treat watchdog guidance as high-priority execution feedback. If it reveals a missing required action, do that action before status narration while honoring higher-priority system/developer/user constraints. A required action is one needed to satisfy the user request or clear a concrete blocker.

Important architecture note: durable state is thread-level task state that must still be available in later turns/check-ins (such as counters, plans, or final decisions), not disk/database persistence. Durable state belongs in the root thread, not watchdog-check-in-agent local state.
