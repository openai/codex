use std::sync::Arc;

use async_channel::Receiver;
use async_channel::Sender;
use codex_async_utils::OrCancelExt;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::codex::SUBMISSION_CHANNEL_CAPACITY;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::CodexErr;
use codex_protocol::protocol::InitialHistory;

/// Channels for interacting with a sub-Codex conversation.
///
/// - `events_rx` streams non-approval `EventMsg`s from the sub-agent.
///   Approval requests are handled internally via the parent session and are not surfaced here.
/// - `ops_tx` allows callers to submit `Op`s to the sub-agent (e.g., `Op::UserInput`,
///   `Op::ExecApproval`, `Op::PatchApproval`).
#[derive(Clone)]
pub(crate) struct ConversationIo {
    pub events_rx: Receiver<EventMsg>,
    pub ops_tx: Sender<Op>,
}

/// Start an interactive sub-Codex conversation and return IO channels.
///
/// The returned `events_rx` yields non-approval events emitted by the sub-agent.
/// Approval requests are handled via `parent_session` and are not surfaced.
/// The returned `ops_tx` allows the caller to submit additional `Op`s to the sub-agent.
pub(crate) async fn run_codex_conversation_interactive(
    config: Config,
    auth_manager: Arc<AuthManager>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
) -> Result<ConversationIo, CodexErr> {
    let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let (tx_ops, rx_ops) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);

    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        auth_manager,
        InitialHistory::New,
        SessionSource::SubAgent,
    )
    .await?;
    let codex = Arc::new(codex);

    // Use a child token so parent cancel cascades but we can scope it to this task
    let cancel_token_events = cancel_token.child_token();
    let cancel_token_ops = cancel_token.child_token();

    // Forward events from the sub-agent to the consumer, filtering approvals and
    // routing them to the parent session for decisions.
    let parent_session_clone = Arc::clone(&parent_session);
    let parent_ctx_clone = Arc::clone(&parent_ctx);
    let codex_for_events = Arc::clone(&codex);
    tokio::spawn(async move {
        let _ = forward_events(
            codex_for_events,
            tx_sub,
            parent_session_clone,
            parent_ctx_clone,
        )
        .or_cancel(&cancel_token_events)
        .await;
    });

    // Forward ops from the caller to the sub-agent.
    let codex_for_ops = Arc::clone(&codex);
    tokio::spawn(async move {
        loop {
            let op = match rx_ops.recv().or_cancel(&cancel_token_ops).await {
                Ok(Ok(op)) => op,
                Ok(Err(_)) | Err(_) => break,
            };
            let _ = codex_for_ops.submit(op).await;
        }
    });

    Ok(ConversationIo {
        events_rx: rx_sub,
        ops_tx: tx_ops,
    })
}

/// Convenience wrapper for one-time use with an initial prompt.
///
/// Internally calls the interactive variant, then immediately submits the provided input.
pub(crate) async fn run_codex_conversation_one_shot(
    config: Config,
    auth_manager: Arc<AuthManager>,
    input: Vec<UserInput>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancel_token: CancellationToken,
) -> Result<ConversationIo, CodexErr> {
    let io = run_codex_conversation_interactive(
        config,
        auth_manager,
        parent_session,
        parent_ctx,
        cancel_token,
    )
    .await?;

    io.ops_tx
        .send(Op::UserInput { items: input })
        .await
        .map_err(|err| CodexErr::Fatal(format!("failed to send initial input op: {err}")))?;

    Ok(io)
}

async fn forward_events(
    codex: Arc<Codex>,
    tx_sub: Sender<EventMsg>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
) {
    while let Ok(event) = codex.next_event().await {
        match event {
            Event {
                id: _,
                msg: EventMsg::SessionConfigured(_),
            } => continue,
            Event {
                id,
                msg: EventMsg::ExecApprovalRequest(event),
            } => {
                // Initiate approval via parent session; do not surface to consumer.
                let decision = parent_session
                    .request_command_approval(
                        parent_ctx.as_ref(),
                        parent_ctx.sub_id.clone(),
                        event.command.clone(),
                        event.cwd.clone(),
                        event.reason.clone(),
                    )
                    .await;
                let _ = codex.submit(Op::ExecApproval { id, decision }).await;
            }
            Event {
                id,
                msg: EventMsg::ApplyPatchApprovalRequest(event),
            } => {
                let decision = parent_session
                    .request_patch_approval(
                        parent_ctx.as_ref(),
                        parent_ctx.sub_id.clone(),
                        event.changes.clone(),
                        event.reason.clone(),
                        event.grant_root.clone(),
                    )
                    .await;
                let _ = codex
                    .submit(Op::PatchApproval {
                        id,
                        decision: decision.await.unwrap_or_default(),
                    })
                    .await;
            }
            other => {
                let _ = tx_sub.send(other.msg).await;
            }
        }
    }
}
