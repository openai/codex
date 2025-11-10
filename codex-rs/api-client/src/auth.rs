use async_trait::async_trait;
use codex_app_server_protocol::AuthMode;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthContext {
    pub mode: AuthMode,
    pub bearer_token: Option<String>,
    pub account_id: Option<String>,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn auth_context(&self) -> Option<AuthContext>;
    async fn refresh_token(&self) -> std::result::Result<Option<String>, String>;
}
