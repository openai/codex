//! Holds one action's prepared reviewer and its pre-capture authorization revision.
//! Native retries take a new reservation under the original verdict deadline.
use super::super::*;
use super::PreparedApproval;
use super::ReviewAuthorization;
use crate::guardian::input_budget::PendingReviewContext;
use crate::guardian::review_session::PreparedVerdictTurn;
use crate::guardian::review_session::ensure_guardian_followup_reminder;
use crate::guardian::review_session::prepare_verdict_session;
use crate::guardian::review_session::review_turn;
use crate::guardian_review::BoundReviewer;
use crate::guardian_review::PreparedAssessment;
use crate::guardian_review::ReviewPreparationError;
use codex_analytics::GuardianReviewSessionKind;
use codex_guardian_context::ComposedContext;
use codex_guardian_reviewer::GuardianReviewSessionOutcome;
use codex_guardian_reviewer::SessionDisposition;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::user_input::UserInput;
use tokio::sync::Mutex;

pub(super) struct AttachedApproval {
    provider: BoundReviewer,
    config: GuardianReviewSessionConfig,
    first: Mutex<Option<(ReviewAuthorization, Box<dyn PreparedAssessment>)>>,
}

impl AttachedApproval {
    pub async fn prepare(
        provider: BoundReviewer,
        authorization: ReviewAuthorization,
        parent: &Arc<Session>,
        context: &GuardianReviewContext,
        deadline: ReviewDeadline,
        cancel: &CancellationToken,
    ) -> Result<Self, ReviewPreparationError> {
        let prepare = async {
            let config =
                guardian_review_session_config(parent, context, provider.retrieval.clone())
                    .await
                    .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
            let reserved = Self::reserve(&provider, &config, parent, context, cancel).await?;
            Ok(Self {
                provider,
                config,
                first: Mutex::new(Some((authorization, reserved))),
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
        provider: &BoundReviewer,
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
        )
        .await
        .map_err(|_| ReviewPreparationError::ReviewerUnavailable)?;
        provider.preparation.prepare(&setup, cancel).await
    }

    pub async fn attempt(
        &self,
        parent: &Arc<Session>,
        approval: &PreparedApproval,
        reasons: &ApprovalRequestReasons,
        authorization: ReviewAuthorization,
        deadline: ReviewDeadline,
        cancel: &CancellationToken,
    ) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
        let mut reservation = None;
        let mut analytics = GuardianReviewAnalyticsResult::without_session();
        let mut usage_at_start = None;
        let prepare = async {
            let first = self.first.lock().await.take();
            let (authorization, model) = match first {
                Some(first) => first,
                None => (
                    authorization,
                    Self::reserve(
                        &self.provider,
                        &self.config,
                        parent,
                        &approval.context,
                        cancel,
                    )
                    .await
                    .map_err(|err| GuardianReviewSessionOutcome::PromptBuildFailed(err.into()))?,
                ),
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
            if let Some(stale) = authorization.invalidation(parent, Some(cancel)).await {
                analytics.context_changed = stale.context_changed;
                return Err(stale.outcome);
            }
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
            Ok((runtime, authorization, turn))
        };
        let prepared = run_before_review_deadline(deadline, Some(cancel), prepare).await;
        let result = async {
            let (runtime, authorization, turn) = prepared??;
            // Let the runner interrupt and drain the turn before it finalizes a provisional denial.
            let (turn, _) = runtime
                .run_sync_verdict(&approval.context, turn, deadline, Some(cancel), &mut analytics)
                .await?;
            let stale = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(GuardianReviewSessionOutcome::Aborted),
                _ = deadline.wait() => {
                    // An interrupted second sample may still carry the first denial.
                    let recovered_denial = match &turn.outcome {
                        GuardianReviewSessionOutcome::Completed(Ok(Some(response))) if !turn.turn_completed => {
                            response.assessment().is_ok_and(|assessment| {
                                assessment.outcome == GuardianAssessmentOutcome::Deny
                            })
                        }
                        _ => false,
                    };
                    if !recovered_denial {
                        return Err(GuardianReviewSessionOutcome::TimedOut);
                    }
                    None
                }
                stale = authorization.invalidation(parent, Some(cancel)) => stale,
            };
            if let Some(stale) = stale {
                analytics.context_changed |= stale.context_changed;
                return Err(stale.outcome);
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
