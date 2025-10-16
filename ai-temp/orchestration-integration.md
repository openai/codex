# Multi-Agent Orchestration Integration Plan

This document describes how to wire true sub-agent orchestration into the Codex CLI so the primary agent can delegate work to agent profiles living under `~/.codex/agents/<agent_id>/`. It focuses on runtime control-flow, UI/UX, and minimal-coupling integration points in the existing codebase.

---

## 1. Runtime Architecture

### 1.1 Components

- **`codex-multi-agent` crate (`codex-rs/multi-agent/src/lib.rs`)**  
  Already exposes `AgentId`, `AgentRegistry`, and async loaders that return `AgentContext` values (merged `ConfigToml` + `Config`). We extend this crate with an orchestration module to keep agent resolution and config cloning isolated from the rest of the app. Each `AgentContext` now captures its own `multi_agent.agents` list so child delegates inherit the correct allowlist automatically.

- **Orchestrator core (new)**  
  Proposed module `codex-rs/multi-agent/src/orchestrator.rs` exporting:
  - `AgentHandle`: carries `AgentContext`, active `ConversationId`, and bookkeeping (start/end timestamps, status).
  - `DelegateRequest`: SPA-style struct describing who/what to run (`agent_id`, prompt payload, optional working directory override).
  - `AgentOrchestrator`: stateful controller that owns:
    - A primary `AgentHandle` (mirrors currently running conversation).
    - A per-agent `ConversationManager` + `UnboundedSender<Op>` pair created via `ConversationManager::with_delegate` so child runs can spawn their own delegates.
    - Result channels to stream `Event` values back to the primary UI after post-processing.
    - A stack of active run ids so nested delegates can execute concurrently.

- **`ConversationManager` reuse**  
  Sub-agent sessions use the same `ConversationManager` entry points. The orchestrator calls `ConversationManager::new_conversation` with the agent-specific `Config` so all persistence automatically lands in `~/.codex/agents/<id>/` (per §2.2).

- **Primary session**  
  Unchanged: `tui::App` (`codex-rs/tui/src/app.rs:78`) continues to own a `ConversationManager` for the main agent. The orchestrator is injected so it can spawn additional conversations on demand.

### 1.2 Execution Flow

1. **Delegate trigger**  
   - User explicitly requests delegation (see UI plan below), or the primary agent emits a structured tool call.
   - We normalize the intent into `DelegateRequest`.

2. **Agent resolution**  
   - `AgentOrchestrator::resolve_agent` calls `AgentConfigLoader::load` with the requested `AgentId`.
   - On success, the orchestrator instantiates / reuses a `ConversationManager` scoped to that agent. Authentication stays shared (`AuthManager` from the primary session) per current design docs. The returned `AgentContext` also defines which downstream agents this delegate is allowed to call.

3. **Conversation bootstrap**  
   - Call `ConversationManager::new_conversation` with the agent `Config`.
   - The orchestrator captures the new `UnboundedSender<Op>` from `spawn_agent` (`codex-rs/tui/src/chatwidget/agent.rs:16`) or an equivalent helper in the orchestrator crate.

4. **Task execution**  
   - The orchestrator forwards the translated prompt into the sub-agent conversation (`conversation.submit`).  
   - Streamed `Event` values are intercepted before they reach the UI. For every event:
     - Persist to the sub-agent transcript as normal (handled by core).
     - Convert to orchestrator messages (`DelegateProgress`, `DelegateOutput`), then forward to the primary session via a new `AppEvent::DelegateUpdate`. Nested runs simply push additional `Started` events with greater depth.

5. **Completion and summary**  
   - When `EventMsg::TaskComplete` fires, the orchestrator synthesizes a summary cell (e.g., `history_cell::AgentMessageCell`) and injects it into the primary transcript via `AppEvent::InsertHistoryCell`.
   - Store a compact record (duration, exit status) for `/status` display and optional audit logging (`~/.codex/log/multi-agent.log` per `ai-temp/persistence-design.md`).

6. **Cleanup**  
   - Keep the sub-agent conversation alive if the profile supports follow-up chat, otherwise call `ConversationManager::remove_conversation`.

---

## 2. Control-Flow Integration

### 2.1 Entry Points

| Concern | File | Hook |
| --- | --- | --- |
| Orchestrator instantiation | `codex-rs/tui/src/app.rs:82` | Inject an `AgentOrchestrator` alongside the existing `ConversationManager`. |
| Slash-command parsing | `codex-rs/tui/src/slash_command.rs` & `codex-rs/tui/src/chatwidget.rs:1126` | Add `/delegate` (or `/agent`) command to open a delegate picker or dispatch a delegate request. |
| App event handling | `codex-rs/tui/src/app.rs:247` (`while let Some(event)`) | Route new `AppEvent::DelegateRequest` to `AgentOrchestrator::handle_request`. |
| Event fan-in | `codex-rs/tui/src/app.rs:330` | Handle `AppEvent::DelegateUpdate` to mutate transcript/history cells. |
| Status card | `codex-rs/tui/src/status/card.rs:68` | Pull orchestrator metrics (active agents, last run) to display in `/status`. |

### 2.2 Persistence

- Sub-agent sessions reuse existing persistence automatically because `Config::codex_home` already points at `~/.codex/agents/<id>` once we load through `AgentConfigLoader`.
- For the primary history: add summary inserts via `AppEvent::InsertHistoryCell` (`codex-rs/tui/src/app_event.rs:31`). No changes needed in core rollout recording.

### 2.3 Error Handling

- Map orchestration errors to `AppEvent::InsertHistoryCell` with `history_cell::new_error_event` so failures surface in the main transcript.
- Log details with `tracing::error!` inside the orchestrator, aligning with the `ai-temp/error-handling.md` guidance.

---

## 3. UI & UX Plan

### 3.1 Invocation

- **Slash command**: `/delegate <agent_id> [prompt...]`  
  - Add `SlashCommand::Delegate` in `codex-rs/tui/src/slash_command.rs`.  
  - In `ChatWidget::dispatch_command` (`codex-rs/tui/src/chatwidget.rs:1126`), call a new method `open_delegate_dialog()` that lists available agents via `AgentRegistry::list_agent_ids`.


### 3.2 Transcript Presentation

- Introduce a specialized history cell (e.g., `DelegationSummaryCell`) under `codex-rs/tui/src/history_cell.rs`.  
  - Show a header `↳ rust_test_writer (success in 23s)` and embed the sub-agent's final answer.  
  - Link to the sub-agent session path using the existing `SessionHeader` styling helpers (`codex-rs/tui/src/chatwidget/session_header.rs`).

- While the sub-agent runs, insert a “progress” cell (spinner) similar to exec command cells (`codex-rs/tui/src/exec_cell/render.rs:157`). Update via `DelegateProgress` events.

### 3.3 Status View

- Extend `compose_agents_summary` (`codex-rs/tui/src/status/helpers.rs:14`) to append active sub-agent counts and last-run statuses by querying the orchestrator handle cache.

### 3.4 Keyboard & UX

- Shortcut: `Ctrl+D` opens the delegate picker when the composer is empty.
- For task isolation, disable `/delegate` while another sub-agent call is running unless the selected agent supports concurrent runs (metadata flag in agent config).

---

## 4. Minimal Coupling Strategy

1. **Keep core unaware**  
   - No changes to `codex-rs/core/src/codex.rs` or the protocol. The orchestrator consumes the existing `Op`/`Event` API via `CodexConversation`.

2. **Orchestrator as a library**  
   - Implement orchestration in `codex-multi-agent` (new module) so the CLI/TUI crates depend only on a slim API:
     ```rust
     pub struct AgentOrchestrator { /* … */ }
     impl AgentOrchestrator {
         pub async fn available_agents(&self) -> Result<Vec<AgentId>>;
         pub async fn delegate(&self, request: DelegateRequest) -> Result<DelegateHandle>;
         pub fn subscribe(&self) -> mpsc::UnboundedReceiver<DelegateUpdate>;
     }
     ```
   - This keeps the TUI glue thin and defers heavy logic to the crate that already knows how to load configs.

3. **UI changes confined to `tui/`**  
   - Avoid threading orchestration state through unrelated widgets. Only `ChatWidget`, `App`, and the status card interact with the orchestrator.

4. **CLI parity**  
   - Other frontends (`codex exec`, `codex cloud`) can opt-in later because orchestration lives behind a library boundary. No changes required now.

---

## 5. Implementation Phases

1. **Library groundwork**
   - Extend `codex-multi-agent` with orchestrator types and helper methods.
   - Add unit tests verifying `delegate()` spawns conversations and streams events (mock `ConversationManager`).

2. **TUI integration**
   - Instantiate orchestrator in `App::run` (`codex-rs/tui/src/app.rs:84`).
   - Add new `AppEvent` variants (`codex-rs/tui/src/app_event.rs:15`).
   - Update `ChatWidget` to emit delegate requests and render updates.

3. **UI polish**
   - Add history cell types and status indicators.
   - Expose keyboard shortcuts and help text.

4. **Testing**
   - Snapshot tests for `/delegate` output in `tui/src/chatwidget/tests.rs`.
   - Integration test creating a fake agent directory and verifying the orchestrator selects the correct `Config`.
   - Manual smoke test using the sample Codex home in `ai-temp/example-codex-home/`.

---

## 6. Decisions & Open Questions

- **Concurrent delegates**: The orchestrator now maintains a stack of active runs so delegates can invoke their own delegates; the UI surfaces the stack depth with indented history entries.
- **Prompt hand-off semantics**: The primary agent composes the sub-agent prompt with all relevant context before invoking `delegate()`. The orchestrator forwards the prompt verbatim without trimming history.
- **Return payload**: Still open. Default plan remains to summarize results in the primary transcript while exposing a “view details” action to open the sub-agent session.
- **Auth isolation**: Shared. All agents continue to use the primary `AuthManager`; per-agent credentials are out of scope unless a future requirement emerges.

---

## 7. References

- Agent loader implementation – `codex-rs/multi-agent/src/lib.rs`
- Conversation bootstrap – `codex-rs/core/src/conversation_manager.rs:57`
- TUI spawn helpers – `codex-rs/tui/src/chatwidget/agent.rs:16`
- Slash command dispatch – `codex-rs/tui/src/chatwidget.rs:1126`
- History cell construction – `codex-rs/tui/src/history_cell.rs`
- Status card summary – `codex-rs/tui/src/status/helpers.rs:14`
- App event wiring – `codex-rs/tui/src/app.rs:212` & `codex-rs/tui/src/app_event.rs:15`

These anchors will guide the low-impact code changes required to hook orchestration into the existing CLI.
