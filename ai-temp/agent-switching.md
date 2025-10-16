# Agent Switching Flow

## Goal
- Let users temporarily leave the main assistant, talk directly to the delegate that just ran, and then return to the primary conversation with the new context automatically folded in.
- Preserve the sub-agent’s own history/logs while also giving the main agent enough summary data to continue the original task without manual copy/paste.
- Keep this behaviour additive to the existing delegation pipeline (`ai-temp/orchestration-integration.md`) so we do not fork separate orchestration code paths.

## Interaction Flow
1. **Primary delegation** – User asks the main agent for work; it invokes `AgentOrchestrator::delegate(...)` (`codex-rs/multi-agent/src/orchestrator.rs`) and streams the sub-agent result as today.
2. **Switch request** – Once the delegate finishes, the UI surfaces an affordance (button, slash command, or keyboard shortcut) to “enter” that delegate session. The request references the agent id plus the conversation/session handle held by the orchestrator.
3. **Direct conversation** – While switched, user prompts are routed straight to the sub-agent’s `ConversationManager` (`codex-rs/core/src/conversation_manager.rs:57`), writing to `~/.codex/agents/<id>/history.jsonl` and `sessions/` per `ai-temp/persistence-design.md`.
4. **Return & sync** – Exiting the sub-agent triggers a summary event back to the primary conversation. The orchestrator composes:
   - Latest sub-agent turns since the last delegation summary.
   - Any user instructions entered during the switch.
   - Optional metadata (elapsed time, exit status) for the main transcript.
5. **Primary follow-up** – The main agent resumes with an updated context item (e.g., injected history cell) so the user can issue the closing instruction (“Solve task X”) without restating manual edits. It stays idle until the user provides that follow-up prompt; there is no automatic validation pass unless the user explicitly asks for one.

## Orchestrator Responsibilities
- Track active sub-agent sessions beyond the initial delegate run, including an offset to know which messages were created during the manual switch.
- Provide APIs to:
  - `enter_agent(agent_id, session_id)` – hand back a handle to the sub-agent conversation.
  - `exit_agent(agent_id, session_id)` – return summaries for rehydrating the primary transcript.
- Maintain a lightweight audit of switches (agent id, start/end timestamps) for `/status` (`codex-rs/tui/src/status/helpers.rs`) and debugging.
- Ensure authentication and tool permissions obey the main agent’s policy; the switch cannot elevate capabilities beyond what the delegate already has.

## Persistence & Context Sync
- Sub-agent turns continue to live exclusively under `~/.codex/agents/<id>/` so per-agent isolation stays intact (`ai-temp/persistence-design.md`).
- The main agent stores only synthesized snapshots: user switch transcript, sub-agent response digest, and references to the underlying rollout file.
- Conflict reconciliation remains manual. Unless a sub-agent overrides its working directory, it edits the same workspace as the main agent, so users should rely on git/review tooling to resolve overlapping changes.
- Each `AgentContext` already persists the delegate’s working directory (`ConfigOverrides::cwd`). When the sub-agent runs with a non-default cwd, the return summary should echo that path so the main agent understands where the edits landed.
- Consider storing a “since marker” (session id + line number) inside the orchestrator so re-entry picks up where the user left off.
- When returning, append a history cell in the main transcript citing the sub-agent session path and summarizing the net changes.

## UI Considerations
- Extend the delegation UI (`ai-temp/ui-ux-delegation.md`) with:
  - A status banner showing `In #<agent>` while switched, with a shortcut to return to the primary agent.
  - History cells that log switch events (`Entered #critic`, `Returned from #critic – applied adjustments`).
  - Optional shortcut `/agent return` to exit quickly.
- While switched, show an inline footer indicator (`In #agent`) next to the context meter so the active delegate is always visible.
- Hide picker entries whose conversations are no longer resumable (e.g., cleanup, failure). If the user attempts to switch into a stale handle, surface an error toast and keep them in the current context while logging the failure.
- While switched, the prompt input should clearly identify the active agent (e.g., placeholder text, accent color) to avoid accidental edits.
- Surface breadcrumbs in `/status` showing the current agent stack (`Main → #ideas_provider → #critic`), making nested switches easier to follow later.
- Scope out a history browser for now; we do not surface delegate sessions from previous main-agent runs.

## Edge Cases & Safeguards
- **Aborted delegate sessions** – If the orchestrator or sub-agent errors while you are switched in, emit a `DelegateEvent::Failed`, append an error history cell in the main transcript, and automatically return the user to the main agent. Also write the detailed failure to `codex-tui.log`.
- **Active-run visibility** – The orchestrator now tracks a stack of in-flight delegates. Surface the full stack in the UI so users know which nested agents are working; only the top-most run streams output.
- **Multi-agent hopping** – Switching among multiple delegates is hub-and-spoke: you can move main ↔ #ideas ↔ main ↔ #critic freely. Future “delegate chains” (sub-agents invoking their own sub-agents) remain out of scope; note this in breadcrumbs/help text so expectations stay clear.
- **Undo/redo** – Codex does not provide an orchestrator-level undo stack. Any manual file edits a user performs while switched should be managed through their VCS tooling.
- **Tool overlap** – Each sub-agent carries its own tool registry (e.g., plan tool). Streaming results during the switch stay in the sub-agent transcript; summaries injected on return should mention any plan updates so the main agent context is accurate.

## Code Impact

### Multi-Agent Feature Surfaces
- `codex-rs/multi-agent/src/orchestrator.rs` – extend state to track active delegate sessions, add `enter_agent`/`exit_agent` helpers, and retain offsets so we know which turns to summarize when the user returns.
- `codex-rs/multi-agent/src/lib.rs` – re-export the switching API and plumb new structs/enums (e.g., switch summaries, session handles). We may add a dedicated `switching.rs` module for bookkeeping.
- `codex-rs/multi-agent/src/tests/` (new) – cover enter/exit flows, ensuring we capture only newly added turns and that summaries are produced correctly.

### Core Runtime
- `codex-rs/core/src/conversation_manager.rs` – expose APIs to hand out existing `CodexConversation` handles (or resume by rollout) so the orchestrator can park and resume sub-agent sessions. We may need a lightweight “since marker” abstraction here.
- `codex-rs/core/src/delegate_tool.rs` – extend `DelegateToolEvent`/`DelegateToolRun` to serialize manual switch summaries back to the client.
- `codex-rs/core/src/tools/handlers/delegate.rs` – accept the richer payload, surface switch-specific metadata to the model, and ensure the handler stops streaming once the user exits the sub-agent.
- `codex-rs/core/src/codex.rs` – thread the orchestrator’s switch adapter into new conversations (similar to how the delegate adapter is wired today).

### TUI Integration
- `codex-rs/tui/src/app_event.rs` & `codex-rs/tui/src/app.rs` – introduce `AppEvent` variants for “enter agent”, “exit agent”, and “switch summaries”; drive the event loop transitions.
- `codex-rs/tui/src/chatwidget.rs` (plus `chatwidget/agent.rs`) – route user input to the active sub-agent while switched, render banners/breadcrumbs, and rehydrate the main transcript when returning.
- `codex-rs/tui/src/history_cell.rs` – add cell types for “entered delegate” / “returned from delegate” entries with session links.
- `codex-rs/tui/src/status/helpers.rs` & `/status` widgets – surface the active agent stack and recent switch history.
- `codex-rs/tui/src/slash_command.rs` – wire `/agent enter <id>` / `/agent return` (or similar) commands if we expose keyboard-driven switching.
- `codex-rs/tui/src/tests/` – update snapshot/unit tests to cover the new event stream and UI affordances.
- We explicitly skip building a “replay” browser for older delegate sessions in this iteration.

### CLI & Configuration
- `codex-rs/cli/src/main.rs` – ensure the CLI still constructs the orchestrator once, passing the new switching adapter into the TUI bootstrapper.
- `docs/` – update user-facing documentation (e.g., `docs/tui.md`, `docs/multi-agent.md`) to describe how to enter/exit a delegate session.
