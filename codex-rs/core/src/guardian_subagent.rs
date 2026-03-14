use std::future::Future;
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
    cancel_token: CancellationToken,
    live_network_config: Option<NetworkProxyConfig>,
}

impl GuardianSubagent {
    async fn shutdown(self) {
        self.cancel_token.cancel();
        let _ = self.codex.shutdown_and_wait().await;
    }

    fn shutdown_in_background(self) {
        drop(tokio::spawn(async move {
            self.shutdown().await;
        }));
    }
}

impl GuardianSubagentManager {
    pub(crate) async fn shutdown(&self) {
        let subagent = self.state.lock().await.take();
        if let Some(subagent) = subagent {
            subagent.shutdown().await;
        }
    }

    pub(crate) async fn run_review(
        &self,
        params: GuardianSubagentRunParams,
    ) -> GuardianSubagentRunOutcome {
        let deadline = tokio::time::Instant::now() + GUARDIAN_REVIEW_TIMEOUT;
        let mut state = match run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            self.state.lock(),
        )
        .await
        {
            Ok(state) => state,
            Err(outcome) => return outcome,
        };

        if state
            .as_ref()
            .is_some_and(|subagent| subagent.live_network_config != params.live_network_config)
            && let Some(subagent) = state.take()
        {
            subagent.shutdown_in_background();
        }

        if state.is_none() {
            let spawn_cancel_token = CancellationToken::new();
            match run_before_review_deadline_with_cancel(
                deadline,
                params.external_cancel.as_ref(),
                &spawn_cancel_token,
                Box::pin(spawn_guardian_subagent(&params, spawn_cancel_token.clone())),
            )
            .await
            {
                Ok(Ok(subagent)) => {
                    *state = Some(subagent);
                }
                Ok(Err(err)) => {
                    return GuardianSubagentRunOutcome::Completed(Err(err));
                }
                Err(outcome) => return outcome,
            }
        }

        let Some(subagent) = state.as_mut() else {
            return GuardianSubagentRunOutcome::Completed(Err(anyhow!(
                "guardian subagent was not available after spawn"
            )));
        };

        let submit_result = run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            Box::pin(async {
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
                    .copy_session_approved_hosts_to(
                        &subagent.codex.session.services.network_approval,
                    )
                    .await;

                subagent
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
                    .await
            }),
        )
        .await;
        let submit_result = match submit_result {
            Ok(submit_result) => submit_result,
            Err(outcome) => {
                if let Some(subagent) = state.take() {
                    subagent.shutdown_in_background();
                }
                return outcome;
            }
        };
        if let Err(err) = submit_result {
            if let Some(subagent) = state.take() {
                subagent.shutdown_in_background();
            }
            return GuardianSubagentRunOutcome::Completed(Err(err.into()));
        }

        let (outcome, keep_subagent) =
            wait_for_guardian_review(subagent, deadline, params.external_cancel.as_ref()).await;
        if !keep_subagent && let Some(subagent) = state.take() {
            subagent.shutdown_in_background();
        }
        outcome
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(&self, codex: Codex) {
        *self.state.lock().await = Some(GuardianSubagent {
            codex,
            cancel_token: CancellationToken::new(),
            live_network_config: None,
        });
    }
}

async fn spawn_guardian_subagent(
    params: &GuardianSubagentRunParams,
    cancel_token: CancellationToken,
) -> anyhow::Result<GuardianSubagent> {
    let codex = run_codex_thread_interactive(
        params.spawn_config.clone(),
        params.parent_session.services.auth_manager.clone(),
        params.parent_session.services.models_manager.clone(),
        Arc::clone(&params.parent_session),
        Arc::clone(&params.parent_turn),
        cancel_token.clone(),
        SubAgentSource::Other(GUARDIAN_SUBAGENT_NAME.to_string()),
        None,
    )
    .await?;

    Ok(GuardianSubagent {
        codex,
        cancel_token,
        live_network_config: params.live_network_config.clone(),
    })
}

async fn wait_for_guardian_review(
    subagent: &GuardianSubagent,
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
) -> (GuardianSubagentRunOutcome, bool) {
    let timeout = tokio::time::sleep_until(deadline);
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

async fn run_before_review_deadline<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    future: impl Future<Output = T>,
) -> Result<T, GuardianSubagentRunOutcome> {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);

    tokio::select! {
        output = future => Ok(output),
        _ = &mut timeout => Err(GuardianSubagentRunOutcome::TimedOut),
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Err(GuardianSubagentRunOutcome::Aborted),
    }
}

async fn run_before_review_deadline_with_cancel<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    cancel_token: &CancellationToken,
    future: impl Future<Output = T>,
) -> Result<T, GuardianSubagentRunOutcome> {
    let result = run_before_review_deadline(deadline, external_cancel, future).await;
    if result.is_err() {
        cancel_token.cancel();
    }
    result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_times_out_before_future_completes() {
        let outcome = run_before_review_deadline(
            tokio::time::Instant::now() + Duration::from_millis(10),
            None,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        )
        .await;

        assert!(matches!(outcome, Err(GuardianSubagentRunOutcome::TimedOut)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_aborts_when_cancelled() {
        let cancel_token = CancellationToken::new();
        let canceller = cancel_token.clone();
        drop(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            canceller.cancel();
        }));

        let outcome = run_before_review_deadline(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Some(&cancel_token),
            std::future::pending::<()>(),
        )
        .await;

        assert!(matches!(outcome, Err(GuardianSubagentRunOutcome::Aborted)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_cancels_token_on_timeout() {
        let cancel_token = CancellationToken::new();

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_millis(10),
            None,
            &cancel_token,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            },
        )
        .await;

        assert!(matches!(outcome, Err(GuardianSubagentRunOutcome::TimedOut)));
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_cancels_token_on_abort() {
        let external_cancel = CancellationToken::new();
        let external_canceller = external_cancel.clone();
        let cancel_token = CancellationToken::new();
        drop(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            external_canceller.cancel();
        }));

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Some(&external_cancel),
            &cancel_token,
            std::future::pending::<()>(),
        )
        .await;

        assert!(matches!(outcome, Err(GuardianSubagentRunOutcome::Aborted)));
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
        let cancel_token = CancellationToken::new();

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_secs(1),
            None,
            &cancel_token,
            async { 7_u8 },
        )
        .await;

        assert!(matches!(outcome, Ok(7)));
        assert!(!cancel_token.is_cancelled());
    }
}
