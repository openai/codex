use crate::config::Config;
use codex_account::AddCreditsNudgeEmailStatus;
use codex_account::SendAddCreditsNudgeEmailError;
use codex_login::AuthManager;

pub async fn send_add_credits_nudge_email(
    config: &Config,
    auth_manager: &AuthManager,
) -> Result<AddCreditsNudgeEmailStatus, SendAddCreditsNudgeEmailError> {
    codex_account::send_add_credits_nudge_email(config.chatgpt_base_url.clone(), auth_manager).await
}
