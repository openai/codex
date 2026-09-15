//! Account-scoped analytics authentication, credential recovery, and request identity checks.

use crate::Client;
use crate::RequestError;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_login::AuthManagerConfig;
use codex_login::CodexAuth;
use codex_protocol::account::PlanType;
use std::sync::Arc;

/// Non-secret account metadata associated with an analytics session.
#[derive(Clone, Debug)]
pub struct AnalyticsAccount {
    pub id: String,
    pub email: Option<String>,
    pub plan_type: Option<PlanType>,
}

/// A backend client that remains bound to its initial ChatGPT account and user.
pub struct AnalyticsSession {
    client: Client,
    auth_manager: Arc<AuthManager>,
    auth: CodexAuth,
    account: AnalyticsAccount,
}

impl AnalyticsSession {
    /// Load local ChatGPT credentials using the configured auth and HTTP policies.
    pub async fn from_config(
        config: &impl AuthManagerConfig,
        http_client_factory: HttpClientFactory,
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
            plan_type: auth.account_plan_type(),
        };
        let client = Client::new_without_redirects(config.chatgpt_base_url(), http_client_factory)
            .with_auth_provider(codex_model_provider::auth_provider_from_auth_manager(
                Arc::clone(&auth_manager),
                &auth,
            ));
        Ok(Self {
            client,
            auth_manager,
            auth,
            account,
        })
    }

    /// Return the account metadata captured when this session was opened.
    pub fn account(&self) -> &AnalyticsAccount {
        &self.account
    }
    /// Reject responses or cached data after a local account or user switch.
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
            if result.as_ref().is_err_and(RequestError::is_unauthorized) && recovery.has_next() {
                recovery.next().await.map_err(|_| {
                    RequestError::Other(anyhow::anyhow!("Sign in again to view Analytics."))
                })?;
                continue;
            }
            self.ensure_identity()
                .await
                .map_err(|error| RequestError::Other(anyhow::anyhow!(error)))?;
            return result;
        }
    }
}
