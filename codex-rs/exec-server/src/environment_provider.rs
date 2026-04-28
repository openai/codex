use std::collections::HashMap;

use async_trait::async_trait;

use crate::ExecServerError;
use crate::environment::CODEX_EXEC_SERVER_URL_ENV_VAR;
use crate::environment::REMOTE_ENVIRONMENT_ID;

/// Provider-supplied environment definition consumed by `EnvironmentManager`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfiguration {
    pub exec_server_url: String,
}

/// Provider-supplied environment snapshot consumed by `EnvironmentManager`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentConfigurations {
    environments: HashMap<String, EnvironmentConfiguration>,
}

impl EnvironmentConfigurations {
    pub fn new(
        mut environments: HashMap<String, EnvironmentConfiguration>,
    ) -> Result<Self, ExecServerError> {
        for (id, configuration) in &mut environments {
            if id.is_empty() {
                return Err(ExecServerError::Protocol(
                    "environment configuration id cannot be empty".to_string(),
                ));
            }

            match normalize_exec_server_url(Some(configuration.exec_server_url.clone())) {
                (Some(exec_server_url), false) => {
                    configuration.exec_server_url = exec_server_url;
                }
                (None, false) | (None, true) | (Some(_), true) => {
                    return Err(ExecServerError::Protocol(format!(
                        "environment configuration `{id}` must set a remote exec-server URL"
                    )));
                }
            }
        }

        Ok(Self { environments })
    }

    pub fn empty() -> Self {
        Self {
            environments: HashMap::new(),
        }
    }

    pub(crate) fn remote(exec_server_url: String) -> Self {
        Self {
            environments: HashMap::from([(
                REMOTE_ENVIRONMENT_ID.to_string(),
                EnvironmentConfiguration { exec_server_url },
            )]),
        }
    }

    pub(crate) fn into_environments(self) -> HashMap<String, EnvironmentConfiguration> {
        self.environments
    }
}

/// Lists the concrete environment configurations available to Codex.
///
/// Implementations should return the provider-owned portion of the startup
/// snapshot that `EnvironmentManager` will cache. The local environment is
/// always supplied by `EnvironmentManager`.
#[async_trait]
pub trait EnvironmentProvider: Send + Sync {
    /// Returns the environment configurations available for a new manager.
    async fn get_environments(&self) -> Result<EnvironmentConfigurations, ExecServerError>;
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

    pub(crate) async fn environment_configurations(&self) -> EnvironmentConfigurations {
        let exec_server_url = normalize_exec_server_url(self.exec_server_url.clone()).0;

        if let Some(exec_server_url) = exec_server_url {
            EnvironmentConfigurations::remote(exec_server_url)
        } else {
            EnvironmentConfigurations::empty()
        }
    }
}

#[async_trait]
impl EnvironmentProvider for DefaultEnvironmentProvider {
    async fn get_environments(&self) -> Result<EnvironmentConfigurations, ExecServerError> {
        Ok(self.environment_configurations().await)
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

    #[tokio::test]
    async fn default_provider_returns_no_provider_environment_when_url_is_missing() {
        let provider = DefaultEnvironmentProvider::new(/*exec_server_url*/ None);

        assert_eq!(
            provider.get_environments().await.expect("environments"),
            EnvironmentConfigurations::empty()
        );
    }

    #[tokio::test]
    async fn default_provider_returns_no_provider_environment_when_url_is_empty() {
        let provider = DefaultEnvironmentProvider::new(Some(String::new()));

        assert_eq!(
            provider.get_environments().await.expect("environments"),
            EnvironmentConfigurations::empty()
        );
    }

    #[tokio::test]
    async fn default_provider_returns_no_provider_environment_for_none_value() {
        let provider = DefaultEnvironmentProvider::new(Some("none".to_string()));

        assert_eq!(
            provider.get_environments().await.expect("environments"),
            EnvironmentConfigurations::empty()
        );
    }

    #[tokio::test]
    async fn default_provider_adds_remote_environment_for_websocket_url() {
        let provider = DefaultEnvironmentProvider::new(Some("ws://127.0.0.1:8765".to_string()));

        assert_eq!(
            provider.get_environments().await.expect("environments"),
            EnvironmentConfigurations::new(HashMap::from([(
                REMOTE_ENVIRONMENT_ID.to_string(),
                EnvironmentConfiguration {
                    exec_server_url: "ws://127.0.0.1:8765".to_string(),
                },
            )]))
            .expect("environment configurations")
        );
    }

    #[test]
    fn environment_configurations_rejects_empty_exec_server_url() {
        let err = EnvironmentConfigurations::new(HashMap::from([(
            REMOTE_ENVIRONMENT_ID.to_string(),
            EnvironmentConfiguration {
                exec_server_url: String::new(),
            },
        )]))
        .expect_err("empty URL should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment configuration `remote` must set a remote exec-server URL"
        );
    }

    #[test]
    fn environment_configurations_rejects_disabled_exec_server_url() {
        let err = EnvironmentConfigurations::new(HashMap::from([(
            REMOTE_ENVIRONMENT_ID.to_string(),
            EnvironmentConfiguration {
                exec_server_url: "none".to_string(),
            },
        )]))
        .expect_err("disabled URL should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: environment configuration `remote` must set a remote exec-server URL"
        );
    }

    #[test]
    fn environment_configurations_normalizes_exec_server_url() {
        let configurations = EnvironmentConfigurations::new(HashMap::from([(
            REMOTE_ENVIRONMENT_ID.to_string(),
            EnvironmentConfiguration {
                exec_server_url: " ws://127.0.0.1:8765 ".to_string(),
            },
        )]))
        .expect("environment configurations");

        assert_eq!(
            configurations,
            EnvironmentConfigurations::remote("ws://127.0.0.1:8765".to_string())
        );
    }
}
