# Session & Logging Strategy

## Guiding Principles
- Every sub-agent stores its session artifacts exclusively under `~/.codex/agents/<agent_id>/`.
- The main agent’s history remains uncluttered; it records only that a sub-agent was invoked, not the sub-agent’s internal transcript.
- Session and log data must never leak from one agent’s directory into another’s unless the user explicitly exports it.

## Rollouts (`sessions/`)
- `codex_core::rollout::recorder` writes JSONL rollouts beneath `config.codex_home.join("sessions")`.
- When we construct a `Config` for a sub-agent, we set `codex_home = ~/.codex/agents/<agent_id>`, so rollouts automatically land in `~/.codex/agents/<agent_id>/sessions/...`.
- Orchestrator responsibility:
  - Optionally create a lightweight stub entry in the main agent’s `sessions/` noting the sub-agent invocation (timestamp, agent id, summary).
  - Leave the full rollout content in the sub-agent’s directory.

## Streaming History (`history.jsonl`)
- `codex_core::message_history` appends to `config.codex_home/history.jsonl`.
- Sub-agents therefore maintain separate history files without additional work.
- The orchestrator may log a high-level event in the primary history file containing:
  - Agent id
  - Input prompt summary
  - Final output summary
  - Optional metadata (duration, status)
- No token-level or intermediate output from the sub-agent should be written to the main history file.

## Logging (`log/`)
- The TUI initialises its sink via `codex_core::config::log_dir(&config)`.
- With the sub-agent `Config`, logs land in `~/.codex/agents/<agent_id>/log/`.
- The orchestrator can maintain a central audit log (e.g., `~/.codex/log/multi-agent.log`) capturing cross-agent coordination events without duplicating the sub-agent logs.

## Temporary State
- Any ephemeral files (scratch buffers, intermediate diffs) created by sub-agents should live inside their chosen working directory or under their agent directory.
- The orchestrator provides helpers to allocate temp directories scoped to the agent id so cleanup routines are straightforward.

## Main-Agent Visibility
- Primary session/history entries contain only:
  - Which agent was invoked.
  - When the invocation started/finished.
  - Success/failure status plus short text summaries.
- Detailed traces require inspecting the per-agent directories, ensuring isolation by default while still enabling audits when needed.
