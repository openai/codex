use std::collections::HashMap;
use std::sync::Arc;

use codex_app_server_protocol::EnvironmentInfo;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_exec_server::EnvironmentManager;

use crate::error_code::INVALID_REQUEST_ERROR_CODE;

#[derive(Clone, Debug, Default)]
pub(crate) struct EnvironmentRegistry {
    environments: Arc<tokio::sync::RwLock<HashMap<String, RegisteredEnvironment>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegisteredEnvironment {
    pub(crate) id: String,
    pub(crate) exec_server_url: Option<String>,
}

impl RegisteredEnvironment {
    pub(crate) fn info(&self) -> EnvironmentInfo {
        EnvironmentInfo {
            id: self.id.clone(),
            exec_server_url: self.exec_server_url.clone(),
        }
    }

    pub(crate) fn manager(&self) -> Arc<EnvironmentManager> {
        Arc::new(EnvironmentManager::new(self.exec_server_url.clone()))
    }
}

impl EnvironmentRegistry {
    pub(crate) async fn register(
        &self,
        id: String,
        exec_server_url: Option<String>,
    ) -> RegisteredEnvironment {
        let normalized = RegisteredEnvironment {
            id: id.clone(),
            exec_server_url: exec_server_url
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty()),
        };
        self.environments
            .write()
            .await
            .insert(id, normalized.clone());
        normalized
    }

    pub(crate) async fn get(&self, id: &str) -> Option<RegisteredEnvironment> {
        self.environments.read().await.get(id).cloned()
    }

    pub(crate) async fn list(&self) -> Vec<EnvironmentInfo> {
        let mut environments = self
            .environments
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        environments.sort_by(|left, right| left.id.cmp(&right.id));
        environments.into_iter().map(|env| env.info()).collect()
    }

    pub(crate) async fn resolve(
        &self,
        environment_id: Option<&str>,
    ) -> Result<Option<RegisteredEnvironment>, JSONRPCErrorError> {
        let Some(environment_id) = environment_id else {
            return Ok(None);
        };

        self.get(environment_id)
            .await
            .ok_or_else(|| JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("unknown environment id `{environment_id}`"),
                data: None,
            })
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::EnvironmentRegistry;

    #[tokio::test]
    async fn register_normalizes_url_and_lists_sorted() {
        let registry = EnvironmentRegistry::default();
        registry
            .register("b".to_string(), Some("  ws://127.0.0.1:8123  ".to_string()))
            .await;
        registry
            .register("a".to_string(), Some("".to_string()))
            .await;

        let environments = registry.list().await;
        assert_eq!(environments.len(), 2);
        assert_eq!(environments[0].id, "a");
        assert_eq!(environments[0].exec_server_url, None);
        assert_eq!(environments[1].id, "b");
        assert_eq!(
            environments[1].exec_server_url.as_deref(),
            Some("ws://127.0.0.1:8123")
        );
    }

    #[tokio::test]
    async fn resolve_returns_error_for_unknown_environment() {
        let registry = EnvironmentRegistry::default();
        let error = registry
            .resolve(Some("missing"))
            .await
            .expect_err("resolve should fail for unknown environment");
        assert_eq!(error.message, "unknown environment id `missing`");
    }
}
