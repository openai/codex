use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::codex::Codex;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_interactive;
use crate::config::Config;
use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::guardian::GUARDIAN_SUBAGENT_NAME;
use crate::protocol::SandboxPolicy;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) enum GuardianSubagentRunOutcome {
    Completed(anyhow::Result<Option<String>>),
    TimedOut,
    Aborted,
}

pub(crate) struct GuardianSubagentRunParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_turn: Arc<TurnContext>,
    pub(crate) spawn_config: Config,
    pub(crate) live_network_config: Option<NetworkProxyConfig>,
    pub(crate) prompt_items: Vec<UserInput>,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) personality: Option<Personality>,
    pub(crate) external_cancel: Option<CancellationToken>,
}

#[derive(Default)]
pub(crate) struct GuardianSubagentManager {
    state: Mutex<Option<GuardianSubagent>>,
}

struct GuardianSubagent {
    codex: Codex,
    live_network_config: Option<NetworkProxyConfig>,
}

impl GuardianSubagent {
    async fn shutdown(self) {
        let _ = self.codex.shutdown_and_wait().await;
    }
}

impl GuardianSubagentManager {
    pub(crate) async fn run_review(
        &self,
        params: GuardianSubagentRunParams,
    ) -> GuardianSubagentRunOutcome {
        let mut state = self.state.lock().await;

        if state
            .as_ref()
            .is_some_and(|subagent| subagent.live_network_config != params.live_network_config)
            && let Some(subagent) = state.take()
        {
            subagent.shutdown().await;
        }

        if state.is_none() {
            match spawn_guardian_subagent(&params).await {
                Ok(subagent) => {
                    *state = Some(subagent);
                }
                Err(err) => {
                    return GuardianSubagentRunOutcome::Completed(Err(err));
                }
            }
        }

        let Some(subagent) = state.as_mut() else {
            return GuardianSubagentRunOutcome::Completed(Err(anyhow!(
                "guardian subagent was not available after spawn"
            )));
        };

        // Keep the same conversation id for prompt-cache reuse, but clear prior
        // review turns so each approval is evaluated independently.
        subagent
            .codex
            .session
            .replace_history(Vec::new(), None)
            .await;
        subagent
            .codex
            .session
            .set_previous_turn_settings(None)
            .await;

        params
            .parent_session
            .services
            .network_approval
            .copy_session_approved_hosts_to(&subagent.codex.session.services.network_approval)
            .await;

        let submit_result = subagent
            .codex
            .submit(Op::UserTurn {
                items: params.prompt_items,
                cwd: params.parent_turn.cwd.clone(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                model: params.model,
                effort: params.reasoning_effort,
                summary: Some(params.reasoning_summary),
                service_tier: None,
                final_output_json_schema: Some(params.schema),
                collaboration_mode: None,
                personality: params.personality,
            })
            .await;
        if let Err(err) = submit_result {
            if let Some(subagent) = state.take() {
                subagent.shutdown().await;
            }
            return GuardianSubagentRunOutcome::Completed(Err(err.into()));
        }

        let (outcome, keep_subagent) =
            wait_for_guardian_review(subagent, params.external_cancel.as_ref()).await;
        if !keep_subagent && let Some(subagent) = state.take() {
            subagent.shutdown().await;
        }
        outcome
    }
}

async fn spawn_guardian_subagent(
    params: &GuardianSubagentRunParams,
) -> anyhow::Result<GuardianSubagent> {
    let codex = run_codex_thread_interactive(
        params.spawn_config.clone(),
        params.parent_session.services.auth_manager.clone(),
        params.parent_session.services.models_manager.clone(),
        Arc::clone(&params.parent_session),
        Arc::clone(&params.parent_turn),
        CancellationToken::new(),
        SubAgentSource::Other(GUARDIAN_SUBAGENT_NAME.to_string()),
        None,
    )
    .await?;

    Ok(GuardianSubagent {
        codex,
        live_network_config: params.live_network_config.clone(),
    })
}

async fn wait_for_guardian_review(
    subagent: &GuardianSubagent,
    external_cancel: Option<&CancellationToken>,
) -> (GuardianSubagentRunOutcome, bool) {
    let timeout = tokio::time::sleep(GUARDIAN_REVIEW_TIMEOUT);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_subagent = interrupt_and_drain_turn(&subagent.codex).await.is_ok();
                return (GuardianSubagentRunOutcome::TimedOut, keep_subagent);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_subagent = interrupt_and_drain_turn(&subagent.codex).await.is_ok();
                return (GuardianSubagentRunOutcome::Aborted, keep_subagent);
            }
            event = subagent.codex.next_event() => {
                match event {
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(event) => {
                            return (
                                GuardianSubagentRunOutcome::Completed(Ok(event.last_agent_message)),
                                true,
                            );
                        }
                        EventMsg::TurnAborted(_) => {
                            return (
                                GuardianSubagentRunOutcome::Completed(Err(anyhow!(
                                    "guardian subagent aborted before producing an assessment"
                                ))),
                                false,
                            );
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return (
                            GuardianSubagentRunOutcome::Completed(Err(err.into())),
                            false,
                        );
                    }
                }
            }
        }
    }
}

async fn interrupt_and_drain_turn(codex: &Codex) -> anyhow::Result<()> {
    let _ = codex.submit(Op::Interrupt).await;

    tokio::time::timeout(GUARDIAN_INTERRUPT_DRAIN_TIMEOUT, async {
        loop {
            let event = codex.next_event().await?;
            if matches!(
                event.msg,
                EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_)
            ) {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await
    .map_err(|_| anyhow!("timed out draining guardian subagent after interrupt"))??;

    Ok(())
}
