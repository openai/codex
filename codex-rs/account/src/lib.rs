use codex_backend_client::Client as BackendClient;
use codex_backend_client::RequestError;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddCreditsNudgeEmailStatus {
    Sent,
    CooldownActive,
}

#[derive(Debug, Error)]
pub enum SendAddCreditsNudgeEmailError {
    #[error("codex account authentication required to notify workspace owner")]
    AuthRequired,

    #[error("chatgpt authentication required to notify workspace owner")]
    ChatGptAuthRequired,

    #[error("failed to construct backend client: {0}")]
    CreateClient(#[from] anyhow::Error),

    #[error("failed to notify workspace owner: {0}")]
    Request(#[from] RequestError),
}

pub async fn send_add_credits_nudge_email(
    chatgpt_base_url: impl Into<String>,
    auth_manager: &AuthManager,
) -> Result<AddCreditsNudgeEmailStatus, SendAddCreditsNudgeEmailError> {
    let auth = auth_manager
        .auth()
        .await
        .ok_or(SendAddCreditsNudgeEmailError::AuthRequired)?;
    send_add_credits_nudge_email_for_auth(chatgpt_base_url, &auth).await
}

pub async fn send_add_credits_nudge_email_for_auth(
    chatgpt_base_url: impl Into<String>,
    auth: &CodexAuth,
) -> Result<AddCreditsNudgeEmailStatus, SendAddCreditsNudgeEmailError> {
    if !auth.is_chatgpt_auth() {
        return Err(SendAddCreditsNudgeEmailError::ChatGptAuthRequired);
    }

    let client = BackendClient::from_auth(chatgpt_base_url, auth)?;
    match client.send_add_credits_nudge_email().await {
        Ok(()) => Ok(AddCreditsNudgeEmailStatus::Sent),
        Err(err) if err.status().is_some_and(|status| status.as_u16() == 429) => {
            Ok(AddCreditsNudgeEmailStatus::CooldownActive)
        }
        Err(err) => Err(err.into()),
    }
}
