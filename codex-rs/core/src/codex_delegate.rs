use std::sync::Arc;

use async_channel::Receiver;
use async_channel::Sender;
use codex_async_utils::OrCancelExt;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::codex::Codex;
use crate::codex::CodexSpawnOk;
use crate::config::Config;
use crate::error::CodexErr;
use codex_protocol::protocol::InitialHistory;

pub(crate) enum AgentEvent {
    ExecApprovalRequest(ExecApprovalRequestEvent, oneshot::Sender<ReviewDecision>),
    PatchApprovalRequest(
        ApplyPatchApprovalRequestEvent,
        oneshot::Sender<ReviewDecision>,
    ),
    EventMsg(EventMsg),
}

pub(crate) async fn run_codex_conversation(
    config: Config,
    auth_manager: Arc<AuthManager>,
    input: Vec<UserInput>,
    cancel_token: CancellationToken,
) -> Result<Receiver<AgentEvent>, CodexErr> {
    let (tx_sub, rx_sub) = async_channel::unbounded();
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        auth_manager,
        InitialHistory::New,
        SessionSource::SubAgent,
    )
    .await?;

    codex.submit(Op::UserInput { items: input }).await?;

    let cancel_token = cancel_token.clone();
    tokio::spawn(async move {
        let _ = forward_events(codex, tx_sub).or_cancel(&cancel_token).await;
    });

    Ok(rx_sub)
}

async fn forward_events(codex: Codex, tx_sub: Sender<AgentEvent>) {
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
                let (tx_approve, rx_approve) = oneshot::channel();
                let _ = tx_sub
                    .send(AgentEvent::ExecApprovalRequest(event, tx_approve))
                    .await;
                let _ = handle_exec_approval_request(id, rx_approve, &codex).await;
            }
            Event {
                id,
                msg: EventMsg::ApplyPatchApprovalRequest(event),
            } => {
                let (tx_approve, rx_approve) = oneshot::channel();
                let _ = tx_sub
                    .send(AgentEvent::PatchApprovalRequest(event, tx_approve))
                    .await;
                let _ = handle_patch_approval_request(id, rx_approve, &codex).await;
            }
            other => {
                let _ = tx_sub.send(AgentEvent::EventMsg(other.msg)).await;
            }
        }
    }
}

async fn handle_exec_approval_request(
    id: String,
    channel: oneshot::Receiver<ReviewDecision>,
    codex: &Codex,
) -> Result<(), CodexErr> {
    match channel.await {
        Ok(decision) => {
            codex.submit(Op::ExecApproval { id, decision }).await?;
            Ok(())
        }
        Err(_) => Err(CodexErr::InternalAgentDied),
    }
}

async fn handle_patch_approval_request(
    id: String,
    channel: oneshot::Receiver<ReviewDecision>,
    codex: &Codex,
) -> Result<(), CodexErr> {
    match channel.await {
        Ok(decision) => {
            codex.submit(Op::PatchApproval { id, decision }).await?;
            Ok(())
        }
        Err(_) => Err(CodexErr::InternalAgentDied),
    }
}
