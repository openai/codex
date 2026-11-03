use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use codex_api::AgentIdentityTelemetry;
use codex_api::ModelsClient;
use codex_api::RequestTelemetry;
use codex_api::ReqwestTransport;
use codex_api::TransportError;
use codex_api::auth_header_telemetry;
use codex_api::map_api_error;
use codex_features::RuntimeAuthMode;
use codex_feedback::FeedbackRequestTags;
use codex_feedback::emit_feedback_request_tags_with_auth_env;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_login::AuthEnvTelemetry;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::GatewayAuthManager;
use codex_login::collect_auth_env_telemetry;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::create_client_for_route_async;
use codex_model_provider_info::CHATGPT_CODEX_BASE_URL;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::ModelCatalogOriginator;
use codex_models_manager::manager::ModelsEndpointClient;
use codex_models_manager::manager::ModelsEndpointFuture;
use codex_models_manager::manager::ModelsEndpointResponse;
use codex_otel::TelemetryAuthMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CoreResult;
use codex_response_debug_context::extract_response_debug_context;
use codex_response_debug_context::telemetry_transport_error_message;
use codex_runtime_agent_instance::RuntimeAuthManager;
use http::HeaderMap;
use tokio::time::timeout;

use crate::auth::agent_identity_telemetry;
use crate::auth::uses_fedramp_api_key;
use crate::combined_auth::compose_auth;
use crate::provider::resolve_api_provider;
use crate::request_setup::resolve_request_auth;

const MODELS_REFRESH_TIMEOUT: Duration = Duration::from_secs(5);
const MODELS_ENDPOINT: &str = "/models";
const FEDRAMP_CODEX_BASE_URL: &str = "https://gov.chatgpt.com/backend-api/codex";

/// Provider-owned OpenAI-compatible `/models` endpoint.
#[derive(Debug)]
pub(crate) struct OpenAiModelsEndpoint {
    provider_info: ModelProviderInfo,
    auth_manager: Option<Arc<AuthManager>>,
    gateway_auth_manager: Option<Result<Arc<GatewayAuthManager>, String>>,
    transport_builder: Arc<dyn ModelsTransportBuilder>,
    runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
}

impl OpenAiModelsEndpoint {
    pub(crate) fn new(
        provider_info: ModelProviderInfo,
        auth_manager: Option<Arc<AuthManager>>,
        gateway_auth_manager: Option<Result<Arc<GatewayAuthManager>, String>>,
    ) -> Self {
        let redirect_policy = if provider_info.model_catalog_url.is_some() {
            ClientRedirectPolicy::Reject
        } else {
            ClientRedirectPolicy::Default
        };
        Self {
            runtime_auth: None,
            provider_info,
            auth_manager,
            gateway_auth_manager,
            transport_builder: Arc::new(RouteAwareModelsTransportBuilder { redirect_policy }),
        }
    }

    /// Use the same runtime owner as the harness's model sessions.
    pub(crate) fn with_runtime_auth(
        mut self,
        runtime_auth: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    ) -> Self {
        self.runtime_auth = runtime_auth;
        self
    }

    async fn auth(&self) -> Option<CodexAuth> {
        let auth_manager = self.auth_manager.clone()?;
        // Catalog startup has its own deadline. Let rotating credentials finish
        // saving even when that caller stops waiting.
        match tokio::spawn(async move { auth_manager.auth().await }).await {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!(?error, "model catalog authentication task failed");
                None
            }
        }
    }

    async fn uses_codex_backend(&self) -> bool {
        self.auth()
            .await
            .as_ref()
            .is_some_and(CodexAuth::uses_codex_backend)
    }

    async fn list_models(
        &self,
        client_version: &str,
        http_client_factory: HttpClientFactory,
    ) -> CoreResult<ModelsEndpointResponse> {
        timeout(
            MODELS_REFRESH_TIMEOUT,
            self.fetch_models(client_version, http_client_factory),
        )
        .await
        .map_err(|_| CodexErr::RequestTimeout)?
    }

    async fn fetch_models(
        &self,
        client_version: &str,
        http_client_factory: HttpClientFactory,
    ) -> CoreResult<ModelsEndpointResponse> {
        let resolved = resolve_request_auth(
            self.provider_info.clone(),
            self.auth_manager.clone(),
            self.runtime_auth.clone(),
            /*scope*/ None,
            self.provider_info
                .model_catalog_url
                .as_deref()
                .map(String::as_str)
                .or(self.provider_info.base_url.as_deref())
                .map(str::to_owned),
        )
        .await?;
        let auth = resolved.credential.clone();
        let metric_auth_mode = if self.has_provider_api_key()
            || auth.as_ref().is_some_and(CodexAuth::is_api_key_auth)
        {
            "api_key"
        } else if auth.is_some() {
            "chatgpt"
        } else {
            "none"
        };
        let _timer = codex_otel::start_global_timer(
            "codex.remote_models.fetch_update.duration_ms",
            &[("auth_mode", metric_auth_mode)],
        );
        let identity = crate::models_identity::identity(&self.provider_info, auth.as_ref())?;
        let auth_mode = auth.as_ref().map(CodexAuth::auth_mode);
        let mut api_provider = resolve_api_provider(&self.provider_info, auth.as_ref())?;
        if (auth.as_ref().is_some_and(CodexAuth::is_api_key_auth) || self.has_provider_api_key())
            && self.supports_api_key_models()
            && self.provider_info.base_url.is_none()
            && self.provider_info.model_catalog_url.is_none()
        {
            // Codex metadata is served by the Codex backend, not the public /v1/models API.
            api_provider.base_url = if uses_fedramp_api_key(auth.as_ref(), &self.provider_info)? {
                FEDRAMP_CODEX_BASE_URL
            } else {
                CHATGPT_CODEX_BASE_URL
            }
            .to_string();
        }
        let http_client_factory = resolved
            .bind_http_client_factory(http_client_factory)
            .map_err(|error| map_api_error(error.into()))?;
        resolved.validate_headers(&api_provider.headers)?;
        let resolved = compose_auth(
            &self.provider_info,
            self.gateway_auth_manager.as_ref(),
            resolved,
        )
        .await?;
        let api_auth = resolved.auth;
        let request_url = match self.provider_info.model_catalog_url.as_deref() {
            Some(catalog_url) => ModelsClient::<ReqwestTransport>::catalog_request_url(
                &api_provider,
                catalog_url,
                client_version,
            )
            .map_err(map_api_error)?,
            None => ModelsClient::<ReqwestTransport>::request_url(&api_provider, client_version),
        };
        let auth_telemetry = auth_header_telemetry(api_auth.as_ref())
            .map_err(|error| map_api_error(error.into()))?;
        let agent_identity_telemetry = if let Some(CodexAuth::AgentIdentity(auth)) = auth.as_ref() {
            Some(agent_identity_telemetry(auth))
        } else {
            None
        };
        let request_telemetry: Arc<dyn RequestTelemetry> = Arc::new(ModelsRequestTelemetry {
            include_response_debug: self.provider_info.model_catalog_url.is_none(),
            auth_mode: auth_mode.map(|mode| TelemetryAuthMode::from(mode).to_string()),
            auth_header_attached: auth_telemetry.attached,
            auth_header_name: auth_telemetry.name,
            agent_identity_telemetry,
            auth_env: self.auth_env(),
        });
        let (models, etag) = {
            let transport = self
                .transport_builder
                .build(http_client_factory, request_url.clone())
                .await?;
            let client = ModelsClient::new(transport, api_provider, api_auth)
                .with_telemetry(Some(request_telemetry));
            client
                .list_models(
                    request_url,
                    HeaderMap::new(),
                    /*response_body_limit_bytes*/ None,
                )
                .await
                .map_err(|mut error| {
                    if self.provider_info.model_catalog_url.is_some()
                        && let codex_api::ApiError::Transport(TransportError::Http {
                            url,
                            headers,
                            body,
                            ..
                        }) = &mut error
                    {
                        // Provider diagnostics may echo URL credentials or other secrets.
                        *url = None;
                        *headers = None;
                        *body = None;
                    }
                    map_api_error(error)
                })
        }?;
        if self.provider_info.model_catalog_url.is_some() {
            let mut slugs = HashSet::new();
            if models
                .iter()
                .any(|model| model.slug.trim().is_empty() || !slugs.insert(&model.slug))
            {
                return Err(CodexErr::InvalidRequest(
                    "model catalog must contain unique, non-empty model slugs".to_string(),
                ));
            }
        }
        Ok(ModelsEndpointResponse {
            models,
            etag,
            identity,
        })
    }

    fn auth_env(&self) -> AuthEnvTelemetry {
        let codex_api_key_env_enabled = self
            .auth_manager
            .as_ref()
            .is_some_and(|auth_manager| auth_manager.codex_api_key_env_enabled());
        collect_auth_env_telemetry(&self.provider_info, codex_api_key_env_enabled)
    }
}

impl ModelsEndpointClient for OpenAiModelsEndpoint {
    fn for_originator(
        &self,
        originator: ModelCatalogOriginator,
    ) -> Option<Arc<dyn ModelsEndpointClient>> {
        let mut provider_info = self.provider_info.clone();
        let headers = provider_info.http_headers.get_or_insert_default();
        headers.retain(|name, _| !name.eq_ignore_ascii_case("originator"));
        headers.insert("originator".to_string(), originator.as_str().into());
        if let Some(headers) = &mut provider_info.env_http_headers {
            headers.retain(|name, _| !name.eq_ignore_ascii_case("originator"));
        }
        Some(Arc::new(Self {
            provider_info,
            auth_manager: self.auth_manager.clone(),
            gateway_auth_manager: self.gateway_auth_manager.clone(),
            runtime_auth: self.runtime_auth.clone(),
            transport_builder: Arc::clone(&self.transport_builder),
        }))
    }

    fn supports_api_key_models(&self) -> bool {
        self.provider_info.model_catalog_url.is_some()
            || (self.provider_info.is_openai() && self.provider_info.base_url.is_none())
    }

    fn has_provider_api_key(&self) -> bool {
        self.provider_info.env_key.is_some()
            || self.provider_info.experimental_bearer_token.is_some()
    }

    fn identity(&self) -> Option<String> {
        let auth = self
            .auth_manager
            .as_ref()
            .and_then(|manager| manager.auth_cached());
        crate::models_identity::identity(&self.provider_info, auth.as_ref()).ok()
    }

    fn has_command_auth(&self) -> bool {
        self.provider_info.has_command_auth()
    }

    fn uses_codex_backend(&self) -> ModelsEndpointFuture<'_, bool> {
        Box::pin(OpenAiModelsEndpoint::uses_codex_backend(self))
    }

    fn list_models<'a>(
        &'a self,
        client_version: &'a str,
        http_client_factory: HttpClientFactory,
    ) -> ModelsEndpointFuture<'a, CoreResult<ModelsEndpointResponse>> {
        Box::pin(OpenAiModelsEndpoint::list_models(
            self,
            client_version,
            http_client_factory,
        ))
    }
}

type ModelsTransportFuture<'a> =
    Pin<Box<dyn Future<Output = std::io::Result<ReqwestTransport>> + Send + 'a>>;

/// Builds the concrete transport selected for one models request.
///
/// Implementations must honor the supplied request-time client factory and exact request URL.
trait ModelsTransportBuilder: fmt::Debug + Send + Sync {
    fn build(
        &self,
        http_client_factory: HttpClientFactory,
        request_url: String,
    ) -> ModelsTransportFuture<'_>;
}

#[derive(Debug)]
struct RouteAwareModelsTransportBuilder {
    redirect_policy: ClientRedirectPolicy,
}

impl ModelsTransportBuilder for RouteAwareModelsTransportBuilder {
    fn build(
        &self,
        http_client_factory: HttpClientFactory,
        request_url: String,
    ) -> ModelsTransportFuture<'_> {
        let redirect_policy = self.redirect_policy;
        Box::pin(async move {
            let client = create_client_for_route_async(
                http_client_factory,
                request_url,
                ClientRouteClass::Api,
                redirect_policy,
            )
            .await?;
            let client = match redirect_policy {
                ClientRedirectPolicy::Default => client,
                ClientRedirectPolicy::Reject => client.without_request_logging(),
            };
            Ok(ReqwestTransport::from_http_client(client))
        })
    }
}

#[derive(Clone)]
struct ModelsRequestTelemetry {
    include_response_debug: bool,
    auth_mode: Option<String>,
    auth_header_attached: bool,
    auth_header_name: Option<&'static str>,
    agent_identity_telemetry: Option<AgentIdentityTelemetry>,
    auth_env: AuthEnvTelemetry,
}

impl RequestTelemetry for ModelsRequestTelemetry {
    fn on_request(
        &self,
        attempt: u64,
        status: Option<http::StatusCode>,
        error: Option<&TransportError>,
        duration: Duration,
    ) {
        let success = status.is_some_and(|code| code.is_success()) && error.is_none();
        let error_message = error.map(telemetry_transport_error_message);
        let response_debug = error
            .filter(|_| self.include_response_debug)
            .map(extract_response_debug_context)
            .unwrap_or_default();
        let status = status.map(|status| status.as_u16());
        tracing::event!(
            target: "codex_otel.log_only",
            tracing::Level::INFO,
            event.name = "codex.api_request",
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            success = success,
            error.message = error_message.as_deref(),
            attempt = attempt,
            endpoint = MODELS_ENDPOINT,
            auth.header_attached = self.auth_header_attached,
            auth.header_name = self.auth_header_name,
            auth.env_openai_api_key_present = self.auth_env.openai_api_key_env_present,
            auth.env_codex_api_key_present = self.auth_env.codex_api_key_env_present,
            auth.env_codex_api_key_enabled = self.auth_env.codex_api_key_env_enabled,
            auth.env_provider_key_name = self.auth_env.provider_env_key_name.as_deref(),
            auth.env_provider_key_present = self.auth_env.provider_env_key_present,
            auth.env_refresh_token_url_override_present = self.auth_env.refresh_token_url_override_present,
            auth.request_id = response_debug.request_id.as_deref(),
            auth.cf_ray = response_debug.cf_ray.as_deref(),
            auth.error = response_debug.auth_error.as_deref(),
            auth.error_code = response_debug.auth_error_code.as_deref(),
            auth.mode = self.auth_mode.as_deref(),
            auth.agent_id = self.agent_identity_telemetry.as_ref().map(|metadata| metadata.agent_id.as_str()),
            auth.task_id = self.agent_identity_telemetry.as_ref().map(|metadata| metadata.task_id.as_str()),
        );
        tracing::event!(
            target: "codex_otel.trace_safe",
            tracing::Level::INFO,
            event.name = "codex.api_request",
            duration_ms = %duration.as_millis(),
            http.response.status_code = status,
            success = success,
            error.message = error_message.as_deref(),
            attempt = attempt,
            endpoint = MODELS_ENDPOINT,
            auth.header_attached = self.auth_header_attached,
            auth.header_name = self.auth_header_name,
            auth.env_openai_api_key_present = self.auth_env.openai_api_key_env_present,
            auth.env_codex_api_key_present = self.auth_env.codex_api_key_env_present,
            auth.env_codex_api_key_enabled = self.auth_env.codex_api_key_env_enabled,
            auth.env_provider_key_name = self.auth_env.provider_env_key_name.as_deref(),
            auth.env_provider_key_present = self.auth_env.provider_env_key_present,
            auth.env_refresh_token_url_override_present = self.auth_env.refresh_token_url_override_present,
            auth.request_id = response_debug.request_id.as_deref(),
            auth.cf_ray = response_debug.cf_ray.as_deref(),
            auth.error = response_debug.auth_error.as_deref(),
            auth.error_code = response_debug.auth_error_code.as_deref(),
            auth.mode = self.auth_mode.as_deref(),
            auth.agent_id = self.agent_identity_telemetry.as_ref().map(|metadata| metadata.agent_id.as_str()),
            auth.task_id = self.agent_identity_telemetry.as_ref().map(|metadata| metadata.task_id.as_str()),
        );
        emit_feedback_request_tags_with_auth_env(
            &FeedbackRequestTags {
                endpoint: MODELS_ENDPOINT,
                auth_header_attached: self.auth_header_attached,
                auth_header_name: self.auth_header_name,
                auth_mode: self.auth_mode.as_deref(),
                auth_retry_after_unauthorized: None,
                auth_recovery_mode: None,
                auth_recovery_phase: None,
                auth_connection_reused: None,
                auth_request_id: response_debug.request_id.as_deref(),
                auth_cf_ray: response_debug.cf_ray.as_deref(),
                auth_error: response_debug.auth_error.as_deref(),
                auth_error_code: response_debug.auth_error_code.as_deref(),
                auth_recovery_followup_success: None,
                auth_recovery_followup_status: None,
            },
            &self.auth_env,
        );
    }
}

#[cfg(test)]
#[path = "models_endpoint_timeout_tests.rs"]
mod timeout_tests;

#[cfg(test)]
#[path = "models_originator_tests.rs"]
mod originator_tests;

#[cfg(test)]
#[path = "models_endpoint_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "models_endpoint_runtime_auth_tests.rs"]
mod runtime_auth_tests;

#[cfg(test)]
#[path = "models_endpoint_refresh_tests.rs"]
mod refresh_tests;
