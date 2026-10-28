//! Resolves reviewer models and adds policy context and live network state to reviewer configuration.
//! Both prewarming and reviews finish this setup before context preparation and reuse checks.
//! A missing default reviewer gets one shared catalog refresh per parent session.
//! Refresh work survives cancelled waiters and precedes checkpoint compatibility decisions.

use std::sync::Arc;

use codex_features::Feature;
use codex_guardian_reviewer::reviewer_instructions;
use codex_models_manager::manager::RefreshStrategy;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::BaseInstructionsProvenance;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use tokio_util::task::AbortOnDropHandle;

use crate::config::Config;
use crate::config::NetworkProxySpec;
use crate::context::ContextualUserFragment;
use crate::context::GuardianConversationHistory;

struct GuardianModelCatalogRefresh(Shared<BoxFuture<'static, ()>>);

/// Adds the captured model, policy prompt and live network rules before reuse selection.
pub fn build_guardian_review_session_config(
    mut guardian_config: Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    reasoning_summary: codex_protocol::config_types::ReasoningSummary,
    personality: Option<codex_protocol::config_types::Personality>,
    model_messages: ResolvedModelMessages<'_>,
) -> anyhow::Result<Config> {
    guardian_config.model = Some(active_model.to_owned());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.model_reasoning_summary = Some(reasoning_summary);
    guardian_config.personality = personality;
    let conversation_history = (guardian_config
        .features
        .enabled(Feature::GuardianConversationHistoryTools)
        && guardian_config.features.enabled(Feature::Apps))
    .then(|| {
        GuardianConversationHistory {
            prompt: guardian_config
                .guardian_conversation_history_prompt
                .as_deref(),
        }
        .render()
        .into_text()
        .unwrap_or_default()
    });
    let instructions = reviewer_instructions(
        guardian_config.guardian_transcript_mode,
        guardian_config.resolve_guardian_policy(model_messages),
        guardian_config.guardian_extra_policy.as_deref(),
        guardian_config.guardian_policy_template.as_deref(),
        model_messages,
        conversation_history.as_deref(),
    );
    guardian_config.base_instructions = Some(instructions);
    guardian_config.base_instructions_provenance = Some(BaseInstructionsProvenance::Custom);
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
            guardian_config.permissions.permission_profile(),
        )?);
    }
    Ok(guardian_config)
}

/// Retries a missing default reviewer once before retaining the parent fallback.
pub(crate) async fn resolve_review_model(
    session: &crate::session::session::Session,
    context: &super::GuardianReviewContext,
) -> (
    codex_guardian_reviewer::ReviewModel,
    Arc<codex_protocol::openai_models::ModelInfo>,
) {
    let turn = context.turn();
    // Check before resolving: a refresh that finishes during the lookup still
    // requires a reread, but later calls only need one catalog resolution.
    let refresh_completed = session
        .services
        .session_extension_data
        .get::<GuardianModelCatalogRefresh>()
        .is_some_and(|refresh| refresh.0.peek().is_some());
    let resolved = resolve_review_model_from_catalog(session, context).await;
    let (review_model, _) = &resolved;
    if !refresh_completed
        && review_model.default_review_model_id == "codex-auto-review"
        && !review_model.model_overridden
        && !review_model.catalog_contains_auto_review
        && turn.config.model_catalog.is_none()
        && turn.provider.info().model_catalog_url.is_none()
        && !super::is_basic_session_source(&turn.session_source)
    {
        // The session retains the task through waiter cancellation. Dropping the
        // session aborts unfinished work; a failed fetch still completes the attempt.
        let refresh = session.services.session_extension_data.get_or_init(|| {
            let manager = Arc::clone(&session.services.models_manager);
            let http_client_factory = turn.config.http_client_factory();
            let task = AbortOnDropHandle::new(tokio::spawn(async move {
                manager
                    .list_models(RefreshStrategy::Online, http_client_factory)
                    .await;
            }));
            GuardianModelCatalogRefresh(
                async move {
                    let _ = task.await;
                }
                .boxed()
                .shared(),
            )
        });
        refresh.0.clone().await;
        return resolve_review_model_from_catalog(session, context).await;
    }
    resolved
}

async fn resolve_review_model_from_catalog(
    session: &crate::session::session::Session,
    context: &super::GuardianReviewContext,
) -> (
    codex_guardian_reviewer::ReviewModel,
    Arc<codex_protocol::openai_models::ModelInfo>,
) {
    let turn = context.turn();
    // Keep a successful refresh even if writing it to the disk cache failed.
    let models_manager = &session.services.models_manager;
    let catalog = models_manager
        .model_catalog_with_origins(RefreshStrategy::InMemory, turn.config.http_client_factory())
        .await;
    let available_models = models_manager.build_available_models(
        catalog
            .into_iter()
            .filter(|entry| {
                context.model_info.auto_review_model_override.is_some()
                    || entry.origin != codex_models_manager::ModelOrigin::Bundled
            })
            .map(|entry| entry.model)
            .collect(),
    );
    let default_review_model_id = turn.provider.approval_review_preferred_model();
    let review_model = codex_guardian_reviewer::select_review_model(
        &context.model_info,
        context.reasoning_effort.as_ref(),
        default_review_model_id,
        &available_models,
    );
    // Resolve a separate reviewer against the current catalog on every attempt.
    // Parent fallback must retain the action's metadata even after a catalog refresh.
    let guardian_model_info =
        if !review_model.catalog_contains_auto_review && !review_model.model_overridden {
            Arc::clone(&context.model_info)
        } else {
            Arc::new(
                session
                    .services
                    .models_manager
                    .get_model_info(
                        review_model.model.as_str(),
                        &turn.config.to_models_manager_config(),
                    )
                    .await,
            )
        };
    (review_model, guardian_model_info)
}
