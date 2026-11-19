//! Resolves reviewer models and adds policy context and live network state to reviewer configuration.
//! Both prewarming and reviews finish this setup before context preparation and reuse checks.
//! Inherited routing refreshes a missing default reviewer once per parent session.
//! Refresh work survives cancelled waiters and precedes checkpoint compatibility decisions.
//! Both review paths use the host's Guardian catalog when the host selects a provider.

use crate::guardian_review::ReviewRetrieval;
use codex_protocol::openai_models::CatalogModel;
use std::borrow::Cow;
use std::sync::Arc;

use codex_features::Feature;
use codex_guardian_reviewer::reviewer_instructions;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_model_provider::ProviderAuthContext;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::with_config_overrides;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;
use tokio_util::task::AbortOnDropHandle;

use crate::config::Config;
use crate::config::NetworkProxySpec;
use crate::context::ContextualUserFragment;
use crate::context::GuardianConversationHistory;

struct GuardianModelCatalogRefresh(Shared<BoxFuture<'static, ()>>);

enum GuardianCatalogLookup {
    InitializeIfNeeded,
    UseAvailable,
}

/// Adds the captured model, policy prompt and live network rules before reuse selection.
#[allow(clippy::too_many_arguments)]
pub fn build_guardian_review_session_config(
    mut guardian_config: Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
    active_model: &str,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    reasoning_summary: codex_protocol::config_types::ReasoningSummary,
    personality: Option<codex_protocol::config_types::Personality>,
    model_messages: ResolvedModelMessages<'_>,
    retrieval: ReviewRetrieval,
) -> anyhow::Result<Config> {
    guardian_config.model = Some(active_model.to_owned());
    guardian_config.model_reasoning_effort = reasoning_effort;
    guardian_config.model_reasoning_summary = Some(reasoning_summary);
    guardian_config.personality = personality;
    // The model catalog can come from user configuration. Managed rules need a trusted
    // template with policy slots unless the host explicitly supplies its own template.
    let requirements = guardian_config.config_layer_stack.requirements_toml();
    let needs_trusted_template = requirements
        .guardian_policy_config
        .iter()
        .chain(&guardian_config.guardian_extra_policy)
        .any(|policy| !policy.trim().is_empty());
    let policy_template = match guardian_config.guardian_policy_template.as_deref() {
        None if needs_trusted_template => Some(
            ResolvedModelMessages::bundled()
                .auto_review()
                .policy_template,
        ),
        template => template,
    };
    let retrieval_guidance = match &retrieval {
        ReviewRetrieval::ConversationHistory
            if guardian_config
                .features
                .enabled(Feature::GuardianConversationHistoryTools)
                && guardian_config.features.enabled(Feature::Apps) =>
        {
            GuardianConversationHistory {
                prompt: guardian_config
                    .guardian_conversation_history_prompt
                    .as_deref(),
            }
            .render()
            .into_text()
            .map(Cow::Owned)
        }
        ReviewRetrieval::ConversationHistory => None,
        ReviewRetrieval::Extension(prompt) => Some(Cow::Borrowed(prompt.as_ref())),
    };
    let instructions = reviewer_instructions(
        guardian_config.guardian_transcript_mode,
        guardian_config.resolve_guardian_policy(model_messages),
        guardian_config.guardian_extra_policy.as_deref(),
        policy_template,
        model_messages,
        retrieval_guidance.as_deref(),
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

/// Resolves the reviewer from its inference provider's catalog.
/// Inherited routing retries a missing default once before falling back to the parent;
/// host-selected routing keeps its provider's preferred model on a catalog miss.
pub(crate) async fn resolve_review_model(
    session: &crate::session::session::Session,
    context: &super::GuardianReviewContext,
) -> anyhow::Result<(codex_guardian_reviewer::ReviewModel, Arc<CatalogModel>)> {
    let turn = context.turn();
    if let Some(inference) = super::GuardianInferenceContext::for_config(
        &turn.config,
        &session.services.thread_extension_data,
    )? {
        return Ok(resolve_explicit_guardian_reviewer(
            &inference,
            GuardianCatalogLookup::InitializeIfNeeded,
        )
        .await);
    }
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
        return Ok(resolve_review_model_from_catalog(session, context).await);
    }
    Ok(resolved)
}

async fn resolve_review_model_from_catalog(
    session: &crate::session::session::Session,
    context: &super::GuardianReviewContext,
) -> (codex_guardian_reviewer::ReviewModel, Arc<CatalogModel>) {
    let turn = context.turn();
    let (review_model, model_info) = resolve_catalog_review_model(
        session.services.models_manager.as_ref(),
        &turn.config,
        &context.model_info,
        context.reasoning_effort.as_ref(),
        turn.provider.approval_review_preferred_model(),
    )
    .await;
    (
        review_model,
        model_info.map_or_else(|| Arc::clone(&context.model_info), Arc::new),
    )
}

/// Resolves async classification policy against the catalog used for Guardian requests.
pub async fn guardian_policy_model(
    thread: &crate::CodexThread,
    config: &Config,
    parent_model: Arc<ModelInfo>,
) -> anyhow::Result<Arc<ModelInfo>> {
    let thread_store = thread.thread_extension_data();
    if let Some(inference) = super::GuardianInferenceContext::for_config(config, thread_store)? {
        let (_, model) =
            resolve_explicit_guardian_reviewer(&inference, GuardianCatalogLookup::UseAvailable)
                .await;
        return Ok(Arc::clone(&model.model));
    }
    #[expect(clippy::expect_used, reason = "Required thread startup invariant")]
    let auth = thread_store
        .get::<ProviderAuthContext>()
        .expect("Core installs provider auth before starting Guardian");
    let provider = auth.model_provider(config.model_provider.clone());
    let (_, model_info) = resolve_catalog_review_model(
        thread.session.services.models_manager.as_ref(),
        config,
        &parent_model,
        config.model_reasoning_effort.as_ref(),
        provider.approval_review_preferred_model(),
    )
    .await;
    Ok(model_info.map_or(parent_model, |model| model.model))
}

async fn resolve_explicit_guardian_reviewer(
    inference: &super::GuardianInferenceContext,
    lookup: GuardianCatalogLookup,
) -> (codex_guardian_reviewer::ReviewModel, Arc<CatalogModel>) {
    let catalog = &inference.models_manager;
    let preferred = inference.provider.approval_review_preferred_model();
    let http_client_factory = inference.http_client_factory.clone();
    let model_config = ModelsManagerConfig::default();
    // Async scoring uses the available metadata while its background retry runs.
    if matches!(lookup, GuardianCatalogLookup::InitializeIfNeeded) {
        catalog
            .ensure_initialized(http_client_factory.clone())
            .await;
    }
    let eligible = eligible_review_models(
        catalog.as_ref(),
        http_client_factory,
        /*model_override*/ None,
    )
    .await;
    let reviewer = codex_guardian_reviewer::select_catalog_reviewer(eligible, preferred);
    let catalog_contains_auto_review = reviewer.is_some();
    let model_info = match reviewer {
        Some(entry) => with_reviewer_config_overrides(entry, &model_config),
        None => catalog.get_model_info(preferred, &model_config).await,
    };
    // An advertised reviewer gets an explicit "none" when it has no default effort.
    let fallback_effort = model_info
        .default_reasoning_level
        .clone()
        .or_else(|| catalog_contains_auto_review.then_some(ReasoningEffort::None));
    let reasoning_effort =
        codex_guardian_reviewer::preferred_reasoning_effort(&model_info, fallback_effort);
    let review_model = codex_guardian_reviewer::ReviewModel {
        model: preferred.to_owned(),
        reasoning_effort,
        default_review_model_id: preferred.to_owned(),
        catalog_contains_auto_review,
        model_overridden: false,
        model_override: None,
    };
    (review_model, Arc::new(model_info))
}

async fn resolve_catalog_review_model(
    models_manager: &dyn ModelsManager,
    config: &Config,
    parent_model: &ModelInfo,
    reasoning_effort: Option<&ReasoningEffort>,
    default_review_model_id: &str,
) -> (codex_guardian_reviewer::ReviewModel, Option<CatalogModel>) {
    let catalog = eligible_review_models(
        models_manager,
        config.http_client_factory(),
        parent_model.auto_review_model_override.as_deref(),
    )
    .await;
    let (review_model, reviewer) = codex_guardian_reviewer::select_review_model(
        parent_model,
        reasoning_effort,
        default_review_model_id,
        catalog,
    );
    let guardian_model_info = if review_model.model_overridden {
        // Explicit names can resolve through aliases or synthetic metadata.
        Some(
            models_manager
                .get_model_info(&review_model.model, &config.to_models_manager_config())
                .await,
        )
    } else {
        reviewer
            .map(|entry| with_reviewer_config_overrides(entry, &config.to_models_manager_config()))
    };
    (review_model, guardian_model_info)
}

fn with_reviewer_config_overrides(
    entry: CatalogModel,
    config: &ModelsManagerConfig,
) -> CatalogModel {
    CatalogModel {
        model: with_config_overrides(Arc::unwrap_or_clone(entry.model), config).into(),
        origin: entry.origin,
    }
}

/// Preserve provider eligibility while selecting from the latest in-memory catalog.
async fn eligible_review_models(
    models_manager: &dyn ModelsManager,
    http_client_factory: HttpClientFactory,
    model_override: Option<&str>,
) -> Vec<CatalogModel> {
    // Keep a successful refresh even if writing it to the disk cache failed.
    let mut catalog = models_manager
        .model_catalog_with_origins(RefreshStrategy::InMemory, http_client_factory)
        .await;
    let uses_codex_backend = models_manager
        .auth_manager()
        .is_some_and(AuthManager::current_auth_uses_codex_backend);
    catalog.retain(|entry| {
        (uses_codex_backend || entry.supported_in_api)
            && (model_override.is_some()
                || matches!(
                    entry.origin,
                    codex_models_manager::ModelOrigin::Provider
                        | codex_models_manager::ModelOrigin::ExplicitConfig
                ))
    });
    catalog
}
