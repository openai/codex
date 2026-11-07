//! Local analytics authentication, server plan discovery, and account/user identity checks.
//! Each new session fetches its plan once; token plan claims do not select reports.

use crate::Client;
use crate::RequestError;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_login::AuthManagerConfig;
use codex_login::CodexAuth;
use codex_protocol::account::PlanType;
use codex_runtime_agent_instance::RegistrationError;
use codex_runtime_agent_instance::RuntimeAuthError;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::resolve_installation_id;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::sync::Arc;

/// Non-secret account metadata associated with an analytics session.
#[derive(Clone, Debug)]
pub struct AnalyticsAccount {
    pub id: String,
    pub email: Option<String>,
    pub plan_type: Option<PlanType>,
}

/// A backend client bound to its initial ChatGPT account and user, with a server plan snapshot.
pub struct AnalyticsSession {
    client: Client,
    auth_manager: Arc<AuthManager>,
    auth: CodexAuth,
    account: AnalyticsAccount,
}

impl AnalyticsSession {
    /// Load local ChatGPT credentials and the current server plan using the configured HTTP policy.
    pub async fn from_config(
        config: &impl AuthManagerConfig,
        http_client_factory: HttpClientFactory,
        runtime_auth_mode: Option<RuntimeAuthMode>,
    ) -> Result<Self, String> {
        let auth_manager =
            AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false)
                .await
                .map_err(|_| {
                    "Couldn't load local sign-in. Sign in with ChatGPT and retry.".to_string()
                })?;
        let auth = auth_manager
            .auth()
            .await
            .filter(CodexAuth::is_chatgpt_auth)
            .ok_or("Sign in locally with ChatGPT to view Analytics.")?;
        let (Some(id), Some(_)) = (auth.get_account_id(), auth.get_chatgpt_user_id()) else {
            return Err("Analytics requires a ChatGPT account and user identity.".into());
        };
        let account = AnalyticsAccount {
            id,
            email: auth.get_account_email(),
            plan_type: None,
        };
        let base_url = config.chatgpt_base_url();
        let runtime_auth_mode = runtime_auth_mode
            .filter(|_| codex_runtime_agent_instance::is_openai_api_url(&base_url));
        let client = match runtime_auth_mode {
            Some(mode) => {
                let home = AbsolutePathBuf::from_absolute_path(config.codex_home())
                    .map_err(|_| "Couldn't initialize local Analytics authentication.")?;
                let installation_id = resolve_installation_id(&home)
                    .await
                    .map_err(|_| "Couldn't initialize local Analytics authentication.")?;
                let runtime = (
                    Arc::new(RuntimeAuthManager::for_codex(
                        Arc::clone(&auth_manager),
                        &base_url,
                        &installation_id,
                    )),
                    mode,
                );
                Client::from_auth_with_runtime_without_redirects(
                    &base_url,
                    &auth,
                    http_client_factory,
                    Some(&runtime),
                    /*headers*/ &Default::default(),
                )
                .await
                .map_err(|error| {
                    match error.downcast_ref::<RuntimeAuthError>() {
                        Some(
                            RuntimeAuthError::RefreshFailed
                            | RuntimeAuthError::RefreshTokenFailed(_)
                            | RuntimeAuthError::Registration(RegistrationError::Rejected {
                                status: http::StatusCode::UNAUTHORIZED,
                                ..
                            }),
                        ) => "Sign in again to load Analytics.",
                        _ => "Couldn't authenticate the Analytics request.",
                    }
                })?
            }
            None => Client::new_without_redirects(base_url, http_client_factory)
                .with_auth_provider(codex_model_provider::auth_provider_from_auth_manager(
                    Arc::clone(&auth_manager),
                    &auth,
                ))
                .map_err(|_| "Couldn't initialize local Analytics authentication.")?,
        };
        let mut session = Self {
            client,
            auth_manager,
            auth,
            account,
        };
        let accounts = session
            .request(|client| async move { client.get_accounts_check().await })
            .await;
        // Preserve account-switch guidance even when it interrupts account discovery.
        session.ensure_identity().await?;
        let accounts = accounts.map_err(|error| {
            if error.is_unauthorized() {
                "Sign in again to load Analytics."
            } else {
                "Couldn't load account plan. Press R to retry Analytics."
            }
        })?;
        session.account.plan_type = Some(
            accounts
                .accounts
                .into_iter()
                .find(|account| account.id == session.account.id)
                .and_then(|account| account.plan_type)
                .ok_or("Couldn't load account plan. Press R to retry Analytics.")?,
        );
        Ok(session)
    }

    /// Return the account metadata captured when this session was opened.
    pub fn account(&self) -> &AnalyticsAccount {
        &self.account
    }
    /// Reject responses or cached data after a local account or user change.
    pub async fn ensure_identity(&self) -> Result<(), String> {
        self.auth_manager.reload().await;
        let current = self.auth_manager.auth().await;
        if current.is_none_or(|auth| {
            auth.get_account_id() != self.auth.get_account_id()
                || auth.get_chatgpt_user_id() != self.auth.get_chatgpt_user_id()
        }) {
            return Err("Account changed. Press R to refresh Analytics.".into());
        }
        Ok(())
    }

    /// Run an account-scoped request with bounded unauthorized recovery.
    pub async fn request<T, F>(&self, request: impl Fn(Client) -> F) -> Result<T, RequestError>
    where
        F: std::future::Future<Output = Result<T, RequestError>>,
    {
        let mut recovery = self.auth_manager.unauthorized_recovery();
        loop {
            self.ensure_identity()
                .await
                .map_err(|error| RequestError::Other(anyhow::anyhow!(error)))?;
            let result = request(self.client.clone()).await;
            // Keep the original 401 when recovery fails so callers retain sign-in guidance.
            if result.as_ref().is_err_and(RequestError::is_unauthorized)
                && recovery.has_next()
                && recovery.next().await.is_ok()
            {
                continue;
            }
            self.ensure_identity()
                .await
                .map_err(|error| RequestError::Other(anyhow::anyhow!(error)))?;
            return result;
        }
    }
}

#[cfg(test)]
#[path = "analytics_session_tests.rs"]
mod tests;
