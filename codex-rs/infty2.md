# Infty v2 - Minimal Cross-Session Loop

Goal: collapse the orchestration to three composable primitives while preserving the existing flow.

- spawn: create a role session with base instructions + config
- await: wait for the assistant message that ends the user turn
- forward: inject an assistant message as a user message in another session

The rest of the orchestrator becomes a tiny router that parses the Solver's signal and calls these helpers.

---

## Design Overview

We build a thin, reusable facade over `codex-core`'s cross-session utilities. This facade is role- and run-aware so callers don't need to handle `ConversationId` bookkeeping.

Key types from `codex-core::cross_session` that we lean on:

- `CrossSessionHub` - registers sessions and routes messages across them
- `PostUserTurnRequest` - payload to submit text to a session
- `TurnHandle` - handle for a turn (used to await the assistant)
- `AssistantMessage` - the first assistant message for a turn
- `SessionEventStream` - event stream for activity/idle timeouts

In `codex-infty`, we expose tiny helpers that wrap these primitives in a role-centric API.
[director.md](codex-infty/src/prompts/director.md)
---

## Minimal API (Facade)

Proposed module: `codex-infty/src/session.rs` (or fold into `orchestrator.rs` if preferred). Names shown here as free functions; methods on a small struct are also fine.

```rust
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use serde_json::Value;
use codex_core::{ConversationManager, NewConversation};
use codex_core::config::Config;
use codex_core::cross_session::{
    CrossSessionHub, PostUserTurnRequest, RoleOrId, TurnHandle, AssistantMessage,
};
use codex_protocol::ConversationId;

/// Opaque role session reference used by the orchestrator.
#[derive(Clone)]
pub struct RoleSession {
    pub role: String,
    pub conversation_id: ConversationId,
    pub conversation: Arc<codex_core::CodexConversation>,
}

/// 1) Spawn a role session with base instructions applied.
pub async fn spawn(
    hub: Arc<CrossSessionHub>,
    manager: &ConversationManager,
    run_id: &str,
    role: &str,
    mut config: Config,
    rollout_dir: impl Into<std::path::PathBuf>,
    ensure_instructions: impl FnOnce(&str, &mut Config),
) -> Result<RoleSession> {
    config.cwd = rollout_dir.into();
    ensure_instructions(role, &mut config);
    let created: NewConversation = manager
        .new_conversation_with_cross_session(
            config,
            codex_core::CrossSessionSpawnParams {
                hub: Arc::clone(&hub),
                run_id: Some(run_id.to_string()),
                role: Some(role.to_string()),
            },
        )
        .await?;
    Ok(RoleSession {
        role: role.to_string(),
        conversation_id: created.conversation_id,
        conversation: created.conversation,
    })
}

/// 2a) Post a user turn to a role.
pub async fn post(
    hub: &CrossSessionHub,
    run_id: &str,
    role: &str,
    text: impl Into<String>,
    final_output_json_schema: Option<Value>,
) -> Result<TurnHandle, codex_core::cross_session::CrossSessionError> {
    hub.post_user_turn(PostUserTurnRequest {
        target: RoleOrId::RunRole { run_id: run_id.to_string(), role: role.to_string() },
        text: text.into(),
        final_output_json_schema,
    }).await
}

/// 2b) Await the first assistant message for this turn.
pub async fn await_first(
    hub: &CrossSessionHub,
    handle: &TurnHandle,
    timeout: Duration,
) -> Result<AssistantMessage, codex_core::cross_session::CrossSessionError> {
    hub.await_first_assistant(handle, timeout).await
}

/// 2c) Await with idle timeout that resets on activity for this submission id.
/// (Move the existing codex-infty implementation here verbatim.)
```

```rust
pub async fn await_first_idle(
    hub: &CrossSessionHub,
    handle: &TurnHandle,
    idle_timeout: Duration,
) -> Result<AssistantMessage> {
    use anyhow::{anyhow, bail};
    use codex_core::protocol::EventMsg;
    use tokio::time::Instant;
    use tokio_stream::StreamExt as _;

    let mut events = hub.stream_events(handle.conversation_id())?;
    let wait_first = hub.await_first_assistant(handle, idle_timeout);
    tokio::pin!(wait_first);

    let idle = tokio::time::sleep(idle_timeout);
    tokio::pin!(idle);

    let sub_id = handle.submission_id().to_string();

    loop {
        tokio::select! {
            res = &mut wait_first => { return res.map_err(|e| anyhow!(e)); }
            maybe_event = events.next() => {
                let Some(ev) = maybe_event else { bail!(codex_core::cross_session::CrossSessionError::SessionClosed); };
                if ev.event.id == sub_id {
                    if let EventMsg::Error(err) = &ev.event.msg { bail!(anyhow!(err.message.clone())); }
                    idle.as_mut().reset(Instant::now() + idle_timeout);
                }
            }
            _ = &mut idle => { bail!(codex_core::cross_session::CrossSessionError::AwaitTimeout(idle_timeout)); }
        }
    }
}
```

```rust
/// 3) Forward an assistant's content as a user message to another role.
pub async fn forward_assistant(
    hub: &CrossSessionHub,
    run_id: &str,
    target_role: &str,
    assistant: &AssistantMessage,
    timeout: Duration,
    final_output_json_schema: Option<Value>,
) -> Result<AssistantMessage> {
    let handle = post(
        hub,
        run_id,
        target_role,
        assistant.message.message.clone(),
        final_output_json_schema,
    ).await?;
    Ok(await_first(hub, &handle, timeout).await?)
}

/// Convenience: do both post + await in one call.
pub async fn call(
    hub: &CrossSessionHub,
    run_id: &str,
    role: &str,
    text: impl Into<String>,
    timeout: Duration,
    final_output_json_schema: Option<Value>,
) -> Result<AssistantMessage> {
    let handle = post(hub, run_id, role, text, final_output_json_schema).await?;
    Ok(await_first(hub, &handle, timeout).await?)
}
```

Notes:
- `await_first_idle` is the ergonomic default in Infty because it handles streaming with activity-based resets.
- The facade leaves JSON schema optional and role-addressing consistent with `RunRole { run_id, role }`.

---

## Orchestrator Main Loop Becomes Tiny

Once the three operations exist, the loop reduces to routing:

```rust
// Pseudocode using the facade
let mut solver_ev = hub.stream_events(sessions.solver.conversation_id)?;

if let Some(objective) = options.objective.as_deref() {
    post(&hub, &run_id, &sessions.solver.role, objective, Some(solver_signal_schema())).await?;
}

loop {
    let ev = solver_ev.next().await.ok_or_else(|| anyhow::anyhow!("solver closed"))?;
    if let EventMsg::AgentMessage(agent) = &ev.event.msg {
        if let Some(signal) = parse_solver_signal(&agent.message) {
            match signal {
                SolverSignal::DirectionRequest { prompt: Some(p) } => {
                    let req = serde_json::to_string(&DirectionRequestPayload {
                        kind: "direction_request",
                        prompt: &p,
                        objective: options.objective.as_deref(),
                    })?;
                    let directive = call(&hub, &run_id, &sessions.director.role, req, options.director_timeout, Some(directive_response_schema())).await?;
                    let _ = forward_assistant(&hub, &run_id, &sessions.solver.role, &directive, std::time::Duration::from_secs(5), Some(solver_signal_schema())).await?;
                }
                SolverSignal::VerificationRequest { claim_path: Some(path), notes } => {
                    let req = serde_json::to_string(&VerificationRequestPayload {
                        kind: "verification_request",
                        claim_path: &path,
                        notes: notes.as_deref(),
                        objective: options.objective.as_deref(),
                    })?;
                    let mut verdicts = Vec::new();
                    for v in &sessions.verifiers {
                        let verdict = call(&hub, &run_id, &v.role, &req, options.verifier_timeout, Some(verifier_verdict_schema())).await?;
                        verdicts.push((v.role.clone(), parse_json_struct::<VerifierVerdict>(&verdict.message.message)?));
                    }
                    let summary = aggregate_verdicts(verdicts);
                    let _ = post(&hub, &run_id, &sessions.solver.role, serde_json::to_string(&summary)?, Some(solver_signal_schema())).await?;
                }
                SolverSignal::FinalDelivery { deliverable_path: Some(path), summary } => {
                    let deliverable = resolve_deliverable_path(sessions.store.path(), &path)?;
                    return Ok(RunOutcome { run_id, deliverable_path: deliverable, summary, raw_message: agent.message.clone() });
                }
                _ => {}
            }
        }
    }
}
```

Everything above already exists in `codex-infty` today; the facade simply standardizes the small operations so the loop reads linearly.

---

## Implementation Steps

1) Extract helpers
- Add `session.rs` with `spawn`, `post`, `await_first`, `await_first_idle`, `forward_assistant`, `call`.
- Move the existing `await_first_assistant_idle` body from `orchestrator.rs` to this module (exported).
- Re-export from `lib.rs` if desirable for external callers.

2) Adopt helpers in `orchestrator.rs`
- Replace `post_to_role`, `await_first_assistant`, `relay_assistant_to_role`, and `call_role` with the facade functions.
- Keep signal parsing and run-store logic; delete glue code that becomes redundant.

3) Keep role spawn/resume minimal
- Inline `spawn_role_session` and `resume_role_session` to call `session::spawn(...)` with `prompts::ensure_instructions`.
- Preserve persistence of rollout/config paths via `RunStore`.

4) Preserve JSON schema guarantees
- Pass schemas through `post`/`call`/`forward_assistant` exactly as today:
  - Solver outbound: `solver_signal_schema()`
  - Director outbound: `directive_response_schema()`
  - Verifier outbound: `verifier_verdict_schema()`
  - Finalization: `final_delivery_schema()` for the last probe

5) Progress reporting stays orthogonal
- Where the orchestrator previously called `progress.*`, keep those calls around the facade usage (no change to the trait).

6) Tests and docs
- Unit-test the facade with a tiny harness that posts to a mock/run role and awaits the first assistant.
- Update README examples to use `call` and `forward_assistant` for clarity.

---

## Snippets to Drop In

- Posting user input and awaiting the assistant with idle timeout:

```rust
let handle = session::post(hub, &run_id, &role, user_text, schema).await?;
let assistant = session::await_first_idle(hub, &handle, std::time::Duration::from_secs(120)).await?;
```

- Forwarding an assistant to another role:

```rust
let reply = session::forward_assistant(hub, &run_id, &target_role, &assistant, std::time::Duration::from_secs(60), target_schema).await?;
```

- Spawning a session with base instructions:

```rust
let solver = session::spawn(
    Arc::clone(&hub),
    &conversation_manager,
    &run_id,
    "solver",
    solver_cfg.clone(),
    run_path, // becomes cfg.cwd
    |role, cfg| prompts::ensure_instructions(role, cfg),
).await?;
```

---

## Why This Simplifies Things

- One mental model: "post -> await -> forward" across roles.
- Orchestrator logic is a small, readable router.
- Cross-session reliability remains in one place (the hub).
- Tests become surgical: assert an assistant message is forwarded or a schema is respected.

---

## Backward Compatibility

- All current public behavior stays the same.
- `InftyOrchestrator` public methods keep signatures; they are implemented in terms of the facade.
- No changes to `codex-core` types or wire protocol.

---

## Optional Follow-Ups

- Consider upstreaming `await_first_idle` into `codex-core` so others can reuse it outside Infty.
- Add typed wrappers for JSON payloads (newtypes) to reduce `serde_json::Value` usage at call sites.
- Provide a tiny `SessionRouter` example crate to demonstrate building custom flows with these primitives.
