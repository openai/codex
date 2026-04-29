use std::collections::HashMap;

use async_trait::async_trait;

use crate::Environment;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::environment::CODEX_EXEC_SERVER_URL_ENV_VAR;
use crate::environment::LOCAL_ENVIRONMENT_ID;
use crate::environment::REMOTE_ENVIRONMENT_ID;

/// Lists the concrete environments available to Codex.
///
/// Implementations should return the provider-owned startup snapshot that
/// `EnvironmentManager` will cache. Providers that want the local environment to
/// be addressable by id should include it explicitly in the returned map.
#[async_trait]
pub trait EnvironmentProvider: Send + Sync {
    /// Returns the environments available for a new manager.
    async fn get_environments(
        &self,
        local_runtime_paths: &ExecServerRuntimePaths,
    ) -> Result<HashMap<String, Environment>, ExecServerError>;
}

/// Default provider backed by `CODEX_EXEC_SERVER_URL`.
#[derive(Clone, Debug)]
pub struct DefaultEnvironmentProvider {
    exec_server_url: Option<String>,
}

impl DefaultEnvironmentProvider {
    /// Builds a provider from an already-read raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self { exec_server_url }
    }

    /// Builds a provider by reading `CODEX_EXEC_SERVER_URL`.
    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    pub(crate) fn environments(
        &self,
        local_runtime_paths: &ExecServerRuntimePaths,
    ) -> HashMap<String, Environment> {
        let mut environments = HashMap::from([(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Environment::local(local_runtime_paths.clone()),
        )]);
        let exec_server_url = normalize_exec_server_url(self.exec_server_url.clone()).0;

        if let Some(exec_server_url) = exec_server_url {
            environments.insert(
                REMOTE_ENVIRONMENT_ID.to_string(),
                Environment::remote_inner(exec_server_url, Some(local_runtime_paths.clone())),
            );
        }

        environments
    }
}

#[async_trait]
impl EnvironmentProvider for DefaultEnvironmentProvider {
    async fn get_environments(
        &self,
        local_runtime_paths: &ExecServerRuntimePaths,
    ) -> Result<HashMap<String, Environment>, ExecServerError> {
        Ok(self.environments(local_runtime_paths))
    }
}

pub(crate) fn normalize_exec_server_url(exec_server_url: Option<String>) -> (Option<String>, bool) {
    match exec_server_url.as_deref().map(str::trim) {
        None | Some("") => (None, false),
        Some(url) if url.eq_ignore_ascii_case("none") => (None, true),
        Some(url) => (Some(url.to_string()), false),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ExecServerRuntimePaths;

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn default_provider_returns_local_environment_when_url_is_missing() {
        let provider = DefaultEnvironmentProvider::new(/*exec_server_url*/ None);
        let runtime_paths = test_runtime_paths();
        let environments = provider
            .get_environments(&runtime_paths)
            .await
            .expect("environments");

        assert!(!environments[LOCAL_ENVIRONMENT_ID].is_remote());
        assert_eq!(
            environments[LOCAL_ENVIRONMENT_ID].local_runtime_paths(),
            Some(&runtime_paths)
        );
        assert!(!environments.contains_key(REMOTE_ENVIRONMENT_ID));
    }

    #[tokio::test]
    async fn default_provider_returns_local_environment_when_url_is_empty() {
        let provider = DefaultEnvironmentProvider::new(Some(String::new()));
        let runtime_paths = test_runtime_paths();
        let environments = provider
            .get_environments(&runtime_paths)
            .await
            .expect("environments");

        assert!(!environments[LOCAL_ENVIRONMENT_ID].is_remote());
        assert!(!environments.contains_key(REMOTE_ENVIRONMENT_ID));
    }

    #[tokio::test]
    async fn default_provider_returns_local_environment_for_none_value() {
        let provider = DefaultEnvironmentProvider::new(Some("none".to_string()));
        let runtime_paths = test_runtime_paths();
        let environments = provider
            .get_environments(&runtime_paths)
            .await
            .expect("environments");

        assert!(!environments[LOCAL_ENVIRONMENT_ID].is_remote());
        assert!(!environments.contains_key(REMOTE_ENVIRONMENT_ID));
    }

    #[tokio::test]
    async fn default_provider_adds_remote_environment_for_websocket_url() {
        let provider = DefaultEnvironmentProvider::new(Some("ws://127.0.0.1:8765".to_string()));
        let runtime_paths = test_runtime_paths();
        let environments = provider
            .get_environments(&runtime_paths)
            .await
            .expect("environments");

        assert!(!environments[LOCAL_ENVIRONMENT_ID].is_remote());
        let remote_environment = &environments[REMOTE_ENVIRONMENT_ID];
        assert!(remote_environment.is_remote());
        assert_eq!(
            remote_environment.exec_server_url(),
            Some("ws://127.0.0.1:8765")
        );
    }

    #[tokio::test]
    async fn default_provider_normalizes_exec_server_url() {
        let provider = DefaultEnvironmentProvider::new(Some(" ws://127.0.0.1:8765 ".to_string()));
        let runtime_paths = test_runtime_paths();
        let environments = provider
            .get_environments(&runtime_paths)
            .await
            .expect("environments");

        assert_eq!(
            environments[REMOTE_ENVIRONMENT_ID].exec_server_url(),
            Some("ws://127.0.0.1:8765")
        );
    }
}
