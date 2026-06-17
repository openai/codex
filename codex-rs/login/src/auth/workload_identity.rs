use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

use codex_app_server_protocol::AuthMode;
use codex_workload_identity_providers::ConfiguredWorkloadIdentityClient;
use codex_workload_identity_providers::WorkloadIdentityConfig;
use codex_workload_identity_providers::build_client as build_workload_identity_client;

use super::ExternalAuth;
use super::ExternalAuthFuture;
use super::ExternalAuthRefreshContext;
use super::ExternalAuthTokens;

pub(super) struct WorkloadIdentityExternalAuth {
    client: ConfiguredWorkloadIdentityClient,
    process_isolation_error: Option<String>,
}

struct SharedWorkloadIdentityExternalAuth {
    config: WorkloadIdentityConfig,
    client_id: String,
    auth: Weak<WorkloadIdentityExternalAuth>,
}

fn shared_workload_identity_auths() -> &'static Mutex<Vec<SharedWorkloadIdentityExternalAuth>> {
    static SHARED_AUTHS: OnceLock<Mutex<Vec<SharedWorkloadIdentityExternalAuth>>> = OnceLock::new();
    SHARED_AUTHS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(super) fn shared_workload_identity_external_auth(
    config: WorkloadIdentityConfig,
    client_id: String,
    http: reqwest::Client,
) -> Arc<dyn ExternalAuth> {
    let mut shared_auths = shared_workload_identity_auths()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    shared_auths.retain(|entry| entry.auth.strong_count() > 0);
    if let Some(auth) = shared_auths.iter().find_map(|entry| {
        (entry.config == config && entry.client_id == client_id)
            .then(|| entry.auth.upgrade())
            .flatten()
    }) {
        return auth;
    }

    let auth = Arc::new(WorkloadIdentityExternalAuth::new(
        build_workload_identity_client(config.clone(), client_id.clone(), http),
    ));
    shared_auths.push(SharedWorkloadIdentityExternalAuth {
        config,
        client_id,
        auth: Arc::downgrade(&auth),
    });
    auth
}

impl WorkloadIdentityExternalAuth {
    pub(super) fn new(client: ConfiguredWorkloadIdentityClient) -> Self {
        #[cfg(target_os = "windows")]
        // Core config forces WIF sessions onto the Windows restricted-token sandbox, which keeps
        // model-controlled child processes from opening the parent process.
        let process_isolation_error = None;
        #[cfg(not(target_os = "windows"))]
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
        Box::pin(async move {
            self.tokens(/*force_refresh*/ false).await.map(Some)
        })
    }

    fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> ExternalAuthFuture<'_, ExternalAuthTokens> {
        Box::pin(async move {
            self.tokens(/*force_refresh*/ true).await
        })
    }
}

#[cfg(test)]
#[path = "workload_identity_tests.rs"]
mod tests;
