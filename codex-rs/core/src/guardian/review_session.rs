use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
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
use crate::config::Constrained;
use crate::config::NetworkProxySpec;
use crate::features::Feature;
use crate::protocol::SandboxPolicy;

use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::prompt::guardian_policy_prompt;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) enum GuardianReviewSessionOutcome {
    Completed(anyhow::Result<Option<String>>),
    TimedOut,
    Aborted,
}

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_turn: Arc<TurnContext>,
    pub(crate) spawn_config: Config,
    pub(crate) prompt_items: Vec<UserInput>,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) personality: Option<Personality>,
    pub(crate) external_cancel: Option<CancellationToken>,
}

#[derive(Default)]
pub(crate) struct GuardianReviewSessionManager {
    state: Mutex<Option<GuardianReviewSession>>,
}

struct GuardianReviewSession {
    codex: Codex,
    cancel_token: CancellationToken,
    spawn_config: Config,
}

impl GuardianReviewSession {
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

impl GuardianReviewSessionManager {
    pub(crate) async fn shutdown(&self) {
        let review_session = self.state.lock().await.take();
        if let Some(review_session) = review_session {
            review_session.shutdown().await;
        }
    }

    pub(crate) async fn run_review(
        &self,
        params: GuardianReviewSessionParams,
    ) -> GuardianReviewSessionOutcome {
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

        if state.as_ref().is_some_and(|review_session| {
            guardian_review_session_config_changed(
                &review_session.spawn_config,
                &params.spawn_config,
            )
        }) && let Some(review_session) = state.take()
        {
            review_session.shutdown_in_background();
        }

        if state.is_none() {
            let spawn_cancel_token = CancellationToken::new();
            match run_before_review_deadline_with_cancel(
                deadline,
                params.external_cancel.as_ref(),
                &spawn_cancel_token,
                Box::pin(spawn_guardian_review_session(
                    &params,
                    spawn_cancel_token.clone(),
                )),
            )
            .await
            {
                Ok(Ok(review_session)) => {
                    *state = Some(review_session);
                }
                Ok(Err(err)) => {
                    return GuardianReviewSessionOutcome::Completed(Err(err));
                }
                Err(outcome) => return outcome,
            }
        }

        let Some(review_session) = state.as_mut() else {
            return GuardianReviewSessionOutcome::Completed(Err(anyhow!(
                "guardian review session was not available after spawn"
            )));
        };

        let submit_result = run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            Box::pin(async {
                review_session
                    .codex
                    .session
                    .replace_history(Vec::new(), None)
                    .await;
                review_session
                    .codex
                    .session
                    .set_previous_turn_settings(None)
                    .await;

                params
                    .parent_session
                    .services
                    .network_approval
                    .copy_session_approved_hosts_to(
                        &review_session.codex.session.services.network_approval,
                    )
                    .await;

                review_session
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
                if let Some(review_session) = state.take() {
                    review_session.shutdown_in_background();
                }
                return outcome;
            }
        };
        if let Err(err) = submit_result {
            if let Some(review_session) = state.take() {
                review_session.shutdown_in_background();
            }
            return GuardianReviewSessionOutcome::Completed(Err(err.into()));
        }

        let (outcome, keep_review_session) =
            wait_for_guardian_review(review_session, deadline, params.external_cancel.as_ref())
                .await;
        if !keep_review_session && let Some(review_session) = state.take() {
            review_session.shutdown_in_background();
        }
        outcome
    }

    #[cfg(test)]
    pub(crate) async fn cache_for_test(&self, codex: Codex) {
        let spawn_config = (*codex.session.get_config().await).clone();
        *self.state.lock().await = Some(GuardianReviewSession {
            spawn_config,
            codex,
            cancel_token: CancellationToken::new(),
        });
    }
}

async fn spawn_guardian_review_session(
    params: &GuardianReviewSessionParams,
    cancel_token: CancellationToken,
) -> anyhow::Result<GuardianReviewSession> {
    let codex = run_codex_thread_interactive(
        params.spawn_config.clone(),
        params.parent_session.services.auth_manager.clone(),
        params.parent_session.services.models_manager.clone(),
        Arc::clone(&params.parent_session),
        Arc::clone(&params.parent_turn),
        cancel_token.clone(),
        SubAgentSource::Other(GUARDIAN_REVIEWER_NAME.to_string()),
        None,
    )
    .await?;

    Ok(GuardianReviewSession {
        codex,
        cancel_token,
        spawn_config: params.spawn_config.clone(),
    })
}

fn guardian_review_session_config_changed(
    cached_spawn_config: &Config,
    next_spawn_config: &Config,
) -> bool {
    cached_spawn_config != next_spawn_config
}

async fn wait_for_guardian_review(
    review_session: &GuardianReviewSession,
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
) -> (GuardianReviewSessionOutcome, bool) {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::TimedOut, keep_review_session);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_review_session = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return (GuardianReviewSessionOutcome::Aborted, keep_review_session);
            }
            event = review_session.codex.next_event() => {
                match event {
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            return (
                                GuardianReviewSessionOutcome::Completed(Ok(turn_complete.last_agent_message)),
                                true,
                            );
                        }
                        EventMsg::TurnAborted(_) => {
                            return (GuardianReviewSessionOutcome::Aborted, true);
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return (
                            GuardianReviewSessionOutcome::Completed(Err(err.into())),
                            false,
                        );
                    }
                }
            }
        }
    }
}

pub(crate) fn build_guardian_review_session_config(
    parent_config: &Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
) -> anyhow::Result<Config> {
    let mut guardian_config = parent_config.clone();
    guardian_config.model = Some(active_model.to_string());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.developer_instructions = Some(guardian_policy_prompt());
    guardian_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    guardian_config.permissions.sandbox_policy =
        Constrained::allow_only(SandboxPolicy::new_read_only_policy());
    if let Some(live_network_config) = live_network_config
        && guardian_config.permissions.network.is_some()
    {
        let network_constraints = guardian_config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()
            .map(|network| network.value.clone());
        guardian_config.permissions.network = Some(NetworkProxySpec::from_config_and_constraints(
            live_network_config,
            network_constraints,
            &SandboxPolicy::new_read_only_policy(),
        )?);
    }
    for feature in [
        Feature::SpawnCsv,
        Feature::Collab,
        Feature::WebSearchRequest,
        Feature::WebSearchCached,
    ] {
        guardian_config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "guardian review session could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if guardian_config.features.enabled(feature) {
            anyhow::bail!(
                "guardian review session requires `features.{}` to be disabled",
                feature.key()
            );
        }
    }
    Ok(guardian_config)
}

async fn run_before_review_deadline<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    tokio::select! {
        _ = tokio::time::sleep_until(deadline) => Err(GuardianReviewSessionOutcome::TimedOut),
        result = future => Ok(result),
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Err(GuardianReviewSessionOutcome::Aborted),
    }
}

async fn run_before_review_deadline_with_cancel<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    cancel_token: &CancellationToken,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
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
    .map_err(|_| anyhow!("timed out draining guardian review session after interrupt"))??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardian_review_session_config_change_invalidates_cached_session() {
        let parent_config = crate::config::test_config();
        let cached_spawn_config =
            build_guardian_review_session_config(&parent_config, None, "active-model", None)
                .expect("cached guardian config");

        let mut changed_parent_config = parent_config;
        changed_parent_config.model_provider.base_url =
            Some("https://guardian.example.invalid/v1".to_string());
        let next_spawn_config = build_guardian_review_session_config(
            &changed_parent_config,
            None,
            "active-model",
            None,
        )
        .expect("next guardian config");

        assert!(guardian_review_session_config_changed(
            &cached_spawn_config,
            &next_spawn_config
        ));
        assert!(!guardian_review_session_config_changed(
            &cached_spawn_config,
            &cached_spawn_config
        ));
    }

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

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::TimedOut)
        ));
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

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::Aborted)
        ));
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

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::TimedOut)
        ));
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

        assert!(matches!(
            outcome,
            Err(GuardianReviewSessionOutcome::Aborted)
        ));
        assert!(cancel_token.is_cancelled());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_before_review_deadline_with_cancel_preserves_token_on_success() {
        let cancel_token = CancellationToken::new();

        let outcome = run_before_review_deadline_with_cancel(
            tokio::time::Instant::now() + Duration::from_secs(1),
            None,
            &cancel_token,
            async { 42usize },
        )
        .await;

        assert_eq!(outcome.unwrap(), 42);
        assert!(!cancel_token.is_cancelled());
    }
}
