use codex_app_server_protocol::AuthMode;
use codex_workload_identity_providers::ConfiguredWorkloadIdentityClient;

use super::ExternalAuth;
use super::ExternalAuthFuture;
use super::ExternalAuthRefreshContext;
use super::ExternalAuthTokens;

pub(super) struct WorkloadIdentityExternalAuth {
    client: ConfiguredWorkloadIdentityClient,
    process_isolation_error: Option<String>,
}

impl WorkloadIdentityExternalAuth {
    pub(super) fn new(client: ConfiguredWorkloadIdentityClient) -> Self {
        let process_isolation_error = codex_process_hardening::disable_process_inspection()
            .err()
            .map(|error| format!("workload identity process isolation failed: {error}"));
        Self {
            client,
            process_isolation_error,
        }
    }

    async fn tokens(&self, force_refresh: bool) -> std::io::Result<ExternalAuthTokens> {
        if let Some(error) = self.process_isolation_error.as_ref() {
            return Err(std::io::Error::other(error.clone()));
        }
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

impl ExternalAuth for WorkloadIdentityExternalAuth {
    fn auth_mode(&self) -> AuthMode {
        AuthMode::Chatgpt
    }

    fn requires_successful_resolution(&self) -> bool {
        true
    }

    fn resolve(&self) -> ExternalAuthFuture<'_, Option<ExternalAuthTokens>> {
        Box::pin(async move { self.tokens(false).await.map(Some) })
    }

    fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> ExternalAuthFuture<'_, ExternalAuthTokens> {
        Box::pin(async move { self.tokens(true).await })
    }
}
