//! Pins one EMA connection to an account login or a dedicated enterprise grant.
//! Account refresh exclusion ends with the IdP exchange, before resource-server I/O.

use std::sync::Arc;

use anyhow::Result;
use anyhow::bail;
use codex_config::McpEmaRegistration;
use codex_exec_server::HttpClient;
use codex_login::AccountOAuthGrant;
use codex_login::AccountOAuthGrantLease;
use codex_rmcp_client::EmaAccessToken;
use codex_rmcp_client::EmaAuthFailure;
use codex_rmcp_client::EmaAuthTokenExchangeRequest;
use codex_rmcp_client::EmaCredentialKind;
use codex_rmcp_client::EmaCredentialLease;
use codex_rmcp_client::EmaCredentialSnapshot;
use codex_rmcp_client::EmaIdpIdentityRequest;
use codex_rmcp_client::EmaInvalidGrantSource;
use codex_rmcp_client::EmaTokenSource;
use codex_rmcp_client::EmaTokenSourceError;
use codex_rmcp_client::StreamableHttpRedirectMode;
use codex_rmcp_client::exchange_ema_auth_token;
use codex_rmcp_client::resolve_ema_account_idp_identity;
use codex_rmcp_client::resolve_ema_idp_identity;
use url::Url;

use crate::ema_account::account_configuration_issue;
use crate::ema_account::account_grant_is_usable;
use crate::ema_auth::EmaAuth;
use crate::ema_auth::stored_ema_credentials;

enum CredentialSource {
    Account { client_id: String },
    Keyring(Box<EmaCredentialSnapshot>),
}

enum IdpLease {
    Account(AccountOAuthGrantLease),
    Keyring(EmaCredentialLease),
}

enum ExchangeReceipt {
    Account(AccountOAuthGrant),
    Keyring(EmaCredentialLease),
}

pub(crate) struct EnterpriseTokenSource {
    endpoint: Url,
    registration: McpEmaRegistration,
    credentials: CredentialSource,
    auth: Arc<EmaAuth>,
    http_client: Arc<dyn HttpClient>,
}

impl EnterpriseTokenSource {
    pub(crate) async fn new(
        registration: McpEmaRegistration,
        auth: Arc<EmaAuth>,
        http_client: Arc<dyn HttpClient>,
    ) -> Result<Self> {
        auth.check_account().await?;
        let credentials = if let Some(snapshot) = stored_ema_credentials(
            registration.idp(),
            &auth.scope,
            auth.config.auth_keyring_backend_kind,
        )
        .await?
        {
            CredentialSource::Keyring(Box::new(snapshot.pin_ema_credentials().await?))
        } else {
            let account = auth.manager.lease_account_oauth_grant().await?;
            let client_id = account
                .as_ref()
                .filter(|lease| account_grant_is_usable(lease.tokens(), registration.idp()))
                .and_then(|lease| lease.tokens().oauth_provenance.as_ref())
                .map(|provenance| provenance.client_id.clone())
                .ok_or_else(|| {
                    anyhow::Error::new(EmaAuthFailure::ReauthenticationRequired).context(
                        account_configuration_issue(&auth.account, registration.idp())
                            .unwrap_or("Complete enterprise sign-in to use this MCP server"),
                    )
                })?;
            CredentialSource::Account { client_id }
        };
        Ok(Self {
            endpoint: Url::parse(registration.server_url())?,
            registration,
            credentials,
            auth,
            http_client,
        })
    }

    async fn account_grant(&self, client_id: &str) -> Result<AccountOAuthGrant> {
        self.auth.check_account().await?;
        let lease = self
            .auth
            .manager
            .lease_account_oauth_grant()
            .await?
            .ok_or(EmaAuthFailure::ReauthenticationRequired)?;
        if !account_grant_is_usable(lease.tokens(), self.registration.idp())
            || lease
                .tokens()
                .oauth_provenance
                .as_ref()
                .is_none_or(|provenance| provenance.client_id != client_id)
        {
            bail!(EmaAuthFailure::ReauthenticationRequired);
        }
        Ok(lease.release())
    }

    async fn exchange(&self, account: Option<&AccountOAuthGrant>) -> Result<EmaAccessToken> {
        let idp = self.registration.idp();
        let client_id = match &self.credentials {
            CredentialSource::Account { client_id } => client_id,
            CredentialSource::Keyring(_) => &idp.client_id,
        };
        let identity = Box::pin(async {
            self.auth.check_account().await?;
            match &self.credentials {
                CredentialSource::Account { .. } => {
                    let grant = account.ok_or(EmaAuthFailure::ReauthenticationRequired)?;
                    let tokens = grant.tokens();
                    // Metadata discovery must not block ordinary account refresh.
                    let identity = resolve_ema_account_idp_identity(
                        &idp.issuer,
                        client_id,
                        &tokens.id_token.raw_jwt,
                        tokens.refresh_token.clone(),
                        /*credentials*/ (),
                        Arc::clone(&self.http_client),
                        StreamableHttpRedirectMode::Legacy,
                    )
                    .await?;
                    let lease = self
                        .auth
                        .manager
                        .lease_account_oauth_grant()
                        .await?
                        .ok_or(EmaAuthFailure::ReauthenticationRequired)?;
                    if lease.tokens() != tokens {
                        bail!(EmaAuthFailure::ReauthenticationRequired);
                    }
                    Ok(identity.map_credentials(|()| IdpLease::Account(lease)))
                }
                CredentialSource::Keyring(snapshot) => {
                    resolve_ema_idp_identity(EmaIdpIdentityRequest {
                        issuer: &idp.issuer,
                        client_id,
                        credentials: snapshot.acquire_current_ema_credentials().await?,
                        http_client: Arc::clone(&self.http_client),
                        redirect_mode: StreamableHttpRedirectMode::Legacy,
                    })
                    .await
                    .map(|identity| identity.map_credentials(IdpLease::Keyring))
                }
            }
        });
        let (token, receipt) = exchange_ema_auth_token(
            EmaAuthTokenExchangeRequest {
                server_url: self.registration.server_url(),
                resource: self.registration.resource(),
                scopes: self.registration.scopes(),
                mcp_client_id: self.registration.client_id(),
                expected_authorization_server_issuer: self
                    .registration
                    .authorization_server_issuer(),
                idp_issuer: &idp.issuer,
                idp_client_id: client_id,
                idp_identity: identity,
                idp_http_client: Arc::clone(&self.http_client),
                resource_http_client: Arc::clone(&self.http_client),
            },
            |lease| match lease {
                IdpLease::Account(lease) => ExchangeReceipt::Account(lease.release()),
                IdpLease::Keyring(lease) => ExchangeReceipt::Keyring(lease),
            },
        )
        .await?;
        match receipt {
            ExchangeReceipt::Account(grant) => {
                if !grant.check_current().await? {
                    // A successful exchange with a stale receipt must fail this call.
                    bail!(EmaTokenSourceError::new(
                        EmaCredentialKind::Account,
                        EmaAuthFailure::ReauthenticationRequired
                    ));
                }
            }
            ExchangeReceipt::Keyring(lease) => {
                drop(lease);
                if let CredentialSource::Keyring(snapshot) = &self.credentials {
                    snapshot.acquire_current_ema_credentials().await?;
                }
            }
        }
        self.auth.check_account().await?;
        Ok(token)
    }

    async fn acquire_token(&self) -> Result<EmaAccessToken> {
        self.auth.check_account().await?;
        match &self.credentials {
            CredentialSource::Account { client_id } => {
                let grant = self.account_grant(client_id).await?;
                self.exchange(Some(&grant)).await
            }
            CredentialSource::Keyring(snapshot) => {
                let result = self.exchange(/*account*/ None).await;
                if result.as_ref().is_err_and(|error| {
                    matches!(
                        error.downcast_ref::<EmaAuthFailure>(),
                        Some(EmaAuthFailure::InvalidGrant {
                            grant_source: EmaInvalidGrantSource::EnterpriseIdentity
                        })
                    )
                }) && snapshot
                    .invalidate_ema_credentials_if_current(async {
                        self.auth.check_account().await.is_ok()
                    })
                    .await
                    .is_err()
                {
                    tracing::warn!("failed to clear rejected enterprise credentials");
                }
                result
            }
        }
    }
}

impl EmaTokenSource for EnterpriseTokenSource {
    fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    async fn access_token(&self) -> Result<EmaAccessToken> {
        let kind = match self.credentials {
            CredentialSource::Account { .. } => EmaCredentialKind::Account,
            CredentialSource::Keyring(_) => EmaCredentialKind::Keyring,
        };
        let result = tokio::select! {
            biased;
            () = self.auth.config.mcp_ema_cancellation.cancelled() => Err(EmaAuthFailure::ReauthenticationRequired.into()),
            result = self.acquire_token() => result,
        };
        result.map_err(|error| {
            if let Some(error) = error.downcast_ref::<EmaTokenSourceError>() {
                (*error).into()
            } else {
                match error.downcast_ref::<EmaAuthFailure>() {
                    Some(failure) => EmaTokenSourceError::new(kind, *failure),
                    None if codex_rmcp_client::is_authentication_required_error(&error) => {
                        EmaTokenSourceError::new(kind, EmaAuthFailure::ReauthenticationRequired)
                    }
                    None => EmaTokenSourceError::unexpected(kind),
                }
                .into()
            }
        })
    }
}
