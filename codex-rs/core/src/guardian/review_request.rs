//! Captures a review on the host's original action and authorization state.
//! The extension chooses effects; this adapter supplies evidence, validation and publication.

use super::*;
use crate::codex_thread::GuardianAuthorizationVersion;
use codex_guardian_reviewer::ReviewHost;
use codex_protocol::approvals::GuardianReviewReason;

pub(in crate::guardian) struct PreparedApproval {
    request: GuardianApprovalRequest,
    root_authorization_version: Option<GuardianAuthorizationVersion>,
    user_message_revision: u64,
    review_evidence: Option<(
        Arc<GuardianReviewEvidence>,
        String,
        GuardianAuthorizationVersion,
        Option<GuardianAuthorizationVersion>,
    )>,
}

impl ReviewHost for super::super::runtime::ReviewRuntime {
    type Prepared = PreparedApproval;

    fn cancellation(&self) -> Option<&CancellationToken> {
        self.options.external_cancel.as_ref()
    }

    async fn servicing_turn(
        &self,
    ) -> Option<(String, Arc<codex_protocol::openai_models::ModelInfo>)> {
        let active = self.session.active_turn.lock().await;
        let turn = &active.as_ref()?.task.as_ref()?.turn_context;
        Some((turn.sub_id.clone(), Arc::clone(turn.model_info())))
    }

    async fn prepare(
        &self,
        review_reason: GuardianReviewReason,
        deadline: Instant,
    ) -> Result<(PreparedApproval, codex_guardian_reviewer::ReviewReport), ReviewDecision> {
        let super::super::runtime::ReviewRuntime {
            session,
            history_reset: _,
            context,
            review_id,
            request,
            reasons: _,
            options,
        } = self.clone();
        let request = match request.validate(&context) {
            Ok(request) => request.clone(),
            Err(decision) => return Err(decision),
        };
        let model_context = context.model_context();
        let turn = Arc::clone(context.turn());
        let GuardianReviewOptions {
            plugin_attribution_override,
            approval_request_source,
            external_cancel,
            require_synchronous_review: _,
            require_guardian: _,
        } = options;
        let target_item_id = guardian_request_target_item_id(&request).map(str::to_string);
        let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
        let plugin_attribution = match plugin_attribution_override {
            Some(attribution) => Some(attribution),
            None if matches!(&request, GuardianApprovalRequest::ExecCommand { .. }) => {
                let cancellation = external_cancel
                    .clone()
                    .unwrap_or_else(CancellationToken::new);
                let attribution_deadline = std::cmp::min(
                    deadline,
                    Instant::now() + GUARDIAN_PLUGIN_ATTRIBUTION_TIMEOUT,
                );
                let attribution = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return Err(ReviewDecision::Abort),
                    attribution = tokio::time::timeout_at(
                        attribution_deadline,
                        plugin_attribution_for_guardian_request(&context, &request),
                    ) => attribution,
                };
                match attribution {
                    Ok(attribution) => attribution,
                    Err(_) => {
                        tracing::warn!(
                            timeout_ms = GUARDIAN_PLUGIN_ATTRIBUTION_TIMEOUT.as_millis(),
                            "Guardian plugin attribution timed out"
                        );
                        None
                    }
                }
            }
            None => plugin_attribution_for_guardian_request(&context, &request).await,
        };
        let (plugin_id, script_path) = plugin_attribution
            .as_ref()
            .map(PluginCommandAttribution::serialized_fields)
            .unzip();
        let report =
            codex_guardian_reviewer::ReviewReport::new(codex_guardian_reviewer::ReviewMetadata {
                thread_id: session.thread_id.to_string(),
                turn_id: assessment_turn_id,
                review_id,
                target_item_id,
                plugin_id,
                script_path,
                approval_request_source,
                reviewed_action: guardian_reviewed_action(&request),
                action: guardian_assessment_action(&request),
                review_reason,
                model_context,
            });
        let root_authorization_version = session
            .services
            .agent_control
            .root_user_authorization(session.thread_id)
            .await
            .map(|snapshot| snapshot.authorization_version);
        // Keep the authorization revision even when no cacheable review evidence exists.
        let history = session.conversation_history_snapshot().await;
        let user_message_revision = history.user_message_revision();
        let review_evidence = if let Some(evidence) = session
            .services
            .thread_extension_data
            .get::<GuardianReviewEvidence>()
        {
            let authorization_version = evidence.authorization_version(history.as_ref());
            format_guardian_action_pretty(&request).ok().map(|action| {
                (
                    evidence,
                    action,
                    authorization_version,
                    root_authorization_version,
                )
            })
        } else {
            None
        };
        drop(history);
        Ok((
            PreparedApproval {
                request,
                root_authorization_version,
                user_message_revision,
                review_evidence,
            },
            report,
        ))
    }

    async fn attempt(
        &self,
        prepared: &PreparedApproval,
        deadline: Instant,
    ) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
        let (mut outcome, analytics) = run_guardian_review_session_before_deadline(
            Arc::clone(&self.session),
            self.context.clone(),
            prepared.request.clone(),
            self.reasons.clone(),
            guardian_output_schema(),
            self.options.external_cancel.clone(),
            deadline,
        )
        .await;
        let session = &self.session;
        let root_authorization_version = prepared.root_authorization_version;
        let user_message_revision = prepared.user_message_revision;
        if matches!(&outcome, GuardianReviewOutcome::Completed(assessment) if assessment.outcome == GuardianAssessmentOutcome::Allow)
            && ((session.guardian_context_mode == GuardianContextMode::ThreadOwned
                && (root_authorization_version
                    != session
                        .services
                        .agent_control
                        .root_user_authorization(session.thread_id)
                        .await
                        .map(|snapshot| snapshot.authorization_version)
                    || user_message_revision
                        != session
                            .conversation_history_snapshot()
                            .await
                            .user_message_revision()))
                || self.history_reset.is_cancelled()
                || self
                    .options
                    .external_cancel
                    .as_ref()
                    .is_some_and(CancellationToken::is_cancelled))
        {
            // A completed approval cannot outlive the owning-session or root evidence
            // it evaluated, including when either changed before prompt construction.
            outcome = GuardianReviewOutcome::Error(GuardianReviewError::Cancelled);
        }

        (outcome, analytics)
    }

    async fn emit(&self, event: EventMsg) {
        self.session.send_event(self.context.turn(), event).await;
    }

    async fn record_evidence(
        &self,
        prepared: &PreparedApproval,
        event: &codex_protocol::protocol::GuardianAssessmentEvent,
    ) {
        if let Some((evidence, action, authorization_version, root_authorization_version)) =
            &prepared.review_evidence
        {
            evidence.record(
                event,
                action,
                *authorization_version,
                *root_authorization_version,
            );
        }
    }

    async fn interrupt(&self, turn_id: &str, warning: EventMsg) {
        self.session
            .interrupt_turn_with_warning(turn_id, warning)
            .await;
    }
}
