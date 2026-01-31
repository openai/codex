//! Cloud-hosted config requirements for Codex.
//!
//! This crate fetches `requirements.toml` data from the backend as an alternative to loading it
//! from the local filesystem. It only applies to Enterprise ChatGPT customers.
//!
//! Enterprise ChatGPT customers must successfully fetch these requirements before Codex will run.

use async_trait::async_trait;
use codex_backend_client::Client as BackendClient;
use codex_core::AuthManager;
use codex_core::auth::CodexAuth;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_protocol::account::PlanType;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::time::timeout;

/// This blocks codecs startup, so must be short.
const CLOUD_REQUIREMENTS_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
enum CloudRequirementsError {
    #[error("cloud requirements user error: {0}")]
    User(CloudRequirementsUserError),
    #[error("cloud requirements network error: {0}")]
    Network(CloudRequirementsNetworkError),
}

impl From<CloudRequirementsUserError> for CloudRequirementsError {
    fn from(err: CloudRequirementsUserError) -> Self {
        CloudRequirementsError::User(err)
    }
}

impl From<CloudRequirementsNetworkError> for CloudRequirementsError {
    fn from(err: CloudRequirementsNetworkError) -> Self {
        CloudRequirementsError::Network(err)
    }
}

impl From<CloudRequirementsError> for io::Error {
    fn from(err: CloudRequirementsError) -> Self {
        let kind = match &err {
            CloudRequirementsError::User(_) => io::ErrorKind::InvalidData,
            CloudRequirementsError::Network(CloudRequirementsNetworkError::Timeout { .. }) => {
                io::ErrorKind::TimedOut
            }
            CloudRequirementsError::Network(_) => io::ErrorKind::Other,
        };
        io::Error::new(kind, err)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
enum CloudRequirementsUserError {
    #[error("failed to parse requirements TOML: {message}")]
    InvalidToml { message: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
enum CloudRequirementsNetworkError {
    #[error("backend client initialization failed: {message}")]
    BackendClient { message: String },
    #[error("request failed: {message}")]
    Request { message: String },
    #[error("cloud requirements response missing contents")]
    MissingContents,
    #[error("timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("cloud requirements task failed: {message}")]
    Task { message: String },
}

#[async_trait]
trait RequirementsFetcher: Send + Sync {
    /// Returns requirements as a TOML string.
    async fn fetch_requirements(&self, auth: &CodexAuth) -> Result<String, CloudRequirementsError>;
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
    async fn fetch_requirements(&self, auth: &CodexAuth) -> Result<String, CloudRequirementsError> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud requirements"
                );
            })
            .map_err(|err| CloudRequirementsNetworkError::BackendClient {
                message: err.to_string(),
            })
            .map_err(CloudRequirementsError::from)?;

        let response = client
            .get_config_requirements_file()
            .await
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to fetch cloud requirements"))
            .map_err(|err| CloudRequirementsNetworkError::Request {
                message: err.to_string(),
            })
            .map_err(CloudRequirementsError::from)?;

        let Some(contents) = response.contents else {
            tracing::warn!("Cloud requirements response missing contents");
            return Err(CloudRequirementsError::from(
                CloudRequirementsNetworkError::MissingContents,
            ));
        };

        Ok(contents)
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

    async fn fetch_with_timeout(
        &self,
    ) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_requirements.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let result = timeout(self.timeout, self.fetch()).await.map_err(|_| {
            CloudRequirementsNetworkError::Timeout {
                timeout_ms: self.timeout.as_millis() as u64,
            }
        })?;

        let elapsed_ms = started_at.elapsed().as_millis();
        match result.as_ref() {
            Ok(Some(requirements)) => {
                tracing::info!(
                    elapsed_ms,
                    status = "success",
                    requirements = ?requirements,
                    "Cloud requirements load completed"
                );
                println!(
                    "cloud_requirements status=success elapsed_ms={elapsed_ms} value={requirements:?}"
                );
            }
            Ok(None) => {
                tracing::info!(
                    elapsed_ms,
                    status = "none",
                    requirements = %"none",
                    "Cloud requirements load completed"
                );
                println!("cloud_requirements status=none elapsed_ms={elapsed_ms} value=none");
            }
            Err(err) => {
                tracing::warn!(
                    elapsed_ms,
                    status = "error",
                    requirements = %"none",
                    error = %err,
                    "Cloud requirements load failed"
                );
                println!(
                    "cloud_requirements status=error elapsed_ms={elapsed_ms} value=none error={err}"
                );
            }
        }

        result
    }

    async fn fetch(&self) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsError> {
        let auth = match self.auth_manager.auth().await {
            Some(auth) => auth,
            None => return Ok(None),
        };
        if !(auth.is_chatgpt_auth() && auth.account_plan_type() == Some(PlanType::Enterprise)) {
            return Ok(None);
        }

        let contents = self.fetcher.fetch_requirements(&auth).await?;
        parse_cloud_requirements(&contents)
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to parse cloud requirements"))
            .map_err(CloudRequirementsError::from)
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
            .map_err(|err| {
                CloudRequirementsError::from(CloudRequirementsNetworkError::Task {
                    message: err.to_string(),
                })
            })
            .and_then(std::convert::identity)
            .map_err(io::Error::from)
            .inspect_err(|err| tracing::warn!(error = %err, "Cloud requirements task failed"))
    })
}

fn parse_cloud_requirements(
    contents: &str,
) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsUserError> {
    if contents.trim().is_empty() {
        return Ok(None);
    }

    let requirements: ConfigRequirementsToml =
        toml::from_str(contents).map_err(|err| CloudRequirementsUserError::InvalidToml {
            message: err.to_string(),
        })?;
    if requirements.is_empty() {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_core::auth::AuthCredentialsStoreMode;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::future::pending;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_auth_json(codex_home: &Path, value: serde_json::Value) -> Result<()> {
        std::fs::write(codex_home.join("auth.json"), serde_json::to_string(&value)?)?;
        Ok(())
    }

    fn auth_manager_with_api_key() -> Result<Arc<AuthManager>> {
        let tmp = tempdir()?;
        let auth_json = json!({
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json)?;
        Ok(Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        )))
    }

    fn auth_manager_with_plan(plan_type: &str) -> Result<Arc<AuthManager>> {
        let tmp = tempdir()?;
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
        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header)?);
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?);
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
        write_auth_json(tmp.path(), auth_json)?;
        Ok(Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            false,
            AuthCredentialsStoreMode::File,
        )))
    }

    fn parse_for_fetch(
        contents: Option<&str>,
    ) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsUserError> {
        contents.map(parse_cloud_requirements).unwrap_or(Ok(None))
    }

    struct StaticFetcher {
        result: Result<String, CloudRequirementsError>,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for StaticFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<String, CloudRequirementsError> {
            self.result.clone()
        }
    }

    struct PendingFetcher;

    #[async_trait::async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<String, CloudRequirementsError> {
            pending::<()>().await;
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_chatgpt_auth() -> Result<()> {
        let service = CloudRequirementsService::new(
            auth_manager_with_api_key()?,
            Arc::new(StaticFetcher {
                result: Ok(String::new()),
            }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(service.fetch().await, Ok(None));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_enterprise_plan() -> Result<()> {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro")?,
            Arc::new(StaticFetcher {
                result: Ok(String::new()),
            }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(service.fetch().await, Ok(None));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_returns_missing_contents_error() -> Result<()> {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise")?,
            Arc::new(StaticFetcher {
                result: Err(CloudRequirementsError::Network(
                    CloudRequirementsNetworkError::MissingContents,
                )),
            }),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Err(CloudRequirementsError::Network(
                CloudRequirementsNetworkError::MissingContents
            ))
        );
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_empty_contents() -> Result<()> {
        assert_eq!(parse_for_fetch(Some("   ")), Ok(None));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_invalid_toml() -> Result<()> {
        assert!(matches!(
            parse_for_fetch(Some("not = [")),
            Err(CloudRequirementsUserError::InvalidToml { .. })
        ));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_empty_requirements() -> Result<()> {
        assert_eq!(parse_for_fetch(Some("# comment")), Ok(None));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parses_valid_toml() -> Result<()> {
        assert_eq!(
            parse_for_fetch(Some("allowed_approval_policies = [\"never\"]")),
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
            }))
        );
        Ok(())
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_times_out() -> Result<()> {
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise")?,
            Arc::new(PendingFetcher),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_REQUIREMENTS_TIMEOUT + Duration::from_millis(1)).await;

        assert_eq!(
            handle.await?,
            Err(CloudRequirementsError::Network(
                CloudRequirementsNetworkError::Timeout {
                    timeout_ms: CLOUD_REQUIREMENTS_TIMEOUT.as_millis() as u64,
                }
            ))
        );
        Ok(())
    }
}
