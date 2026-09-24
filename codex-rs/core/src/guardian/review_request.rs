//! Binds an immutable action and root lifetime to fresh per-attempt authorization evidence.
//! The extension chooses effects; this adapter supplies evidence, validation and publication.

use super::*;
use crate::codex_thread::GuardianAuthorizationVersion;
use codex_guardian_reviewer::ReviewHost;
use codex_protocol::approvals::GuardianReviewReason;

pub(in crate::guardian) struct PreparedApproval {
    request: GuardianApprovalRequest,
    // Unlike authorization, the original history lifetime cannot advance on retry.
    root_history: Option<(codex_protocol::ThreadId, u64)>,
    formatted_action: Option<String>,
}

pub(in crate::guardian) struct ApprovalEvidence {
    store: Arc<GuardianReviewEvidence>,
    authorization_version: GuardianAuthorizationVersion,
    root_authorization_version: Option<GuardianAuthorizationVersion>,
}

impl ReviewHost for super::super::runtime::ReviewRuntime {
    type Prepared = PreparedApproval;
    type Evidence = ApprovalEvidence;

    fn permissions(&self) -> Option<codex_guardian_context::PermissionContext> {
        let request = self.request.request.as_ref().ok()?;
        crate::guardian::permissions::for_environment(
            &self.context,
            request.target_environment_id(),
        )
        .ok()
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
        review_id: &str,
        review_reason: GuardianReviewReason,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<(PreparedApproval, codex_guardian_reviewer::ReviewReport), ReviewDecision> {
        let super::super::runtime::ReviewRuntime {
            session,
            history_reset: _,
            context,
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
            external_cancel: _,
            require_synchronous_review: _,
            require_guardian: _,
        } = options;
        let target_item_id = guardian_request_target_item_id(&request).map(str::to_string);
        let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
        let plugin_attribution = match plugin_attribution_override {
            Some(attribution) => Some(attribution),
            None if matches!(&request, GuardianApprovalRequest::ExecCommand { .. }) => {
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
                review_id: review_id.to_owned(),
                target_item_id,
                plugin_id,
                script_path,
                approval_request_source,
                reviewed_action: guardian_reviewed_action(&request),
                action: guardian_assessment_action(&request),
                review_reason,
                model_context,
            });
        let root_history = session
            .services
            .agent_control
            .get_guardian_package(session.thread_id)
            .await
            .map(|snapshot| (snapshot.root_thread_id, snapshot.history_reset_version));
        Ok((
            PreparedApproval {
                formatted_action: format_guardian_action_pretty(&request).ok(),
                request,
                root_history,
            },
            report,
        ))
    }

    async fn attempt(
        &self,
        prepared: &PreparedApproval,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> (
        GuardianReviewOutcome,
        GuardianReviewAnalyticsResult,
        Option<ApprovalEvidence>,
    ) {
        if self.history_reset.is_cancelled() || cancellation.is_cancelled() {
            return (
                GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
                GuardianReviewAnalyticsResult::without_session(),
                None,
            );
        }
        // Every attempt rebuilds the prompt from live history. Capture its revision and
        // cache attribution anew too; a retried verdict must not use the first snapshot.
        let session = &self.session;
        let root_snapshot = session
            .services
            .agent_control
            .get_guardian_package(session.thread_id)
            .await;
        if prepared.root_history
            != root_snapshot
                .as_ref()
                .map(|snapshot| (snapshot.root_thread_id, snapshot.history_reset_version))
        {
            return (
                GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
                GuardianReviewAnalyticsResult::without_session(),
                None,
            );
        }
        let root_authorization_version =
            root_snapshot.map(|snapshot| snapshot.authorization_version);
        // Keep the authorization revision even when no cacheable review evidence exists.
        let history = session.conversation_history_snapshot().await;
        let user_message_revision = history.user_message_revision();
        let review_evidence = session
            .services
            .thread_extension_data
            .get::<GuardianReviewEvidence>()
            .map(|store| ApprovalEvidence {
                authorization_version: store.authorization_version(history.as_ref()),
                store,
                root_authorization_version,
            });
        drop(history);
        let (mut outcome, analytics) = run_guardian_review_session_before_deadline(
            Arc::clone(&self.session),
            self.context.clone(),
            prepared.request.clone(),
            self.request.category,
            self.reasons.clone(),
            Some(cancellation.clone()),
            deadline,
        )
        .await;
        if matches!(&outcome, GuardianReviewOutcome::Completed(assessment) if assessment.outcome == GuardianAssessmentOutcome::Allow)
        {
            let root_snapshot = session
                .services
                .agent_control
                .get_guardian_package(session.thread_id)
                .await;
            let root_history_changed = prepared.root_history
                != root_snapshot
                    .as_ref()
                    .map(|snapshot| (snapshot.root_thread_id, snapshot.history_reset_version));
            let authorization_changed = root_authorization_version
                != root_snapshot.map(|snapshot| snapshot.authorization_version)
                || user_message_revision
                    != session
                        .conversation_history_snapshot()
                        .await
                        .user_message_revision();
            // An actual stop or history reset wins over a concurrent authorization change.
            if self.history_reset.is_cancelled()
                || cancellation.is_cancelled()
                || root_history_changed
            {
                outcome = GuardianReviewOutcome::Error(GuardianReviewError::Cancelled);
            } else if authorization_changed {
                tracing::info!(thread_id = %session.thread_id, "Guardian approval invalidated by an authorization change");
                outcome = GuardianReviewOutcome::Error(GuardianReviewError::StaleAuthorization);
            }
        }

        let review_evidence = match &outcome {
            GuardianReviewOutcome::Completed(_) => review_evidence,
            GuardianReviewOutcome::Error(_) => None,
        };
        (outcome, analytics, review_evidence)
    }

    fn validate_action(&self) -> Result<(&str, Option<&str>), ReviewDecision> {
        let request = self.request.validate(&self.context)?;
        Ok((
            guardian_request_turn_id(request, &self.context.turn().sub_id),
            guardian_request_target_item_id(request),
        ))
    }

    async fn emit(&self, event: EventMsg) {
        self.session.send_event(self.context.turn(), event).await;
    }

    async fn record_evidence(
        &self,
        prepared: &PreparedApproval,
        evidence: ApprovalEvidence,
        event: &codex_protocol::protocol::GuardianAssessmentEvent,
    ) {
        if let Some(action) = &prepared.formatted_action {
            evidence.store.record(
                event,
                action,
                evidence.authorization_version,
                evidence.root_authorization_version,
            );
        }
    }

    async fn interrupt(&self, turn_id: &str, warning: EventMsg) {
        self.session
            .interrupt_turn_with_warning(turn_id, warning)
            .await;
    }
}
