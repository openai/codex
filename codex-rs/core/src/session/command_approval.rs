//! The pending approval owns the proposal during review, then returns it to
//! the invocation for launch. Dropping either owner finishes an abandoned item.

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
use tokio::task::JoinHandle;
use tokio_util::task::task_tracker::TaskTrackerToken;
use tracing::Instrument;

pub(super) struct ApprovalRequestFailed;

struct ProposedCommand {
    publication: TaskTrackerToken,
    interrupted_status: CommandExecutionStatus,
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    item: CommandExecutionItem,
    started_at_ms: i64,
}

/// The proposal belongs to one invocation across approvals and sandbox retries.
#[derive(Default)]
pub(crate) struct CommandApproval(Option<Box<ProposedCommand>>);

impl CommandApproval {
    pub(super) async fn start(
        &mut self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        request: &ExecApprovalRequestEvent,
        cwd: PathUri,
    ) -> Result<(), ApprovalRequestFailed> {
        if self.0.is_some() {
            return Ok(());
        }
        // Register before checking shutdown so it waits for every admitted proposal,
        // including the completion task spawned when the caller is dropped.
        let publication = session.command_approval_publications.token();
        if session.command_approval_publications.is_closed() {
            return Err(ApprovalRequestFailed);
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
        let event = session
            .prepare_item_started(
                turn,
                TurnItem::CommandExecution(item.clone()),
                request.started_at_ms,
            )
            .await;
        let started_at_ms = event.started_at_ms;
        // Sending to the client queue is the last step of send_event_raw. Take
        // ownership before the next await. The legacy begin waits for launch.
        session
            .send_event_raw(Event {
                id: turn.sub_id.clone(),
                msg: EventMsg::ItemStarted(event),
            })
            .await;
        self.0 = Some(Box::new(ProposedCommand {
            publication,
            interrupted_status: CommandExecutionStatus::Declined,
            session: Arc::clone(session),
            turn: Arc::clone(turn),
            item,
            started_at_ms,
        }));
        Ok(())
    }

    /// Transfer the legacy begin timestamp and shutdown token to execution.
    pub(crate) fn handoff(&mut self) -> Option<(i64, TaskTrackerToken)> {
        self.0
            .take()
            .map(|command| (command.started_at_ms, command.publication))
    }

    pub(crate) async fn fail(&mut self) {
        self.finish(CommandExecutionStatus::Failed).await;
    }

    async fn finish(&mut self, status: CommandExecutionStatus) {
        if let Some(command) = self.0.take()
            && let Err(err) = command.publish_completion(status).await
        {
            tracing::warn!(%err, "command approval publication failed");
        }
    }

    fn approved(&mut self) {
        if let Some(command) = self.0.as_mut() {
            command.interrupted_status = CommandExecutionStatus::Failed;
        }
    }
}

impl ProposedCommand {
    fn publish_completion(self, status: CommandExecutionStatus) -> JoinHandle<()> {
        tokio::spawn(self.complete(status).in_current_span())
    }

    async fn complete(mut self, status: CommandExecutionStatus) {
        self.item.status = status;
        let event = self
            .session
            .prepare_item_completed(
                &self.turn,
                TurnItem::CommandExecution(self.item),
                /*content_token_count*/ None,
                /*user_input_extension_data*/ None,
            )
            .await;
        // No process launched, so there is no legacy ExecCommandEnd to emit.
        self.session
            .send_event_raw(Event {
                id: self.turn.sub_id.clone(),
                msg: EventMsg::ItemCompleted(event),
            })
            .await;
        drop(self.publication);
    }
}

impl Drop for CommandApproval {
    fn drop(&mut self) {
        // A tool can be cancelled while awaiting a reply or preparing to launch.
        if let Some(command) = self.0.take() {
            let status = command.interrupted_status;
            drop(command.publish_completion(status));
        }
    }
}

impl PendingApproval {
    pub(super) async fn resolve_command(&mut self, decision: &ReviewDecision) {
        let status = match decision {
            ReviewDecision::Approved
            | ReviewDecision::ApprovedForSession
            | ReviewDecision::ApprovedExecpolicyAmendment { .. } => return self.command.approved(),
            ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment,
            } if network_policy_amendment.action == NetworkPolicyRuleAction::Allow => {
                return self.command.approved();
            }
            ReviewDecision::NetworkPolicyAmendment { .. }
            | ReviewDecision::Denied { .. }
            | ReviewDecision::Abort => CommandExecutionStatus::Declined,
            ReviewDecision::TimedOut | ReviewDecision::ApprovedMcpPolicyAmendment => {
                CommandExecutionStatus::Failed
            }
        };
        self.command.finish(status).await;
    }
}

impl Session {
    pub(crate) async fn fail_exec_approval(&self, approval_id: &str, turn_id: &str) {
        if let Some((mut pending, _)) = self.take_pending_approval(approval_id, Some(turn_id)).await
        {
            pending.command.fail().await;
            pending.reply(ReviewDecision::denied("approval request failed"));
        }
    }
}
