use async_trait::async_trait;
use codex_app_server_protocol::AuthMode;
use codex_workload_identity_providers::ConfiguredWorkloadIdentityClient;

use super::ExternalAuth;
use super::ExternalAuthRefreshContext;
use super::ExternalAuthTokens;

pub(super) struct WorkloadIdentityExternalAuth {
    client: ConfiguredWorkloadIdentityClient,
}

impl WorkloadIdentityExternalAuth {
    pub(super) fn new(client: ConfiguredWorkloadIdentityClient) -> Self {
        Self { client }
    }

    async fn tokens(&self, force_refresh: bool) -> std::io::Result<ExternalAuthTokens> {
        let token = if force_refresh {
            self.client.refresh().await
        } else {
            self.client.resolve().await
        }
        .map_err(std::io::Error::other)?;

        Ok(ExternalAuthTokens::chatgpt(
            token.access_token,
            token.chatgpt_account_id,
            token.chatgpt_plan_type,
        ))
    }
}

#[async_trait]
impl ExternalAuth for WorkloadIdentityExternalAuth {
    fn auth_mode(&self) -> AuthMode {
        AuthMode::Chatgpt
    }

    async fn resolve(&self) -> std::io::Result<Option<ExternalAuthTokens>> {
        self.tokens(false).await.map(Some)
    }

    async fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens> {
        self.tokens(true).await
    }
}
