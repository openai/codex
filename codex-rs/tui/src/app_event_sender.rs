//! Convenience sender for app events and common outbound TUI commands.
//!
//! This wraps the raw channel so call sites can submit typed `AppCommand`s
//! without duplicating event construction or session logging behavior.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

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
use crate::app_event::ConversationOrigin;
use crate::app_event::ConversationTarget;
use crate::app_event::PaneGeneration;
use crate::app_event::PaneSlot;
use crate::session_log;

#[derive(Clone, Debug)]
struct ConversationScope {
    origin: ConversationOrigin,
    thread_id: Arc<RwLock<Option<ThreadId>>>,
}

fn has_explicit_conversation_delivery(event: &AppEvent) -> bool {
    matches!(
        event,
        AppEvent::ConversationOp { .. }
            | AppEvent::FromConversation { .. }
            | AppEvent::SubmitThreadOp { .. }
    )
}

#[derive(Clone, Debug)]
pub(crate) struct AppEventSender {
    pub app_event_tx: UnboundedSender<AppEvent>,
    conversation_scope: Option<ConversationScope>,
}

impl AppEventSender {
    pub(crate) fn new(app_event_tx: UnboundedSender<AppEvent>) -> Self {
        Self {
            app_event_tx,
            conversation_scope: None,
        }
    }

    /// Returns a sender scope shared by one chat widget and all of its child views.
    pub(crate) fn scoped_to_conversation(&self, pane: PaneSlot) -> Self {
        Self {
            app_event_tx: self.app_event_tx.clone(),
            conversation_scope: Some(ConversationScope {
                origin: ConversationOrigin {
                    pane,
                    generation: PaneGeneration::fresh(),
                },
                thread_id: Arc::new(RwLock::default()),
            }),
        }
    }

    /// Binds this widget scope to the thread supplied by its session configuration.
    pub(crate) fn bind_conversation_thread(&self, thread_id: ThreadId) {
        let Some(scope) = &self.conversation_scope else {
            tracing::warn!(%thread_id, "cannot bind an unscoped app event sender");
            return;
        };
        let mut bound_thread_id = match scope.thread_id.write() {
            Ok(thread_id) => thread_id,
            Err(poisoned) => poisoned.into_inner(),
        };
        *bound_thread_id = Some(thread_id);
    }

    pub(crate) fn conversation_origin(&self) -> Option<ConversationOrigin> {
        self.conversation_scope.as_ref().map(|scope| scope.origin)
    }

    fn conversation_target(&self) -> Option<ConversationTarget> {
        let scope = self.conversation_scope.as_ref()?;
        let thread_id = match scope.thread_id.read() {
            Ok(thread_id) => thread_id,
            Err(poisoned) => poisoned.into_inner(),
        };
        Some(ConversationTarget {
            pane: scope.origin.pane,
            generation: scope.origin.generation,
            thread_id: (*thread_id)?,
        })
    }

    /// Send an event to the app event channel. If it fails, we swallow the
    /// error and log it.
    pub(crate) fn send(&self, event: AppEvent) {
        // Record inbound events for high-fidelity session replay before adding the conversation
        // envelope, so existing event-specific logging remains intact.
        // Avoid double-logging Ops; those are logged at the point of submission.
        if !matches!(
            event,
            AppEvent::CodexOp(_)
                | AppEvent::ConversationOp { .. }
                | AppEvent::FromConversation { .. }
        ) {
            session_log::log_inbound_app_event(&event, self.conversation_origin());
        }

        let event = match (self.conversation_scope.as_ref(), event) {
            (Some(_), AppEvent::CodexOp(op)) => {
                let Some(target) = self.conversation_target() else {
                    tracing::warn!("dropping op from an unbound conversation sender");
                    return;
                };
                AppEvent::ConversationOp { target, op }
            }
            (_, event) if has_explicit_conversation_delivery(&event) => event,
            (Some(scope), event) => AppEvent::FromConversation {
                target: scope.origin,
                event: Box::new(event),
            },
            (None, event) => event,
        };
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

    pub(crate) fn exec_approval(
        &self,
        thread_id: ThreadId,
        id: String,
        decision: CommandExecutionApprovalDecision,
    ) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::exec_approval(id, /*turn_id*/ None, decision),
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
            op: AppCommand::request_permissions_response(id, response),
        });
    }

    pub(crate) fn patch_approval(
        &self,
        thread_id: ThreadId,
        id: String,
        decision: FileChangeApprovalDecision,
    ) {
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::patch_approval(id, decision),
        });
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
        self.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::resolve_elicitation(server_name, request_id, decision, content, meta),
        });
    }
}

#[cfg(test)]
#[path = "app_event_sender_tests.rs"]
mod tests;
