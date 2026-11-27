//! The pending approval owns the proposed command until the client replies.

use super::Session;
use super::TurnContext;
use crate::state::PendingApproval;
use codex_protocol::approvals::NetworkPolicyRuleAction;
use codex_protocol::items::CommandExecutionItem;
use codex_protocol::items::CommandExecutionStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ReviewDecision;
use codex_utils_path_uri::PathUri;
use std::sync::Arc;

pub(crate) struct ProposedCommand {
    turn: Arc<TurnContext>,
    item: CommandExecutionItem,
}

impl ProposedCommand {
    pub(super) fn from_request(
        turn: &Arc<TurnContext>,
        request: &ExecApprovalRequestEvent,
        cwd: PathUri,
    ) -> Option<Box<Self>> {
        if !request.starts_command() {
            return None;
        }
        let item = CommandExecutionItem {
            id: request.call_id.clone(),
            model_context: request.model_context.clone(),
            sandbox_type: None,
            plugin_id: request.plugin_id.clone(),
            script_path: request.script_path.clone(),
            command: request.command.clone(),
            cwd,
            parsed_cmd: request.parsed_cmd.clone(),
            process_id: None,
            source: ExecCommandSource::Agent,
            interaction_input: None,
            status: CommandExecutionStatus::InProgress,
            aggregated_output: None,
            exit_code: None,
            duration: None,
        };
        Some(Box::new(Self {
            turn: Arc::clone(turn),
            item,
        }))
    }

    async fn complete(mut self, session: &Session, status: CommandExecutionStatus) {
        self.item.status = status;
        let event = session
            .prepare_item_completed(
                &self.turn,
                TurnItem::CommandExecution(self.item),
                /*content_token_count*/ None,
                /*user_input_extension_data*/ None,
            )
            .await;
        // No process launched, so there is no legacy ExecCommandEnd to emit.
        session
            .send_event_raw(Event {
                id: self.turn.sub_id.clone(),
                msg: EventMsg::ItemCompleted(event),
            })
            .await;
    }
}

impl PendingApproval {
    pub(super) async fn resolve_command(&mut self, session: &Session, decision: &ReviewDecision) {
        let status = match decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => return,
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } if network_policy_amendment.action == NetworkPolicyRuleAction::Allow => return,
            ReviewDecision::NetworkPolicyAmendment { .. }
            | ReviewDecision::Denied { .. }
            | ReviewDecision::Abort => CommandExecutionStatus::Declined,
            ReviewDecision::TimedOut | ReviewDecision::ApprovedMcpPolicyAmendment => {
                CommandExecutionStatus::Failed
            }
        };
        if let Some(command) = self.command.take() {
            command.complete(session, status).await;
        }
    }
}

impl Session {
    pub(crate) async fn fail_exec_approval(&self, approval_id: &str, turn_id: &str) {
        if let Some((pending, _)) = self.take_pending_approval(approval_id, Some(turn_id)).await {
            if let Some(command) = pending.command {
                command.complete(self, CommandExecutionStatus::Failed).await;
            }
            let _ = pending
                .sender
                .send(ReviewDecision::denied("approval request failed"));
        }
    }
}
