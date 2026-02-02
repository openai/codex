//! Cloud-hosted config requirements for Codex.
//!
//! This crate fetches `requirements.toml` data from the backend as an alternative to loading it
//! from the local filesystem. It only applies to Business (aka Enterprise CBP) or Enterprise ChatGPT
//! customers.
//!
//! Today, fetching is best-effort: on error or timeout, Codex continues without cloud requirements.
//! We expect to tighten this so that Enterprise ChatGPT customers must successfully fetch these
//! requirements before Codex will run.

use async_trait::async_trait;
use codex_backend_client::Client as BackendClient;
use codex_core::AuthManager;
use codex_core::auth::CodexAuth;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_protocol::account::PlanType;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::time::timeout;

/// This blocks codex startup, so must be short.
const CLOUD_REQUIREMENTS_TIMEOUT: Duration = Duration::from_secs(5);

#[async_trait]
trait RequirementsFetcher: Send + Sync {
    /// Returns requirements as a TOML string.
    ///
    /// TODO(gt): For now, returns an Option. But when we want to make this fail-closed, return a
    /// Result.
    async fn fetch_requirements(&self, auth: &CodexAuth) -> Option<String>;
}

struct BackendRequirementsFetcher {
    base_url: String,
}

impl BackendRequirementsFetcher {
    fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait]
impl RequirementsFetcher for BackendRequirementsFetcher {
    async fn fetch_requirements(&self, auth: &CodexAuth) -> Option<String> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud requirements"
                );
            })
            .ok()?;

        let response = client
            .get_config_requirements_file()
            .await
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to fetch cloud requirements"))
            .ok()?;

        let Some(contents) = response.contents else {
            tracing::warn!("Cloud requirements response missing contents");
            return None;
        };

        Some(contents)
    }
}

struct CloudRequirementsService {
    auth_manager: Arc<AuthManager>,
    fetcher: Arc<dyn RequirementsFetcher>,
    timeout: Duration,
}

impl CloudRequirementsService {
    fn new(
        auth_manager: Arc<AuthManager>,
        fetcher: Arc<dyn RequirementsFetcher>,
        timeout: Duration,
    ) -> Self {
        Self {
            auth_manager,
            fetcher,
            timeout,
        }
    }

    async fn fetch_with_timeout(&self) -> Option<ConfigRequirementsToml> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_requirements.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let result = timeout(self.timeout, self.fetch())
            .await
            .inspect_err(|_| {
                tracing::warn!("Timed out waiting for cloud requirements; continuing without them");
            })
            .ok()?;

        match result.as_ref() {
            Some(requirements) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    requirements = ?requirements,
                    "Cloud requirements load completed"
                );
            }
            None => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "Cloud requirements load completed (none)"
                );
            }
        }

        result
    }

    async fn fetch(&self) -> Option<ConfigRequirementsToml> {
        let auth = self.auth_manager.auth().await?;
        if !auth.is_chatgpt_auth()
            || !matches!(
                auth.account_plan_type(),
                Some(PlanType::Business | PlanType::Enterprise)
            )
        {
            return None;
        }

        let contents = self.fetcher.fetch_requirements(&auth).await?;
        parse_cloud_requirements(&contents)
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to parse cloud requirements"))
            .ok()
            .flatten()
    }
}

pub fn cloud_requirements_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
) -> CloudRequirementsLoader {
    let service = CloudRequirementsService::new(
        auth_manager,
        Arc::new(BackendRequirementsFetcher::new(chatgpt_base_url)),
        CLOUD_REQUIREMENTS_TIMEOUT,
    );
    let task = tokio::spawn(async move { service.fetch_with_timeout().await });
    CloudRequirementsLoader::new(async move {
        task.await
            .inspect_err(|err| tracing::warn!(error = %err, "Cloud requirements task failed"))
            .ok()
            .flatten()
    })
}

fn parse_cloud_requirements(
    contents: &str,
) -> Result<Option<ConfigRequirementsToml>, toml::de::Error> {
    if contents.trim().is_empty() {
        return Ok(None);
    }

    let requirements: ConfigRequirementsToml = toml::from_str(contents)?;
    if requirements.is_empty() {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::future::pending;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_auth_json(codex_home: &Path, value: serde_json::Value) -> std::io::Result<()> {
        std::fs::write(codex_home.join("auth.json"), serde_json::to_string(&value)?)?;
        Ok(())
    }

    fn auth_manager_with_api_key() -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let auth_json = json!({
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ))
    }

    fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let header = json!({ "alg": "none", "typ": "JWT" });
        let auth_payload = json!({
            "chatgpt_plan_type": plan_type,
            "chatgpt_user_id": "user-12345",
            "user_id": "user-12345",
        });
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": auth_payload,
        });
        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header"));
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("payload"));
        let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let auth_json = json!({
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": fake_jwt,
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token",
            },
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        ))
    }

    fn parse_for_fetch(contents: Option<&str>) -> Option<ConfigRequirementsToml> {
        contents.and_then(|contents| parse_cloud_requirements(contents).ok().flatten())
    }

    struct StaticFetcher {
        contents: Option<String>,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for StaticFetcher {
        async fn fetch_requirements(&self, _auth: &CodexAuth) -> Option<String> {
            self.contents.clone()
        }
    }

    struct PendingFetcher;

    #[async_trait::async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(&self, _auth: &CodexAuth) -> Option<String> {
            pending::<()>().await;
            None
        }
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_chatgpt_auth() {
        let auth_manager = auth_manager_with_api_key();
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(StaticFetcher { contents: None }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_business_or_enterprise_plan() {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro"),
            Arc::new(StaticFetcher { contents: None }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_plan() {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
            })
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_missing_contents() {
        let result = parse_for_fetch(None);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_empty_contents() {
        let result = parse_for_fetch(Some("   "));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_invalid_toml() {
        let result = parse_for_fetch(Some("not = ["));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_empty_requirements() {
        let result = parse_for_fetch(Some("# comment"));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parses_valid_toml() {
        let result = parse_for_fetch(Some("allowed_approval_policies = [\"never\"]"));

        assert_eq!(
            result,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_times_out() {
        let auth_manager = auth_manager_with_plan("enterprise");
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(PendingFetcher),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_REQUIREMENTS_TIMEOUT + Duration::from_millis(1)).await;

        let result = handle.await.expect("cloud requirements task");
        assert!(result.is_none());
    }
}
