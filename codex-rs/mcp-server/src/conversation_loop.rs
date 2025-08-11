use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::error::Result as CodexResult;
use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::ApplyPatchApprovalRequestEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecApprovalRequestEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use mcp_types::RequestId;
use tokio::sync::Mutex;
// no streaming watch channel; streaming is toggled via set_streaming on the struct
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

use crate::exec_approval::handle_exec_approval_request;
use crate::mcp_protocol::CodexEventNotificationParams;
use crate::mcp_protocol::ConversationId;
use crate::mcp_protocol::InitialStateNotificationParams;
use crate::mcp_protocol::InitialStatePayload;
use crate::mcp_protocol::NotificationMeta;
use crate::mcp_protocol::ServerNotification;
use crate::outgoing_message::OutgoingMessageSender;
use crate::patch_approval::handle_patch_approval_request;
use crate::request_id::request_id_to_string;

/// Conversation struct that owns the Codex session and all per-conversation state.
pub(crate) struct Conversation {
    codex: Arc<Codex>,
    session_id: Uuid,
    outgoing: Arc<OutgoingMessageSender>,
    request_id: RequestId,
    state: Mutex<ConversationState>,
    cancel: CancellationToken,
}

struct ConversationState {
    streaming_enabled: bool,
    buffered_events: Vec<CodexEventNotificationParams>,
    pending_elicitations: Vec<PendingElicitation>,
}

impl Conversation {
    pub(crate) fn new(
        codex: Arc<Codex>,
        outgoing: Arc<OutgoingMessageSender>,
        request_id: RequestId,
        session_id: Uuid,
    ) -> Arc<Self> {
        let conv = Arc::new(Self {
            codex,
            session_id,
            outgoing,
            request_id,
            state: Mutex::new(ConversationState {
                streaming_enabled: false,
                buffered_events: Vec::new(),
                pending_elicitations: Vec::new(),
            }),
            cancel: CancellationToken::new(),
        });
        // Detach a background loop tied to this Conversation
        spawn_conversation_loop(conv.clone());
        conv
    }

    pub(crate) async fn set_streaming(&self, enabled: bool) {
        if enabled {
            let (events_snapshot, pending_snapshot) = {
                let mut st = self.state.lock().await;
                st.streaming_enabled = true;
                (
                    st.buffered_events.clone(),
                    std::mem::take(&mut st.pending_elicitations),
                )
            };
            self.emit_initial_state_with(events_snapshot).await;
            self.drain_pending_elicitations_from(pending_snapshot).await;
        } else {
            let mut st = self.state.lock().await;
            st.streaming_enabled = false;
        }
    }

    pub(crate) fn codex(&self) -> Arc<Codex> {
        self.codex.clone()
    }

    pub(crate) async fn try_submit_user_input(
        &self,
        request_id: RequestId,
        items: Vec<InputItem>,
    ) -> CodexResult<()> {
        let _ = request_id; // request_id is not used to enforce uniqueness; Codex generates ids.
        self.codex.submit(Op::UserInput { items }).await.map(|_| ())
    }

    async fn handle_event(&self, event: Event) {
        {
            let mut st = self.state.lock().await;
            st.buffered_events.push(CodexEventNotificationParams {
                meta: None,
                msg: event.msg.clone(),
            });
        }
        self.stream_event_if_enabled(&event.msg).await;

        match event.msg {
            EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                command,
                cwd,
                call_id,
                reason: _,
            }) => {
                self.process_exec_request(command, cwd, call_id, event.id.clone())
                    .await;
            }
            EventMsg::Error(err) => {
                error!("Codex runtime error: {}", err.message);
            }
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id,
                reason,
                grant_root,
                changes,
            }) => {
                self.start_patch_approval(PatchRequest {
                    call_id,
                    reason,
                    grant_root,
                    changes,
                    event_id: event.id.clone(),
                })
                .await;
            }
            EventMsg::TaskComplete(_) => {}
            EventMsg::TaskStarted => {}
            EventMsg::SessionConfigured(ev) => {
                error!("unexpected SessionConfigured event: {:?}", ev);
            }
            EventMsg::AgentMessageDelta(_) => {}
            EventMsg::AgentReasoningDelta(_) => {}
            EventMsg::AgentMessage(AgentMessageEvent { .. }) => {}
            EventMsg::TokenCount(_)
            | EventMsg::AgentReasoning(_)
            | EventMsg::AgentReasoningRawContent(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::McpToolCallBegin(_)
            | EventMsg::McpToolCallEnd(_)
            | EventMsg::ExecCommandBegin(_)
            | EventMsg::ExecCommandEnd(_)
            | EventMsg::BackgroundEvent(_)
            | EventMsg::ExecCommandOutputDelta(_)
            | EventMsg::PatchApplyBegin(_)
            | EventMsg::PatchApplyEnd(_)
            | EventMsg::GetHistoryEntryResponse(_)
            | EventMsg::PlanUpdate(_)
            | EventMsg::TurnDiff(_)
            | EventMsg::ShutdownComplete => {}
        }
    }

    async fn emit_initial_state_with(&self, events: Vec<CodexEventNotificationParams>) {
        let params = InitialStateNotificationParams {
            meta: Some(NotificationMeta {
                conversation_id: Some(ConversationId(self.session_id)),
                request_id: None,
            }),
            initial_state: InitialStatePayload { events },
        };
        self.outgoing
            .send_server_notification(ServerNotification::InitialState(params))
            .await;
    }

    async fn drain_pending_elicitations_from(&self, items: Vec<PendingElicitation>) {
        for item in items {
            match item {
                PendingElicitation::ExecRequest(ExecRequest {
                    command,
                    cwd,
                    event_id,
                    call_id,
                }) => {
                    handle_exec_approval_request(
                        command,
                        cwd,
                        self.outgoing.clone(),
                        self.codex.clone(),
                        self.request_id.clone(),
                        request_id_to_string(&self.request_id),
                        event_id,
                        call_id,
                    )
                    .await;
                }
                PendingElicitation::PatchRequest(PatchRequest {
                    call_id,
                    reason,
                    grant_root,
                    changes,
                    event_id,
                }) => {
                    handle_patch_approval_request(
                        call_id,
                        reason,
                        grant_root,
                        changes,
                        self.outgoing.clone(),
                        self.codex.clone(),
                        self.request_id.clone(),
                        request_id_to_string(&self.request_id),
                        event_id,
                    )
                    .await;
                }
            }
        }
    }

    async fn process_exec_request(
        &self,
        command: Vec<String>,
        cwd: PathBuf,
        call_id: String,
        event_id: String,
    ) {
        let should_stream = {
            let st = self.state.lock().await;
            st.streaming_enabled
        };
        if should_stream {
            handle_exec_approval_request(
                command,
                cwd,
                self.outgoing.clone(),
                self.codex.clone(),
                self.request_id.clone(),
                request_id_to_string(&self.request_id),
                event_id,
                call_id,
            )
            .await;
        } else {
            let mut st = self.state.lock().await;
            st.pending_elicitations
                .push(PendingElicitation::ExecRequest(ExecRequest {
                    command,
                    cwd,
                    event_id,
                    call_id,
                }));
        }
    }

    async fn start_patch_approval(&self, req: PatchRequest) {
        let PatchRequest {
            call_id,
            reason,
            grant_root,
            changes,
            event_id,
        } = req;
        let should_stream = {
            let st = self.state.lock().await;
            st.streaming_enabled
        };
        if should_stream {
            handle_patch_approval_request(
                call_id,
                reason,
                grant_root,
                changes,
                self.outgoing.clone(),
                self.codex.clone(),
                self.request_id.clone(),
                request_id_to_string(&self.request_id),
                event_id,
            )
            .await;
        } else {
            let mut st = self.state.lock().await;
            st.pending_elicitations
                .push(PendingElicitation::PatchRequest(PatchRequest {
                    call_id,
                    reason,
                    grant_root,
                    changes,
                    event_id,
                }));
        }
    }

    async fn stream_event_if_enabled(&self, msg: &EventMsg) {
        if !{ self.state.lock().await.streaming_enabled } {
            return;
        }
        let method = msg.to_string();
        let params = CodexEventNotificationParams {
            meta: None,
            msg: msg.clone(),
        };
        match serde_json::to_value(&params) {
            Ok(params_val) => {
                self.outgoing
                    .send_custom_notification(&method, params_val)
                    .await;
            }
            Err(err) => {
                error!("Failed to serialize event params: {err:?}");
            }
        }
    }
}

enum PendingElicitation {
    ExecRequest(ExecRequest),
    PatchRequest(PatchRequest),
}

struct PatchRequest {
    call_id: String,
    reason: Option<String>,
    grant_root: Option<PathBuf>,
    changes: HashMap<PathBuf, FileChange>,
    event_id: String,
}

struct ExecRequest {
    command: Vec<String>,
    cwd: PathBuf,
    event_id: String,
    call_id: String,
}

impl Drop for Conversation {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn spawn_conversation_loop(this: Arc<Conversation>) {
    tokio::spawn(async move {
        let codex = this.codex.clone();
        let cancel = this.cancel.clone();
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    break;
                }
                res = codex.next_event() => {
                    match res {
                        Ok(event) => this.handle_event(event).await,
                        Err(e) => {
                            error!("Codex next_event error (session {}): {e}", this.session_id);
                            break;
                        }
                    }
                }
            }
        }
    });
}
