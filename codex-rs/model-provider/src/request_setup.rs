//! Resolve credentials, routing, and transport policy as one request context.

use crate::ModelProvider;
use crate::ProviderAuthScope;
use crate::ResolvedProviderAuth;
use crate::WorkspaceRoutingContext;
use crate::auth::bearer_auth_for_provider;
use crate::auth::resolve_provider_auth;
use crate::auth::resolve_provider_auth_for_scope;
use codex_api::AgentIdentityTelemetry;
use codex_api::Provider;
use codex_api::ReqwestTransport;
use codex_api::SharedAuthProvider;
use codex_features::RuntimeAuthMode;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_login::AuthOwner;
use codex_login::CodexAuth;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::create_client_for_route;
use codex_model_provider_info::GatewayOAuthConfig;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result;
use codex_runtime_agent_instance::RuntimeAuth;
use codex_runtime_agent_instance::RuntimeAuthManager;
use std::sync::Arc;

/// Select auth for inference and discovery, letting rotating credentials finish saving
/// if the requesting operation is cancelled.
pub(crate) async fn resolve_request_auth(
    info: ModelProviderInfo,
    manager: Option<Arc<AuthManager>>,
    runtime: Option<(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
    scope: Option<ProviderAuthScope>,
    destination: Option<String>,
) -> codex_protocol::error::Result<ResolvedProviderAuth> {
    tokio::spawn(async move {
        crate::auth::observe_auth_selection(
            runtime.as_ref().map(|(_, mode)| *mode),
            "model",
            async move {
                if let Some((runtime, mode)) = runtime {
                    crate::validate_runtime_destination(destination.as_deref())?;
                    crate::validate_runtime_environment(&runtime, destination.as_deref())?;
                    let runtime =
                        crate::select_runtime_provider(&info, manager.as_ref(), &runtime)?;
                    let headers = info.build_header_map()?;
                    let selected = crate::runtime_auth::select_runtime_auth(&runtime, &headers)
                        .await
                        .map_err(|error| codex_api::map_api_error(error.into()))?;
                    let resolved = ResolvedProviderAuth::for_runtime(selected, mode);
                    resolved.validate_headers(&headers)?;
                    tracing::debug!(runtime_auth_mode = ?mode, "selected runtime authentication");
                    return Ok(resolved);
                }

                if let Some(bearer) = bearer_auth_for_provider(&info)? {
                    let credential = bearer.token.as_deref().map(CodexAuth::from_api_key);
                    let mut resolved = ResolvedProviderAuth::new(Arc::new(bearer));
                    resolved.credential = credential;
                    return Ok(resolved);
                }

                let credential = match &manager {
                    Some(manager) => manager.auth().await,
                    None => None,
                };
                let mut resolved = match scope {
                    Some(scope) if crate::provider_uses_first_party_auth_path(&info) => {
                        resolve_provider_auth_for_scope(manager, credential.as_ref(), &info, scope)
                            .await?
                    }
                    _ => ResolvedProviderAuth::new(resolve_provider_auth(
                        credential.as_ref(),
                        &info,
                    )?),
                };
                resolved.credential = credential;
                Ok(resolved)
            },
        )
        .await
    })
    .await?
}

/// Capture the selected source's owner before starting work such as a turn or classification.
/// Request preparation returns the owner it actually used so callers can detect a source change.
pub fn request_auth_owner(provider: &dyn ModelProvider) -> Option<AuthOwner> {
    let credentials = provider.auth_manager();
    match provider.runtime_auth() {
        Some((runtime, _)) => {
            crate::select_runtime_provider(provider.info(), credentials.as_ref(), runtime)
                .ok()
                .map(|binding| binding.auth_owner())
        }
        None => credentials.map(|manager| manager.auth_owner()),
    }
}

pub struct ResolvedProviderRequest {
    pub auth: Option<CodexAuth>,
    pub auth_owner: Option<AuthOwner>,
    pub auth_revision: Option<u64>,
    pub api_provider: Provider,
    pub redirect_policy: ClientRedirectPolicy,
    pub api_auth: SharedAuthProvider,
    pub agent_identity_telemetry: Option<AgentIdentityTelemetry>,
    pub runtime_id: Option<String>,
    pub http_client_factory: HttpClientFactory,
}

impl ResolvedProviderRequest {
    pub fn http_transport(&self, endpoint: &str) -> Result<ReqwestTransport> {
        let redirect_policy = if self
            .api_provider
            .headers
            .contains_key(crate::ACCOUNT_ROUTING_HEADER)
        {
            ClientRedirectPolicy::Reject
        } else {
            self.redirect_policy
        };
        let request_url = self.api_provider.url_for_path(endpoint);
        let client = create_client_for_route(
            &self.http_client_factory,
            &request_url,
            ClientRouteClass::Api,
            redirect_policy,
        )
        .map_err(std::io::Error::from)?;
        Ok(ReqwestTransport::from_http_client(client))
    }
}

/// Resolve one attempt, including any workspace route, without mixing credential revisions.
/// The supplied factory retains the caller's proxy/TLS settings; runtime requests bind its
/// application policy to the same selected account used to sign the request.
pub async fn resolve_request(
    provider: &dyn ModelProvider,
    scope: Option<ProviderAuthScope>,
    routing: Option<&WorkspaceRoutingContext>,
    http_client_factory: HttpClientFactory,
) -> Result<ResolvedProviderRequest> {
    let check_auth = provider.runtime_auth().is_none()
        && crate::provider_uses_first_party_auth_path(provider.info());
    crate::prepare_provider_request(provider, check_auth, |state| {
        let scope = scope.clone();
        let http_client_factory = http_client_factory.clone();
        let owner = state
            .and_then(|_| provider.auth_manager())
            .map(|manager| manager.auth_owner());
        async move {
            let (api_provider, resolved_auth) = provider.request_provider_and_auth(scope).await?;
            let auth = match &resolved_auth.credential {
                Some(auth) => Some(auth.clone()),
                None => provider.auth().await,
            };
            let (api_provider, redirect_policy) = match routing {
                Some(routing) => {
                    let resolved = provider
                        .route_responses_api_provider(api_provider, auth.as_ref(), routing)
                        .await?;
                    (resolved.provider, resolved.redirect_policy)
                }
                None => (api_provider, ClientRedirectPolicy::Default),
            };
            resolved_auth.validate_headers(&api_provider.headers)?;
            let runtime_state = resolved_auth
                .runtime
                .as_ref()
                .map(RuntimeAuth::change_state);
            let http_client_factory = resolved_auth
                .bind_http_client_factory(http_client_factory)
                .map_err(|error| provider.map_api_error(error.into()))?;
            let runtime_id = resolved_auth
                .runtime
                .as_ref()
                .map(|selected| selected.runtime().id().to_owned());
            Ok(ResolvedProviderRequest {
                auth,
                auth_owner: resolved_auth
                    .runtime
                    .as_ref()
                    .map(|selected| selected.auth_owner().clone())
                    .or(owner),
                auth_revision: runtime_state
                    .map(|state| state.generation)
                    .or(state.map(|state| state.generation)),
                api_provider,
                redirect_policy: if provider
                    .info()
                    .gateway_oauth
                    .as_ref()
                    .is_some_and(GatewayOAuthConfig::uses_credential_export)
                {
                    ClientRedirectPolicy::Reject
                } else {
                    redirect_policy
                },
                api_auth: resolved_auth.auth,
                agent_identity_telemetry: resolved_auth.agent_identity_telemetry,
                runtime_id,
                http_client_factory,
            })
        }
    })
    .await
}
