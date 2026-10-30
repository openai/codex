//! Request-time authentication for remote executors, including AWS signing.
//!
//! ChatGPT credentials may rotate while an executor is idle. Reload them for
//! registry requests while retaining the identity that registered the executor.
//! Failed credential reads block headers until revalidation, without counting as logout.
//! Quarantine begins with the OAuth exchange, after credential reads and refresh-guard waits.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_api::AuthError;
use codex_api::AuthHeadersFuture;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_aws_auth::AwsAuthConfig;
use codex_aws_auth::AwsAuthContext;
use codex_aws_auth::AwsAuthError;
use codex_aws_auth::AwsRequestToSign;
use codex_http_client::Request;
use codex_http_client::RequestBody;
use codex_http_client::RequestCompression;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use http::HeaderMap;
use tokio::sync::Semaphore;

/// Follows stored credential rotation without allowing an executor's owner to change.
pub(super) fn chatgpt_auth_provider(
    auth_manager: Arc<AuthManager>,
    expected_auth: &CodexAuth,
) -> Result<SharedAuthProvider, AuthError> {
    auth_manager
        .restrict_auth_modes(vec![expected_auth.auth_mode()])
        .map_err(|_| AuthError::Build("executor credential policy is unavailable".to_string()))?;
    let account_id = expected_auth.get_account_id();
    let user_id = expected_auth.get_chatgpt_user_id();
    let identity_complete = account_id.as_ref().is_some_and(|id| !id.trim().is_empty())
        && user_id.as_ref().is_some_and(|id| !id.trim().is_empty());
    let unidentified_headers = if identity_complete {
        None
    } else {
        Some(codex_model_provider::auth_provider_from_auth(expected_auth).to_auth_headers()?)
    };
    Ok(Arc::new(ReloadingChatgptAuthProvider {
        managed: codex_model_provider::auth_provider_from_auth_manager(
            Arc::clone(&auth_manager),
            expected_auth,
        ),
        auth_manager,
        account_id,
        user_id,
        workspace_account: expected_auth.is_workspace_account(),
        fedramp_account: expected_auth.is_fedramp_account(),
        unidentified_headers,
        resolution_lock: Arc::new(Semaphore::new(/*permits*/ 1)),
        headers_valid: Arc::new(AtomicBool::new(true)),
    }))
}

#[derive(Clone)]
struct ReloadingChatgptAuthProvider {
    managed: SharedAuthProvider,
    auth_manager: Arc<AuthManager>,
    account_id: Option<String>,
    user_id: Option<String>,
    workspace_account: bool,
    fedramp_account: bool,
    unidentified_headers: Option<HeaderMap>,
    resolution_lock: Arc<Semaphore>,
    headers_valid: Arc<AtomicBool>,
}

impl ReloadingChatgptAuthProvider {
    fn checked_headers(&self, auth: Option<CodexAuth>) -> Result<HeaderMap, AuthError> {
        let auth = auth
            .filter(|auth| {
                auth.is_chatgpt_auth()
                    && auth.get_account_id() == self.account_id
                    && auth.get_chatgpt_user_id() == self.user_id
                    && auth.is_workspace_account() == self.workspace_account
                    && auth.is_fedramp_account() == self.fedramp_account
            })
            .ok_or_else(|| AuthError::Build("executor authentication changed".to_string()))?;
        let token = auth.get_token().map_err(|_| {
            AuthError::Build("executor credential has no usable bearer".to_string())
        })?;
        if token.trim().is_empty() {
            return Err(AuthError::Build(
                "executor credential has an empty bearer".to_string(),
            ));
        }
        let uncertain = self
            .auth_manager
            .is_chatgpt_refresh_unsettled(&auth)
            .map_err(|_| {
                AuthError::Transient("executor authentication is unavailable".to_string())
            })?;
        if uncertain {
            return Err(AuthError::Transient(
                "executor refresh outcome is unknown; replace or renew stored credentials"
                    .to_string(),
            ));
        }
        let headers = codex_model_provider::auth_provider_from_auth(&auth).to_auth_headers()?;
        if self
            .unidentified_headers
            .as_ref()
            .is_some_and(|expected| expected != &headers)
        {
            return Err(AuthError::Build(
                "executor credential rotation requires a complete account and user identity"
                    .to_string(),
            ));
        }
        Ok(headers)
    }
}

impl AuthProvider for ReloadingChatgptAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError> {
        if !self.headers_valid.load(Ordering::Acquire) {
            return Err(AuthError::Transient(
                "executor credentials have not been successfully revalidated".to_string(),
            ));
        }
        headers.extend(self.checked_headers(self.auth_manager.auth_cached())?);
        Ok(())
    }

    fn apply_auth_headers(&self, mut headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            headers.extend(self.resolve(/*rejected_headers*/ None).await?);
            Ok(headers)
        })
    }

    fn refresh_after_unauthorized(
        &self,
        rejected_headers: HeaderMap,
    ) -> codex_api::AuthRefreshFuture<'_> {
        Box::pin(async move {
            self.resolve(Some(rejected_headers.clone()))
                .await
                .is_ok_and(|headers| {
                    headers
                        .iter()
                        .any(|(name, value)| rejected_headers.get(name) != Some(value))
                })
        })
    }
}

impl ReloadingChatgptAuthProvider {
    async fn resolve(&self, rejected_headers: Option<HeaderMap>) -> Result<HeaderMap, AuthError> {
        let guard = Arc::clone(&self.resolution_lock)
            .acquire_owned()
            .await
            .map_err(|_| {
                AuthError::Transient("executor authentication is unavailable".to_string())
            })?;
        let provider = self.clone();
        // A failed read leaves AuthManager's last known owner intact. Keep those
        // cached credentials unavailable to synchronous callers until revalidated.
        provider.headers_valid.store(false, Ordering::Release);
        // Outlive a cancelled handshake, but never hold the permit indefinitely.
        // A timed-out OAuth exchange may already have rotated the token: quarantine
        // that generation until another login/refresh supplies different credentials.
        tokio::spawn(async move {
            let _guard = guard;
            let result = tokio::time::timeout(Duration::from_secs(30), async {
                provider
                    .auth_manager
                    .reload_with_cached_auth()
                    .await
                    .map_err(|_| {
                        AuthError::Transient(
                            "executor credential storage or hydration is unavailable".to_string(),
                        )
                    })?;
                let current = provider.checked_headers(provider.auth_manager.auth_cached())?;
                if provider.unidentified_headers.is_some() {
                    return Ok(current);
                }
                if let Some(rejected) = rejected_headers {
                    if !current
                        .iter()
                        .any(|(name, value)| rejected.get(name) != Some(value))
                    {
                        provider.managed.refresh_after_unauthorized(rejected).await;
                    }
                } else {
                    provider.auth_manager.auth().await;
                }
                // auth() deliberately falls back to a snapshot on refresh errors.
                // Re-read the selected source before dispatch: logout, replacement,
                // or failed hydration must never authorize that fallback snapshot.
                provider
                    .auth_manager
                    .reload_with_cached_auth()
                    .await
                    .map_err(|_| {
                        AuthError::Transient(
                            "executor credential storage or hydration is unavailable".to_string(),
                        )
                    })?;
                provider.checked_headers(provider.auth_manager.auth_cached())
            })
            .await;
            let result = match result {
                Ok(result) => result,
                Err(_) => Err(AuthError::Transient(
                    "executor credential resolution timed out; retry after repairing credentials"
                        .to_string(),
                )),
            };
            provider
                .headers_valid
                .store(result.is_ok(), Ordering::Release);
            result
        })
        .await
        .map_err(|_| AuthError::Transient("executor authentication task failed".to_string()))?
    }
}

/// Creates a SigV4 provider, preferring an explicit profile over the default credential chain.
pub(super) async fn aws_sigv4_auth_provider(
    mut config: AwsAuthConfig,
    http_client_factory: codex_http_client::HttpClientFactory,
) -> Result<SharedAuthProvider, AwsAuthError> {
    config.profile = config
        .profile
        .map(|profile| profile.trim().to_string())
        .filter(|profile| !profile.is_empty());
    config.region = config
        .region
        .map(|region| region.trim().to_string())
        .filter(|region| !region.is_empty());
    let context = if config.profile.is_some() {
        AwsAuthContext::load_profile(config, http_client_factory).await
    } else {
        AwsAuthContext::load(config, http_client_factory).await
    }?;
    Ok(Arc::new(AwsSigV4AuthProvider { context }))
}

#[derive(Debug)]
struct AwsSigV4AuthProvider {
    context: AwsAuthContext,
}

impl AuthProvider for AwsSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), AuthError> {
        Ok(())
    }

    fn apply_auth(&self, mut request: Request) -> codex_api::AuthProviderFuture<'_> {
        Box::pin(async move {
            let prepared = request.prepare_body_for_send().map_err(AuthError::Build)?;
            let signed = self
                .context
                .sign(AwsRequestToSign {
                    method: request.method.clone(),
                    url: request.url.clone(),
                    headers: prepared.headers.clone(),
                    body: prepared.body_bytes(),
                })
                .await
                .map_err(|error| {
                    if error.is_retryable() {
                        AuthError::Transient(error.to_string())
                    } else {
                        AuthError::Build(error.to_string())
                    }
                })?;
            request.url = signed.url;
            request.headers = signed.headers;
            request.body = prepared.body.map(RequestBody::Raw);
            request.compression = RequestCompression::None;
            Ok(request)
        })
    }
}

#[cfg(test)]
#[path = "exec_server_auth_tests.rs"]
mod tests;
