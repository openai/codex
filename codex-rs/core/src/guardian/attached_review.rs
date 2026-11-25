//! Holds the attached reviewer through the verdict. New evidence can wait for the next review.
//! Native retries take a new reservation under the original verdict deadline.
use super::super::*;
use super::PreparedApproval;
use crate::guardian::input_budget::PendingReviewContext;
use crate::guardian::review_session::PreparedVerdictTurn;
use crate::guardian::review_session::ensure_guardian_followup_reminder;
use crate::guardian::review_session::prepare_verdict_session;
use crate::guardian::review_session::review_turn;
use crate::guardian_review::AttachedReviewer;
use crate::guardian_review::PreparedAssessment;
use crate::guardian_review::ReviewPreparationError;
use codex_analytics::GuardianReviewSessionKind;
use codex_guardian_context::ComposedContext;
use codex_guardian_reviewer::GuardianReviewSessionOutcome;
use codex_guardian_reviewer::SessionDisposition;
use codex_protocol::user_input::UserInput;
use tokio::sync::Mutex;

pub(super) async fn root_history_version(
    parent: &Session,
    reviewer: &dyn AttachedReviewer,
) -> codex_protocol::error::Result<Option<super::RootHistory>> {
    // A hosted worker's authoritative runtime and reset version come from its host.
    if parent.host_guardian_context().is_some() {
        return Ok(parent
            .guardian_root_snapshot()
            .await?
            .as_ref()
            .map(super::root_history_key));
    }
    let Some(root) = reviewer.root_thread().await else {
        return Ok(None);
    };
    Ok(root
        .session
        .guardian_root_history_version()
        .await
        .map(|(id, version)| (id, None, version)))
}

pub(super) struct AttachedApproval {
    provider: Arc<dyn AttachedReviewer>,
    config: GuardianReviewSessionConfig,
    first: Mutex<Option<Box<dyn PreparedAssessment>>>,
}

impl AttachedApproval {
    pub async fn prepare(
        provider: Arc<dyn AttachedReviewer>,
        parent: &Arc<Session>,
        context: &GuardianReviewContext,
        deadline: ReviewDeadline,
        cancel: &CancellationToken,
    ) -> Result<Self, ReviewPreparationError> {
        let prepare = async {
            let config = guardian_review_session_config(parent, context, provider.retrieval())
                .await
                .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
            let reserved =
                Self::reserve(provider.as_ref(), &config, parent, context, cancel).await?;
            Ok(Self {
                provider,
                config,
                first: Mutex::new(Some(reserved)),
            })
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(ReviewPreparationError::Cancelled),
            _ = deadline.wait() => Err(ReviewPreparationError::PreparationTimedOut),
            result = prepare => result,
        }
    }

    async fn reserve(
        provider: &dyn AttachedReviewer,
        config: &GuardianReviewSessionConfig,
        parent: &Arc<Session>,
        context: &GuardianReviewContext,
        cancel: &CancellationToken,
    ) -> Result<Box<dyn PreparedAssessment>, ReviewPreparationError> {
        let setup = crate::guardian_review::PreparedGuardianContext::prepare(
            parent.clone(),
            context.clone(),
            config.spawn_config.clone(),
            &parent.clone_history().await,
            &config.node_repl_policy,
            /*uses_root_evidence*/ false,
        )
        .await
        .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
        provider.prepare(&setup, cancel).await
    }

    pub async fn attempt(
        &self,
        parent: &Arc<Session>,
        approval: &PreparedApproval,
        reasons: &ApprovalRequestReasons,
        deadline: ReviewDeadline,
        cancel: &CancellationToken,
    ) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
        let root_error = |error: codex_protocol::error::CodexErr| match error.details() {
            super::CodexErrorDetails::Timeout => GuardianReviewSessionOutcome::TimedOut,
            _ => GuardianReviewSessionOutcome::Aborted,
        };
        let mut reservation = None;
        let mut analytics = GuardianReviewAnalyticsResult::without_session();
        let mut usage_at_start = None;
        let prepare = async {
            if approval.root_history
                != root_history_version(parent, self.provider.as_ref())
                    .await
                    .map_err(root_error)?
            {
                return Err(GuardianReviewSessionOutcome::Aborted);
            }
            let first = self.first.lock().await.take();
            let model = match first {
                Some(first) => first,
                None => Self::reserve(
                    self.provider.as_ref(),
                    &self.config,
                    parent,
                    &approval.context,
                    cancel,
                )
                .await
                .map_err(|err| GuardianReviewSessionOutcome::PromptBuildFailed(err.into()))?,
            };
            let model = reservation.insert(model);
            let config = &self.config;
            let kind = model.session_kind();
            let runtime = model.runtime();
            analytics = runtime.analytics(
                &config.review_model,
                config.default_reasoning_effort.as_ref(),
                &config.spawn_config.model_provider_id,
                kind,
                matches!(kind, GuardianReviewSessionKind::TrunkReused),
            );
            let action = codex_guardian_reviewer::build_guardian_planned_action(
                &approval.request,
                reasons.retry.clone().or_else(|| reasons.approval.clone()),
                approval.context.enclosing_invocation_id.as_deref(),
            )
            .map_err(|err| GuardianReviewSessionOutcome::PromptBuildFailed(err.into()))?;
            let permissions = super::super::super::permissions::for_request(
                &approval.request,
                Some(&approval.context),
            )
            .map_err(GuardianReviewSessionOutcome::PromptBuildFailed)?;
            let action_context = ComposedContext::for_pending_action(
                action,
                permissions.as_ref(),
                config.spawn_config.guardian_context_budgeting(),
                approval.context.frame_native_agent_actions(),
            )
            .map_err(|err| GuardianReviewSessionOutcome::PromptBuildFailed(err.into()))?;
            let request = review_turn(
                parent,
                &approval.context,
                &config.spawn_config,
                &config.review_model,
                // Final admission replaces the turn-start marker with the typed action and permissions.
                vec![UserInput::Text {
                    text: super::super::super::prompt::GUARDIAN_TRANSCRIPT_START.to_owned(),
                    text_elements: Vec::new(),
                }],
                guardian_output_schema(),
            );
            let turn = PreparedVerdictTurn {
                request,
                evidence: PendingReviewContext {
                    context: action_context,
                    elastic: config.spawn_config.guardian_sync_elastic_budget.clone(),
                },
                node_repl_evidence_sequence: None,
            };
            usage_at_start = Some(runtime.token_usage_baseline().await);
            if matches!(kind, GuardianReviewSessionKind::TrunkReused) {
                ensure_guardian_followup_reminder(runtime).await;
            }
            prepare_verdict_session(
                runtime,
                &approval.context,
                approval.category,
                &config.node_repl_policy,
            )
            .await
            .map_err(GuardianReviewSessionOutcome::PromptBuildFailed)?;
            Ok((runtime, turn))
        };
        let prepared = run_before_review_deadline(deadline, Some(cancel), prepare).await;
        let result = async {
            let (runtime, turn) = prepared??;
            // Let the runner interrupt and drain the turn before it finalizes a provisional denial.
            let (turn, _) = runtime
                .run_sync_verdict(
                    &approval.context,
                    turn,
                    deadline,
                    Some(cancel),
                    &mut analytics,
                )
                .await?;
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(GuardianReviewSessionOutcome::Aborted),
                _ = deadline.wait() => {
                    if !turn.outcome.is_recovered_denial() {
                        return Err(GuardianReviewSessionOutcome::TimedOut);
                    }
                }
                root = root_history_version(parent, self.provider.as_ref()) => {
                    if approval.root_history != root.map_err(root_error)? {
                        return Err(GuardianReviewSessionOutcome::Aborted);
                    }
                }
            }
            Ok((turn.outcome, turn.disposition))
        }
        .await;
        let (outcome, disposition) =
            result.unwrap_or_else(|outcome| (outcome, SessionDisposition::Discard));
        let Some(model) = reservation else {
            return (outcome.into(), analytics);
        };
        if let Some(start) = usage_at_start {
            model
                .runtime()
                .record_verdict_usage(&start, &mut analytics)
                .await;
        }
        model
            .runtime()
            .record_failed_review(
                parent,
                &approval.context,
                &approval.request,
                &self.config.review_model.model,
                codex_guardian_reviewer::ReviewFeedbackSettings {
                    enabled: self.config.spawn_config.feedback_enabled,
                    ephemeral: self.config.spawn_config.ephemeral,
                },
                &outcome,
            )
            .await;
        let outcome = GuardianReviewOutcome::from(outcome);
        model.finish(if matches!(outcome, GuardianReviewOutcome::Completed(_)) {
            disposition
        } else {
            SessionDisposition::Discard
        });
        (outcome, analytics)
    }
}
