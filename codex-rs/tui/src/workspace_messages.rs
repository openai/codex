use crate::legacy_core::config::Config;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::CodexWorkspaceMessagesResponse;
use codex_login::AuthManager;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;

const WORKSPACE_MESSAGES_FETCH_TIMEOUT: Duration = Duration::from_millis(1000);

static WORKSPACE_MESSAGES: OnceLock<Option<CodexWorkspaceMessagesResponse>> = OnceLock::new();
static WORKSPACE_MESSAGES_FETCH_STARTED: OnceLock<()> = OnceLock::new();

pub(crate) fn prewarm_workspace_messages(config: &Config) {
    if WORKSPACE_MESSAGES_FETCH_STARTED.set(()).is_err() {
        return;
    }

    let config = config.clone();
    tokio::spawn(async move {
        let messages = timeout(
            WORKSPACE_MESSAGES_FETCH_TIMEOUT,
            fetch_workspace_messages(config),
        )
        .await
        .ok()
        .flatten();
        let _ = WORKSPACE_MESSAGES.set(messages);
    });
}

async fn fetch_workspace_messages(config: Config) -> Option<CodexWorkspaceMessagesResponse> {
    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;
    let auth = auth_manager.auth().await?;
    if !auth.uses_codex_backend() {
        return None;
    }

    let client = BackendClient::from_auth(config.chatgpt_base_url, &auth).ok()?;
    client.list_workspace_messages().await.ok()
}
