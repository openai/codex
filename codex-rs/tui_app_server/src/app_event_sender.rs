use std::path::PathBuf;

use crate::app_command::AppCommand;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio::sync::mpsc::UnboundedSender;

use crate::app_event::AppEvent;
use crate::session_log;

#[derive(Clone, Debug)]
pub(crate) struct AppEventSender {
    pub app_event_tx: UnboundedSender<AppEvent>,
}

impl AppEventSender {
    pub(crate) fn new(app_event_tx: UnboundedSender<AppEvent>) -> Self {
        Self { app_event_tx }
    }

    /// Send an event to the app event channel. If it fails, we swallow the
    /// error and log it.
    pub(crate) fn send(&self, event: AppEvent) {
        // Record inbound events for high-fidelity session replay.
        // Avoid double-logging Ops; those are logged at the point of submission.
        if !matches!(event, AppEvent::CodexOp(_)) {
            session_log::log_inbound_app_event(&event);
        }
        if let Err(e) = self.app_event_tx.send(event) {
            tracing::error!("failed to send event: {e}");
        }
    }

    pub(crate) fn interrupt(&self) {
        self.send(AppEvent::CodexOp(AppCommand::interrupt().into_core()));
    }

    pub(crate) fn compact(&self) {
        self.send(AppEvent::CodexOp(AppCommand::compact().into_core()));
    }

    pub(crate) fn set_thread_name(&self, name: String) {
        self.send(AppEvent::CodexOp(
            AppCommand::set_thread_name(name).into_core(),
        ));
    }

    pub(crate) fn review(&self, review_request: ReviewRequest) {
        self.send(AppEvent::CodexOp(
            AppCommand::review(review_request).into_core(),
        ));
    }

    pub(crate) fn list_skills(&self, cwds: Vec<PathBuf>, force_reload: bool) {
        self.send(AppEvent::CodexOp(
            AppCommand::list_skills(cwds, force_reload).into_core(),
        ));
    }

    pub(crate) fn realtime_conversation_audio(&self, params: ConversationAudioParams) {
        self.send(AppEvent::CodexOp(
            AppCommand::realtime_conversation_audio(params).into_core(),
        ));
    }

    pub(crate) fn user_input_answer(&self, id: String, response: RequestUserInputResponse) {
        self.send(AppEvent::CodexOp(
            AppCommand::user_input_answer(id, response).into_core(),
        ));
    }

    pub(crate) fn exec_approval(&self, thread_id: ThreadId, id: String, decision: ReviewDecision) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::exec_approval(id, None, decision).into_core(),
        });
    }

    pub(crate) fn request_permissions_response(
        &self,
        thread_id: ThreadId,
        id: String,
        response: RequestPermissionsResponse,
    ) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::request_permissions_response(id, response).into_core(),
        });
    }

    pub(crate) fn patch_approval(&self, thread_id: ThreadId, id: String, decision: ReviewDecision) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::patch_approval(id, decision).into_core(),
        });
    }

    pub(crate) fn resolve_elicitation(
        &self,
        thread_id: ThreadId,
        server_name: String,
        request_id: McpRequestId,
        decision: ElicitationAction,
        content: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::resolve_elicitation(server_name, request_id, decision, content, meta)
                .into_core(),
        });
    }
}
