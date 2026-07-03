//! Convenience sender for app events and common outbound TUI commands.
//!
//! This wraps the raw channel so call sites can submit typed `AppCommand`s
//! without duplicating event construction or session logging behavior.

use std::path::PathBuf;

use crate::app_command::AppCommand;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use codex_protocol::ThreadId;
use codex_protocol::request_permissions::RequestPermissionsResponse;
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
        self.send(AppEvent::CodexOp(AppCommand::interrupt()));
    }

    pub(crate) fn interrupt_and_restore_prompt_if_no_output(&self) {
        self.send(AppEvent::CodexOp(
            AppCommand::interrupt_and_restore_prompt_if_no_output(),
        ));
    }

    pub(crate) fn compact(&self) {
        self.send(AppEvent::CodexOp(AppCommand::compact()));
    }

    pub(crate) fn set_thread_name(&self, name: String) {
        self.send(AppEvent::CodexOp(AppCommand::set_thread_name(name)));
    }

    pub(crate) fn review(&self, target: ReviewTarget) {
        self.send(AppEvent::CodexOp(AppCommand::review(target)));
    }

    pub(crate) fn list_skills(&self, cwds: Vec<PathBuf>, force_reload: bool) {
        self.send(AppEvent::CodexOp(AppCommand::list_skills(
            cwds,
            force_reload,
        )));
    }

    pub(crate) fn user_input_answer(&self, id: String, response: ToolRequestUserInputResponse) {
        self.send(AppEvent::CodexOp(AppCommand::user_input_answer(
            id, response,
        )));
    }

    pub(crate) fn app_server_user_input_answer(
        &self,
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        id: String,
        response: ToolRequestUserInputResponse,
    ) {
        self.send_app_server_response(
            thread_id,
            request_id,
            AppCommand::user_input_answer(id, response),
        );
    }

    pub(crate) fn exec_approval(
        &self,
        thread_id: ThreadId,
        request_id: Option<AppServerRequestId>,
        id: String,
        decision: CommandExecutionApprovalDecision,
    ) {
        self.send_prompt_response(
            thread_id,
            request_id,
            AppCommand::exec_approval(id, /*turn_id*/ None, decision),
        );
    }

    pub(crate) fn request_permissions_response(
        &self,
        thread_id: ThreadId,
        request_id: Option<AppServerRequestId>,
        id: String,
        response: RequestPermissionsResponse,
    ) {
        self.send_prompt_response(
            thread_id,
            request_id,
            AppCommand::request_permissions_response(id, response),
        );
    }

    pub(crate) fn patch_approval(
        &self,
        thread_id: ThreadId,
        request_id: Option<AppServerRequestId>,
        id: String,
        decision: FileChangeApprovalDecision,
    ) {
        self.send_prompt_response(
            thread_id,
            request_id,
            AppCommand::patch_approval(id, decision),
        );
    }

    pub(crate) fn resolve_elicitation(
        &self,
        thread_id: ThreadId,
        server_name: String,
        request_id: AppServerRequestId,
        decision: McpServerElicitationAction,
        content: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) {
        self.send_app_server_response(
            thread_id,
            request_id.clone(),
            AppCommand::resolve_elicitation(server_name, request_id, decision, content, meta),
        );
    }

    fn send_prompt_response(
        &self,
        thread_id: ThreadId,
        request_id: Option<AppServerRequestId>,
        op: AppCommand,
    ) {
        match request_id {
            Some(request_id) => self.send_app_server_response(thread_id, request_id, op),
            None => self.send(AppEvent::SubmitThreadOp { thread_id, op }),
        }
    }

    fn send_app_server_response(
        &self,
        thread_id: ThreadId,
        request_id: AppServerRequestId,
        op: AppCommand,
    ) {
        self.send(AppEvent::ResolveAppServerRequest {
            thread_id,
            request_id,
            op,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    fn thread_id() -> ThreadId {
        ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread id")
    }

    #[test]
    fn typed_app_server_request_ids_select_exact_resolution_route() {
        let (tx, mut rx) = unbounded_channel();
        let sender = AppEventSender::new(tx);

        for request_id in [
            AppServerRequestId::Integer(1),
            AppServerRequestId::String("1".to_string()),
        ] {
            sender.exec_approval(
                thread_id(),
                Some(request_id.clone()),
                "approval".to_string(),
                CommandExecutionApprovalDecision::Accept,
            );

            let AppEvent::ResolveAppServerRequest {
                thread_id: event_thread_id,
                request_id: event_request_id,
                op: AppCommand::ExecApproval { id, .. },
            } = rx.try_recv().expect("expected exact app-server response")
            else {
                panic!("expected ResolveAppServerRequest");
            };
            assert_eq!(event_thread_id, thread_id());
            assert_eq!(event_request_id, request_id);
            assert_eq!(id, "approval");
        }
    }

    #[test]
    fn untagged_approval_uses_ordinary_thread_op_route() {
        let (tx, mut rx) = unbounded_channel();
        let sender = AppEventSender::new(tx);

        sender.exec_approval(
            thread_id(),
            None,
            "approval".to_string(),
            CommandExecutionApprovalDecision::Accept,
        );

        let AppEvent::SubmitThreadOp {
            thread_id: event_thread_id,
            op: AppCommand::ExecApproval { id, .. },
        } = rx.try_recv().expect("expected ordinary thread op")
        else {
            panic!("untagged approval must not use app-server resolution route");
        };
        assert_eq!(event_thread_id, thread_id());
        assert_eq!(id, "approval");
    }
}
