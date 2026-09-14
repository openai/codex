//! Registers Guardian classification and handles thread, skill, and tool lifecycle hooks.

use std::sync::Arc;
use std::sync::Weak;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::context::GuardianReviewEvidence;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::GuardianV2Enabled;
use codex_extension_api::SkillInvocationContributor;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::has_full_access;

use super::config::GuardianV2Config;
use super::sampler::LunaSampler;
use super::score::GuardianV2ScoreProgress;
use super::trusted_skills::TrustedSkillRoots;

#[derive(Clone)]
pub(super) struct GuardianV2Extension {
    auth_manager: Arc<AuthManager>,
    pub(super) event_sink: Arc<dyn ExtensionEventSink>,
    pub(super) thread_manager: Weak<ThreadManager>,
}

impl ThreadLifecycleContributor<Config> for GuardianV2Extension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if !input.config.features.enabled(Feature::GuardianApproval) {
                return;
            }

            let model = input.thread_store.get::<ModelInfo>();
            let thread_id = input.thread_store.level_id().to_string();
            let guardian_config = match GuardianV2Config::resolve(input.config) {
                Ok(config) => config,
                Err(error) => {
                    self.event_sink.emit_warning(ExtensionWarning {
                        thread_id,
                        turn_id: None,
                        message: error,
                    });
                    return;
                }
            };
            let mut policy = guardian_config.policy_for_model(model.as_deref());
            if model.as_ref().is_some_and(|model| {
                input
                    .config
                    .config_layer_stack
                    .requirements()
                    .auto_review_required_for_model(&model.slug)
            }) {
                policy.enforce_required_model();
            }
            let scoring_enabled = policy.scoring_enabled();
            let sampler_config = super::startup::sampler_config(
                &input,
                Arc::clone(&self.auth_manager),
                self.thread_manager.upgrade(),
            )
            .await;

            if scoring_enabled && guardian_config.transcript.include_images {
                input
                    .thread_store
                    .get_or_init(NodeReplReviewEvidence::default)
                    .enable_image_capture();
            }
            input.thread_store.remove::<LunaSampler>();
            let sampler = input
                .thread_store
                .get_or_init(|| LunaSampler::new(sampler_config));
            input.thread_store.insert(guardian_config);
            input.thread_store.insert(GuardianV2ScoreProgress {
                metrics: input.extension_metrics.clone(),
                ..Default::default()
            });
            // Preserve the answer path selected by the host for this thread.
            input
                .thread_store
                .get_or_init(GuardianReviewEvidence::default);
            input
                .thread_store
                .insert(TrustedSkillRoots::from_config(input.config));
            if scoring_enabled {
                input.thread_store.insert(GuardianV2Enabled);
            }

            // Keep the sampler available for later automatic review, but do not
            // prewarm while User approval mode or Full Access is selected.
            if scoring_enabled
                && input.config.approvals_reviewer == ApprovalsReviewer::AutoReview
                && !has_full_access(
                    input.config.permissions.approval_policy.value(),
                    &input.config.permissions.effective_permission_profile(),
                    input
                        .environments
                        .iter()
                        .map(|environment| &environment.config),
                )
            {
                tokio::spawn(async move {
                    sampler.prewarm().await;
                });
            }
        })
    }
}

impl SkillInvocationContributor for GuardianV2Extension {
    fn requires_host_skill_discovery(&self) -> bool {
        false
    }

    fn on_skill_invocation<'a>(
        &'a self,
        input: SkillInvocationInput<'a>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Some(roots) = input.thread_store.get::<TrustedSkillRoots>() else {
                return;
            };
            let Some(skill_path) = roots.trusted_skill_path(input.skill_resource) else {
                return;
            };
            let Some(evidence) = input.thread_store.get::<GuardianReviewEvidence>() else {
                return;
            };
            evidence.record_trusted_skill(input.turn_id, skill_path);
        })
    }
}

impl ToolLifecycleContributor for GuardianV2Extension {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(self.score_tool(input))
    }

    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if let Some(progress) = input.thread_store.get::<GuardianV2ScoreProgress>() {
                progress
                    .oversized_tool_calls
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .remove(input.call_id);
            }
        })
    }
}

/// Installs feature-gated Guardian V2 tool classification for each thread.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    thread_manager: Weak<ThreadManager>,
) {
    let extension = Arc::new(GuardianV2Extension {
        auth_manager,
        event_sink: registry.event_sink(),
        thread_manager,
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.approval_review_contributor(Arc::new(super::approval::GuardianApprovalReviewer {
        thread_manager: extension.thread_manager.clone(),
    }));
    registry.skill_invocation_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension);
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
