//! Resolves the classifier's own model budget and transport configuration.
//! Parent-model context overrides do not change the classifier allowance.

use std::sync::Arc;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::cyber_access_program::ApiKeyCyberAccessPrograms;
use codex_extension_api::ThreadOriginator;
use codex_extension_api::ThreadStartInput;
use codex_features::Feature;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthManager;
use codex_model_provider::ProviderAuthContext;

use super::decisions::DecisionsCredential;
use super::decisions::DecisionsError;
use super::decisions::DecisionsSampler;
use super::decisions::URL;
use super::sampler::LunaSamplerConfig;
use super::sampler::MODEL;

// Decisions setup failures are telemetry only; they never affect the baseline sampler.
pub(super) fn decisions_sampler(
    input: &ThreadStartInput<'_, Config>,
    auth_manager: &Arc<AuthManager>,
) -> Option<DecisionsSampler> {
    use codex_agent_identity::ChatGptEnvironment;
    let auth_context = input.thread_store.get::<ProviderAuthContext>();
    let result = decisions_api_key(
        &input.config.model_provider_id,
        input.config.model_provider.base_url.as_deref(),
        auth_context.as_deref(),
        auth_manager,
    )
    .and_then(|key| {
        let url = if matches!(
            ChatGptEnvironment::from_chatgpt_base_url(&input.config.chatgpt_base_url),
            Ok(ChatGptEnvironment::Staging)
        ) {
            "https://api.openai.org/v1/decisions"
        } else {
            URL
        };
        DecisionsSampler::new(
            input.config.http_client_factory(),
            key,
            url.to_owned(),
            auth_context.as_deref(),
        )
    });
    match result {
        Ok(sampler) => Some(sampler),
        Err(error) => {
            if let Some(metrics) = input.extension_metrics.as_deref() {
                metrics.counter(
                    "codex.guardian_v2.decisions_comparison.setup_failure",
                    /*inc*/ 1,
                    &[("reason", super::metrics::decisions_failure_reason(error))],
                );
            }
            None
        }
    }
}

pub(super) fn decisions_api_key(
    provider_id: &str,
    base_url: Option<&str>,
    context: Option<&ProviderAuthContext>,
    fallback: &Arc<AuthManager>,
) -> Result<DecisionsCredential, DecisionsError> {
    match std::env::var("CODEX_GUARDIAN_DECISIONS_API_KEY") {
        Ok(key) => Ok(DecisionsCredential::ApiKey(key)),
        Err(std::env::VarError::NotPresent) if provider_id == "openai" && base_url.is_none() => {
            let manager = context.map_or(fallback, |context| &context.credentials);
            if let Some(auth) = manager.auth_cached()
                && let Some(key) = auth.api_key()
            {
                if !valid_implicit_decisions_key(key) {
                    return Err(DecisionsError::Credentials);
                }
                if context.is_some_and(|context| context.runtime_auth.is_some()) {
                    let state = manager
                        .auth_change_state_for(&auth)
                        .ok_or(DecisionsError::Credentials)?;
                    return Ok(DecisionsCredential::RuntimeLogin(
                        Arc::clone(manager),
                        state.source_generation,
                    ));
                }
                return Ok(DecisionsCredential::SavedLogin(Arc::clone(manager)));
            }
            codex_login::read_openai_api_key_from_env()
                .filter(|key| valid_implicit_decisions_key(key))
                .map(DecisionsCredential::ApiKey)
                .ok_or(DecisionsError::Credentials)
        }
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => {
            Err(DecisionsError::Credentials)
        }
    }
}

// Decisions uses a commercial endpoint. Reject padded keys without rewriting credentials.
pub(super) fn valid_implicit_decisions_key(key: &str) -> bool {
    !key.is_empty() && key.trim() == key && !codex_login::auth::is_fedramp_api_key(key)
}

pub(super) async fn sampler_config(
    input: &ThreadStartInput<'_, Config>,
    manager: Option<Arc<ThreadManager>>,
) -> LunaSamplerConfig {
    let inherit_parent_cyber_access_program =
        codex_core::RequiredCyberAccessPrograms::for_thread(input.thread_store).is_none();
    let luna_model = if let Some(thread_manager) = manager {
        let mut model_config = input.config.to_models_manager_config();
        model_config.model_context_window = None;
        Some(
            thread_manager
                .get_models_manager()
                .get_model_info(MODEL, &model_config)
                .await,
        )
    } else {
        None
    };
    let max_input_tokens =
        luna_model
            .as_ref()
            .map_or(codex_guardian_context::DEFAULT_MAX_INPUT_TOKENS, |model| {
                codex_guardian_context::effective_input_token_limit(
                    model, /*configured_window*/ None,
                )
            });
    let luna_compaction_hash = luna_model.and_then(|model| model.comp_hash.clone());
    #[expect(clippy::expect_used, reason = "Required thread startup invariant")]
    let auth = input
        .thread_store
        .get::<codex_model_provider::ProviderAuthContext>()
        .expect("Core installs provider auth before starting Guardian");
    LunaSamplerConfig {
        api_key_cyber_access_programs: ApiKeyCyberAccessPrograms::from_config(input.config),
        inherit_parent_cyber_access_program,
        workspace_routing: input
            .thread_store
            .get_or_init(|| input.config.workspace_routing_context())
            .as_ref()
            .clone(),
        provider: auth.model_provider(input.config.model_provider.clone()),
        http_client_factory: input.config.http_client_factory(),
        agent_identity_policy: if input.config.features.enabled(Feature::UseAgentIdentity) {
            AgentIdentityAuthPolicy::ChatGptAuth
        } else {
            AgentIdentityAuthPolicy::JwtOnly
        },
        session_source: input.session_source.clone(),
        session_id: input.session_store.level_id().to_string(),
        thread_id: input.thread_store.level_id().to_string(),
        originator: input
            .thread_store
            .get::<ThreadOriginator>()
            .map(|originator| originator.0.clone()),
        service_tier: input.config.service_tier.clone(),
        luna_compaction_hash,
        max_input_tokens,
        metrics: input.extension_metrics.clone(),
    }
}
