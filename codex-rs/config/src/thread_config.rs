use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use codex_model_provider_info::ModelProviderInfo;
use thiserror::Error;

/// Context available to implementations when loading thread-scoped config.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadConfigContext {
    pub thread_id: Option<String>,
    pub cwd: Option<PathBuf>,
}

/// Config values owned by the service that starts or manages the session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionThreadConfig {
    pub model_provider: Option<String>,
    pub model_providers: HashMap<String, ModelProviderInfo>,
    pub features: BTreeMap<String, bool>,
}

/// Config values owned by the authenticated user.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UserThreadConfig {}

/// A typed config payload paired with the authority that produced it.
#[derive(Clone, Debug, PartialEq)]
pub enum ThreadConfigSource {
    Session(SessionThreadConfig),
    User(UserThreadConfig),
}

/// Stable category for failures returned while loading thread config.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadConfigLoadErrorCode {
    Auth,
    Timeout,
    Parse,
    RequestFailed,
    Internal,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ThreadConfigLoadError {
    code: ThreadConfigLoadErrorCode,
    message: String,
    status_code: Option<u16>,
}

impl ThreadConfigLoadError {
    pub fn new(
        code: ThreadConfigLoadErrorCode,
        status_code: Option<u16>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            status_code,
        }
    }

    pub fn code(&self) -> ThreadConfigLoadErrorCode {
        self.code
    }

    pub fn status_code(&self) -> Option<u16> {
        self.status_code
    }
}

/// Loads typed config sources for a new thread.
///
/// Implementations should fetch only the source-specific config they own and
/// return typed payloads without applying precedence or merge rules. Callers
/// are responsible for resolving the returned sources into the effective
/// runtime config.
#[async_trait]
pub trait ThreadConfigLoader: Send + Sync {
    async fn load(
        &self,
        context: ThreadConfigContext,
    ) -> Result<Vec<ThreadConfigSource>, ThreadConfigLoadError>;
}

/// Loader used when no external thread config source is configured.
#[derive(Clone, Debug, Default)]
pub struct NoopThreadConfigLoader;

#[async_trait]
impl ThreadConfigLoader for NoopThreadConfigLoader {
    async fn load(
        &self,
        _context: ThreadConfigContext,
    ) -> Result<Vec<ThreadConfigSource>, ThreadConfigLoadError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn loader_returns_session_and_user_sources() {
        struct TestThreadConfigLoader;

        #[async_trait]
        impl ThreadConfigLoader for TestThreadConfigLoader {
            async fn load(
                &self,
                context: ThreadConfigContext,
            ) -> Result<Vec<ThreadConfigSource>, ThreadConfigLoadError> {
                assert_eq!(context.thread_id.as_deref(), Some("thread-1"));
                Ok(vec![
                    ThreadConfigSource::Session(SessionThreadConfig {
                        model_provider: Some("local".to_string()),
                        model_providers: HashMap::from([(
                            "local".to_string(),
                            test_provider("local"),
                        )]),
                        features: BTreeMap::from([("plugins".to_string(), false)]),
                    }),
                    ThreadConfigSource::User(UserThreadConfig::default()),
                ])
            }
        }

        let loader = TestThreadConfigLoader;

        let sources = loader
            .load(ThreadConfigContext {
                thread_id: Some("thread-1".to_string()),
                ..Default::default()
            })
            .await
            .expect("thread config loads");

        assert_eq!(
            sources,
            vec![
                ThreadConfigSource::Session(SessionThreadConfig {
                    model_provider: Some("local".to_string()),
                    model_providers: HashMap::from([("local".to_string(), test_provider("local"))]),
                    features: BTreeMap::from([("plugins".to_string(), false)]),
                }),
                ThreadConfigSource::User(UserThreadConfig::default()),
            ]
        );
    }

    fn test_provider(name: &str) -> ModelProviderInfo {
        ModelProviderInfo {
            name: name.to_string(),
            base_url: Some("http://127.0.0.1:8061/api/codex".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: false,
            supports_websockets: true,
        }
    }
}
