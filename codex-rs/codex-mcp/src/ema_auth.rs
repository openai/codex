//! Dedicated enterprise grants and one thread's EMA authorization lifetime.
//! Account credentials identify the owner; only the separate keyring grant is exchanged.

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_config::CloudConfigBundleBindingStatus;
use codex_config::McpEmaAuthScope;
use codex_config::McpEmaRegistration;
use codex_config::McpServerIdpOAuthConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_rmcp_client::EmaAccessToken;
use codex_rmcp_client::EmaAuthFailure;
use codex_rmcp_client::EmaAuthTokenExchangeRequest;
use codex_rmcp_client::EmaAuthenticatedHttpClient;
use codex_rmcp_client::EmaCredentialSnapshot;
use codex_rmcp_client::EmaIdpIdentityRequest;
use codex_rmcp_client::EmaInvalidGrantSource;
use codex_rmcp_client::EmaRequestAuthority;
use codex_rmcp_client::EmaTokenSource;
use codex_rmcp_client::McpOAuthRefreshMode;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StoredOAuthCredentialSnapshot;
use codex_rmcp_client::StreamableHttpBearerToken;
use codex_rmcp_client::StreamableHttpRedirectMode;
use codex_rmcp_client::exchange_ema_auth_token;
use codex_rmcp_client::resolve_ema_idp_identity;
use codex_rmcp_client::stored_ema_identity_is_usable;
use codex_rmcp_client::stored_oauth_credential_snapshot;
use futures::future::BoxFuture;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;
use url::Url;

use crate::EffectiveMcpServer;
use crate::McpConfig;
use crate::McpServerSource;
use crate::rmcp_client::StartupOutcomeError;

pub(crate) type EmaAuthContext = Result<Arc<EmaAuth>, StartupOutcomeError>;

pub fn ema_auth_scope(auth: Option<&CodexAuth>) -> Option<McpEmaAuthScope> {
    let auth = auth?;
    McpEmaAuthScope::new(auth.get_chatgpt_user_id()?, auth.get_account_id()?)
}

/// Retained by the thread across MCP refreshes; a revoked owner cannot be replaced.
pub(crate) struct EmaAuth {
    scope: McpEmaAuthScope,
    config: Arc<McpConfig>,
    authority: EmaRequestAuthority,
    _revocation: AbortOnDropHandle<()>,
}

impl EmaAuth {
    pub(crate) fn new(
        config: Arc<McpConfig>,
        auth: Option<&CodexAuth>,
        auth_manager: Option<&Arc<AuthManager>>,
    ) -> Result<Arc<Self>> {
        let scope = ema_auth_scope(auth).ok_or(EmaAuthFailure::ReauthenticationRequired)?;
        let manager = auth_manager.context("enterprise MCP requires an account owner")?;
        let mut changes = manager.auth_change_state_receiver();
        let login_generation = changes.borrow().login_generation;
        // Never hold the watch guard while reading cached auth: writers take the auth lock first.
        if ema_auth_scope(manager.auth_cached().as_ref()).as_ref() != Some(&scope)
            || changes.borrow().login_generation != login_generation
        {
            bail!(EmaAuthFailure::ReauthenticationRequired);
        }
        let cancellation = config.mcp_ema_cancellation.clone();
        let policy = config.config_layer_stack.cloud_config_binding().cloned();
        let authority = EmaRequestAuthority::new(cancellation.clone(), {
            let changes = changes.clone();
            let policy = policy.clone();
            move || {
                changes.borrow().login_generation == login_generation
                    && policy.as_ref().is_none_or(|binding| {
                        binding.read().status == CloudConfigBundleBindingStatus::Current
                    })
            }
        });
        authority.check()?;
        let revocation = tokio::spawn(async move {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => {}
                () = async {
                    match policy {
                        Some(binding) => binding.invalidated().await,
                        None => std::future::pending().await,
                    }
                } => {}
                () = async {
                    while changes.changed().await.is_ok() {
                        if changes.borrow().login_generation != login_generation {
                            break;
                        }
                    }
                } => {}
            }
            cancellation.cancel();
        });
        Ok(Arc::new(Self {
            scope,
            config,
            authority,
            _revocation: AbortOnDropHandle::new(revocation),
        }))
    }

    pub(crate) async fn connect(
        self: &Arc<Self>,
        server_name: &str,
        server: &EffectiveMcpServer,
        http_client: Arc<dyn HttpClient>,
        protocol_mode: McpProtocolMode,
    ) -> Result<RmcpClient> {
        self.authority.check()?;
        let config = server.config();
        config
            .validate_ema_auth_transport()
            .map_err(anyhow::Error::msg)?;
        if !self.config.xaa_enabled
            || !self
                .config
                .mcp_server_catalog
                .server(server_name)
                .is_some_and(|server| matches!(server.source(), McpServerSource::Config))
        {
            bail!("enterprise MCP requires an enabled, directly configured server");
        }
        let registration = config
            .ema_registration()
            .context("enterprise MCP registration is missing")?
            .clone();
        let credentials = stored_ema_credentials(
            registration.idp(),
            &self.scope,
            self.config.auth_keyring_backend_kind,
        )
        .await?
        .ok_or(EmaAuthFailure::ReauthenticationRequired)?
        .pin_ema_credentials()
        .await?;
        let exchange_client: Arc<dyn HttpClient> = Arc::new(EnterpriseExchangeHttpClient {
            inner: Arc::clone(&http_client),
            authority: self.authority.clone(),
            cancellation: self.config.mcp_ema_cancellation.clone(),
        });
        let url = registration.server_url().to_owned();
        let source = EnterpriseTokenSource {
            endpoint: Url::parse(registration.server_url())?,
            registration,
            credentials,
            auth: Arc::clone(self),
            http_client: exchange_client,
        };
        let token = source.access_token().await?;
        let http_client = Arc::new(EmaAuthenticatedHttpClient::new(
            http_client,
            source,
            token,
            self.authority.clone(),
        ));
        RmcpClient::new_streamable_http_client_with_protocol_mode_and_redirect_mode(
            server_name,
            &url,
            Some(StreamableHttpBearerToken::ProvidedByHttpClient),
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            /*oauth_config*/ None,
            OAuthCredentialsStoreMode::Keyring,
            self.config.auth_keyring_backend_kind,
            http_client,
            /*auth_provider*/ None,
            protocol_mode,
            StreamableHttpRedirectMode::Legacy,
            McpOAuthRefreshMode::Legacy,
        )
        .await
    }
}

pub(crate) async fn stored_ema_credentials(
    idp: &McpServerIdpOAuthConfig,
    scope: &McpEmaAuthScope,
    keyring_backend: AuthKeyringBackendKind,
) -> Result<Option<StoredOAuthCredentialSnapshot>> {
    let name = idp.credential_name(scope);
    let idp = idp.clone();
    tokio::task::spawn_blocking(move || {
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            stored_oauth_credential_snapshot(
                &name,
                &idp.issuer,
                OAuthCredentialsStoreMode::Keyring,
                keyring_backend,
            )
            .map(|snapshot| {
                snapshot.filter(|snapshot| {
                    stored_ema_identity_is_usable(
                        snapshot.credentials(),
                        &idp.issuer,
                        &idp.client_id,
                    )
                })
            })
            .map_err(|_| anyhow!("failed to read enterprise credentials"))
        })
    })
    .await
    .map_err(|_| anyhow!("enterprise credential read task failed"))?
}

// Discovery and both credential exchanges must check the owner at each dispatch,
// including when the revocation watcher has not yet been scheduled.
struct EnterpriseExchangeHttpClient {
    inner: Arc<dyn HttpClient>,
    authority: EmaRequestAuthority,
    cancellation: CancellationToken,
}

impl HttpClient for EnterpriseExchangeHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        Box::pin(async move {
            self.authority.check()?;
            tokio::select! {
                biased;
                () = self.cancellation.cancelled() => Err(ExecServerError::HttpRequest("enterprise MCP authorization was revoked".into())),
                response = self.inner.http_request(params) => {
                    self.authority.check()?;
                    response
                }
            }
        })
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        Box::pin(async move {
            self.authority.check()?;
            tokio::select! {
                biased;
                () = self.cancellation.cancelled() => Err(ExecServerError::HttpRequest("enterprise MCP authorization was revoked".into())),
                response = self.inner.http_request_stream(params) => {
                    self.authority.check()?;
                    response.map(|(response, body)| (response, body.cancel_on(self.cancellation.clone())))
                }
            }
        })
    }
}

struct EnterpriseTokenSource {
    endpoint: Url,
    registration: McpEmaRegistration,
    credentials: EmaCredentialSnapshot,
    auth: Arc<EmaAuth>,
    http_client: Arc<dyn HttpClient>,
}

impl EmaTokenSource for EnterpriseTokenSource {
    fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    async fn access_token(&self) -> Result<EmaAccessToken> {
        self.auth.authority.check()?;
        let idp = self.registration.idp();
        let exchange = exchange_ema_auth_token(EmaAuthTokenExchangeRequest {
            server_url: self.registration.server_url(),
            resource: self.registration.resource(),
            scopes: self.registration.scopes(),
            mcp_client_id: self.registration.client_id(),
            expected_authorization_server_issuer: self.registration.authorization_server_issuer(),
            idp_issuer: &idp.issuer,
            idp_client_id: &idp.client_id,
            idp_identity: Box::pin(async {
                self.auth.authority.check()?;
                resolve_ema_idp_identity(EmaIdpIdentityRequest {
                    issuer: &idp.issuer,
                    client_id: &idp.client_id,
                    credentials: self.credentials.acquire_current_ema_credentials().await?,
                    http_client: Arc::clone(&self.http_client),
                    redirect_mode: StreamableHttpRedirectMode::Legacy,
                })
                .await
            }),
            idp_http_client: Arc::clone(&self.http_client),
            resource_http_client: Arc::clone(&self.http_client),
        });
        let result = tokio::select! {
            biased;
            () = self.auth.config.mcp_ema_cancellation.cancelled() => {
                bail!(EmaAuthFailure::ReauthenticationRequired);
            }
            result = exchange => result,
        };
        self.auth.authority.check()?;
        match result {
            Ok((token, _credentials)) => {
                self.credentials.acquire_current_ema_credentials().await?;
                self.auth.authority.check()?;
                Ok(token)
            }
            Err(error) => {
                if matches!(
                    error.downcast_ref::<EmaAuthFailure>(),
                    Some(EmaAuthFailure::InvalidGrant {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    })
                ) && self
                    .credentials
                    .invalidate_ema_credentials_if_current()
                    .await
                    .is_err()
                {
                    tracing::warn!("failed to clear rejected enterprise credentials");
                }
                Err(match error.downcast::<EmaAuthFailure>() {
                    Ok(failure) => failure.into(),
                    Err(_) => anyhow!("enterprise MCP token exchange failed"),
                })
            }
        }
    }
}
