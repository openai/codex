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
use futures::FutureExt;
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
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, CloudRequirementsLoadFailure>;
}

#[derive(Debug, Clone, Default, PartialEq)]
struct CloudRequirementsLoadOutcome {
    requirements: Option<ConfigRequirementsToml>,
    warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CloudRequirementsLoadFailure {
    status_code: Option<u16>,
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
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, CloudRequirementsLoadFailure> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud requirements"
                );
            })
            .map_err(|_| CloudRequirementsLoadFailure { status_code: None })?;

        let response = client.get_config_requirements_file().await.map_err(|err| {
            let status_code = extract_http_status_code(&err.to_string());
            tracing::warn!(
                error = %err,
                status_code,
                "Failed to fetch cloud requirements"
            );
            CloudRequirementsLoadFailure { status_code }
        })?;

        let Some(contents) = response.contents else {
            tracing::warn!("Cloud requirements response missing contents");
            return Ok(None);
        };

        Ok(Some(contents))
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

    async fn fetch_with_timeout(&self) -> CloudRequirementsLoadOutcome {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_requirements.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let result = match timeout(self.timeout, self.fetch()).await {
            Ok(result) => result,
            Err(_) => {
                let warning = "Failed to load Cloud Requirements: request timed out. Continuing without cloud requirements.".to_string();
                tracing::warn!("{warning}");
                return CloudRequirementsLoadOutcome {
                    requirements: None,
                    warning: Some(warning),
                };
            }
        };

        match result.requirements.as_ref() {
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

        if let Some(warning) = result.warning.as_deref() {
            tracing::warn!("{warning}");
        }

        result
    }

    async fn fetch(&self) -> CloudRequirementsLoadOutcome {
        let Some(auth) = self.auth_manager.auth().await else {
            return CloudRequirementsLoadOutcome::default();
        };
        if !auth.is_chatgpt_auth()
            || !matches!(
                auth.account_plan_type(),
                Some(PlanType::Business | PlanType::Enterprise)
            )
        {
            return CloudRequirementsLoadOutcome::default();
        }

        let contents = match self.fetcher.fetch_requirements(&auth).await {
            Ok(Some(contents)) => contents,
            Ok(None) => return CloudRequirementsLoadOutcome::default(),
            Err(err) => {
                return CloudRequirementsLoadOutcome {
                    requirements: None,
                    warning: Some(fetch_warning_message(err.status_code)),
                };
            }
        };
        match parse_cloud_requirements(&contents) {
            Ok(requirements) => CloudRequirementsLoadOutcome {
                requirements,
                warning: None,
            },
            Err(err) => {
                tracing::warn!(error = %err, "Failed to parse cloud requirements");
                CloudRequirementsLoadOutcome {
                    requirements: None,
                    warning: Some("Failed to load Cloud Requirements due to invalid response format. Continuing without cloud requirements.".to_string()),
                }
            }
        }
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
    let load_outcome = async move {
        task.await
            .inspect_err(|err| tracing::warn!(error = %err, "Cloud requirements task failed"))
            .ok()
            .unwrap_or_else(|| CloudRequirementsLoadOutcome {
                requirements: None,
                warning: Some(
                    "Failed to load Cloud Requirements due to an internal task failure. Continuing without cloud requirements.".to_string(),
                ),
            })
    }
    .shared();
    CloudRequirementsLoader::new_with_warning(
        {
            let load_outcome = load_outcome.clone();
            async move { load_outcome.await.requirements }
        },
        async move { load_outcome.await.warning },
    )
}

fn fetch_warning_message(status_code: Option<u16>) -> String {
    match status_code {
        Some(status_code) => format!(
            "Failed to load Cloud Requirements (HTTP {status_code}). Continuing without cloud requirements."
        ),
        None => {
            "Failed to load Cloud Requirements. Continuing without cloud requirements.".to_string()
        }
    }
}

fn extract_http_status_code(error_message: &str) -> Option<u16> {
    let status_text = error_message.split_once(" failed: ")?.1;
    let status_digits: String = status_text
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if status_digits.len() != 3 {
        return None;
    }
    status_digits.parse::<u16>().ok()
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
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, CloudRequirementsLoadFailure> {
            Ok(self.contents.clone())
        }
    }

    struct PendingFetcher;

    #[async_trait::async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, CloudRequirementsLoadFailure> {
            pending::<()>().await;
            Ok(None)
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
        assert_eq!(result, CloudRequirementsLoadOutcome::default());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_business_or_enterprise_plan() {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro"),
            Arc::new(StaticFetcher { contents: None }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert_eq!(result, CloudRequirementsLoadOutcome::default());
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
            CloudRequirementsLoadOutcome {
                requirements: Some(ConfigRequirementsToml {
                    allowed_approval_policies: Some(vec![AskForApproval::Never]),
                    allowed_sandbox_modes: None,
                    mcp_servers: None,
                    rules: None,
                    enforce_residency: None,
                }),
                warning: None,
            }
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
        assert_eq!(
            result.warning,
            Some(
                "Failed to load Cloud Requirements: request timed out. Continuing without cloud requirements.".to_string()
            )
        );
        assert_eq!(result.requirements, None);
    }

    #[test]
    fn parse_http_status_code_from_backend_error_message() {
        assert_eq!(
            extract_http_status_code(
                "GET https://chatgpt.com/backend-api/wham/config/requirements failed: 403 Forbidden; content-type=application/json; body={}"
            ),
            Some(403)
        );
        assert_eq!(
            extract_http_status_code("Decode error for https://example.com: unexpected EOF"),
            None
        );
    }

    #[test]
    fn fetch_warning_message_includes_status_code_when_available() {
        assert_eq!(
            fetch_warning_message(Some(429)),
            "Failed to load Cloud Requirements (HTTP 429). Continuing without cloud requirements."
        );
    }
}
