//! Vertex AI provider routing, command-backed credentials, and supported model capabilities.

use std::path::PathBuf;
use std::sync::Arc;

use codex_api::Provider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::manager::StaticModelsManager;
use codex_protocol::account::ProviderAccount;
use codex_protocol::error::CodexErr;
use codex_protocol::error::EnvVarError;
use codex_protocol::error::Result;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ToolMode;

use crate::ProviderCapabilities;
use crate::RemoteCompactionSupport;
use crate::auth::auth_manager_for_provider;
use crate::auth::resolve_provider_auth;
use crate::provider::ModelProvider;
use crate::provider::ModelProviderFuture;
use crate::provider::ProviderAccountResult;
use crate::provider::ProviderAccountState;

const GOOGLE_CLOUD_PROJECT_ENV_VAR: &str = "GOOGLE_CLOUD_PROJECT";
const GOOGLE_CLOUD_LOCATION_ENV_VAR: &str = "GOOGLE_CLOUD_LOCATION";
const DEFAULT_GOOGLE_CLOUD_LOCATION: &str = "global";

/// Runtime provider for Google Cloud Vertex AI's OpenAI-compatible endpoint.
#[derive(Clone, Debug)]
pub(crate) struct GcpModelProvider {
    info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
}

impl GcpModelProvider {
    pub(crate) fn new(
        provider_info: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Self {
        let auth_manager = auth_manager_for_provider(auth_manager, &provider_info);
        Self {
            info: provider_info,
            auth_manager,
        }
    }

    async fn auth(&self) -> Option<CodexAuth> {
        match self.auth_manager.as_ref() {
            Some(auth_manager) => auth_manager.auth().await,
            None => None,
        }
    }

    async fn api_provider(&self) -> Result<Provider> {
        let mut provider_info = self.info.clone();
        provider_info.base_url = self.runtime_base_url().await?;
        provider_info.to_api_provider(/*auth_mode*/ None)
    }

    async fn runtime_base_url(&self) -> Result<Option<String>> {
        if let Some(base_url) = self.info.base_url.clone() {
            return Ok(Some(base_url));
        }

        let project = std::env::var(GOOGLE_CLOUD_PROJECT_ENV_VAR).ok();
        let location = std::env::var(GOOGLE_CLOUD_LOCATION_ENV_VAR).ok();
        vertex_base_url(&self.info, project.as_deref(), location.as_deref()).map(Some)
    }
}

impl ModelProvider for GcpModelProvider {
    fn info(&self) -> &ModelProviderInfo {
        &self.info
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            reasoning_summaries: false,
            image_generation: false,
            web_search: false,
            external_web_access: false,
            remote_compaction: RemoteCompactionSupport::V2,
        }
    }

    fn approval_review_preferred_model(&self) -> &'static str {
        // TODO: Switch Guardian to gpt-5.6-luna once it is available on GCP.
        "gpt-6-luna"
    }

    fn memory_extraction_preferred_model(&self) -> &'static str {
        "gpt-6-luna"
    }

    fn memory_consolidation_preferred_model(&self) -> &'static str {
        "gpt-6.1-terra"
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    fn auth(&self) -> ModelProviderFuture<'_, Option<CodexAuth>> {
        Box::pin(GcpModelProvider::auth(self))
    }

    fn account_state(&self) -> ProviderAccountResult {
        Ok(ProviderAccountState {
            account: Some(ProviderAccount::Gcp),
            requires_openai_auth: false,
        })
    }

    fn api_provider(&self) -> ModelProviderFuture<'_, Result<Provider>> {
        Box::pin(GcpModelProvider::api_provider(self))
    }

    fn api_auth(&self) -> ModelProviderFuture<'_, Result<SharedAuthProvider>> {
        Box::pin(async move {
            let auth = self.auth().await.ok_or_else(|| {
                CodexErr::InvalidRequest(
                    "Google Cloud credentials are unavailable or expired. Run \
`gcloud auth application-default login` to reauthenticate."
                        .to_string(),
                )
            })?;
            resolve_provider_auth(Some(&auth), &self.info)
        })
    }

    fn runtime_base_url(&self) -> ModelProviderFuture<'_, Result<Option<String>>> {
        Box::pin(GcpModelProvider::runtime_base_url(self))
    }

    fn models_manager(
        &self,
        _codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        self.models_manager_without_cache(config_model_catalog)
    }

    fn models_manager_without_cache(
        &self,
        config_model_catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        let origin = if config_model_catalog.is_some() {
            codex_models_manager::ModelOrigin::ExplicitConfig
        } else {
            codex_models_manager::ModelOrigin::Provider
        };
        let mut model_catalog = config_model_catalog
            .or_else(|| codex_models_manager::bundled_models_response().ok())
            .unwrap_or_default();
        model_catalog.models.retain(|model| {
            matches!(
                model.slug.as_str(),
                "gpt-6-astra" | "gpt-6-luna" | "gpt-6.1-sol" | "gpt-6.1-terra"
            )
        });
        // Vertex supports function tools, namespaces, and client-side tool search, but not
        // Responses Lite or custom tools.
        for model in &mut model_catalog.models {
            model.use_responses_lite = false;
            model.tool_mode = Some(ToolMode::Direct);
            model.apply_patch_tool_type = None;
        }
        Arc::new(StaticModelsManager::new(
            self.auth_manager.clone(),
            model_catalog,
            origin,
        ))
    }
}

fn vertex_base_url(
    info: &ModelProviderInfo,
    environment_project: Option<&str>,
    environment_location: Option<&str>,
) -> Result<String> {
    let project = info
        .project_id
        .as_deref()
        .map(str::trim)
        .or_else(|| {
            environment_project
                .map(str::trim)
                .filter(|project| !project.is_empty())
        })
        .ok_or_else(|| {
            CodexErr::EnvVar(EnvVarError {
                var: GOOGLE_CLOUD_PROJECT_ENV_VAR.to_string(),
                instructions: Some(
                    "Set model_providers.gcp.project_id or GOOGLE_CLOUD_PROJECT to your Google \
Cloud project ID, or configure model_providers.gcp.base_url."
                        .to_string(),
                ),
            })
        })?;
    let location = info
        .location
        .as_deref()
        .map(str::trim)
        .or_else(|| {
            environment_location
                .map(str::trim)
                .filter(|location| !location.is_empty())
        })
        .unwrap_or(DEFAULT_GOOGLE_CLOUD_LOCATION);

    for (name, value) in [
        (
            if info.project_id.is_some() {
                "model_providers.gcp.project_id"
            } else {
                GOOGLE_CLOUD_PROJECT_ENV_VAR
            },
            project,
        ),
        (
            if info.location.is_some() {
                "model_providers.gcp.location"
            } else {
                GOOGLE_CLOUD_LOCATION_ENV_VAR
            },
            location,
        ),
    ] {
        if value.is_empty()
            || !value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return Err(CodexErr::InvalidRequest(format!(
                "{name} must be non-empty and contain only ASCII letters, digits, and hyphens"
            )));
        }
    }

    Ok(format!(
        "https://aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/openai/v1"
    ))
}

#[cfg(test)]
#[path = "gcp_tests.rs"]
mod tests;
