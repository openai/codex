use crate::config::Config;
use codex_account::AddCreditsNudgeEmailStatus;
use codex_account::SendAddCreditsNudgeEmailError;
use codex_account::WorkspaceOwnership;
use codex_login::AuthManager;
use codex_login::CodexAuth;

pub async fn send_add_credits_nudge_email(
    config: &Config,
    auth_manager: &AuthManager,
) -> Result<AddCreditsNudgeEmailStatus, SendAddCreditsNudgeEmailError> {
    codex_account::send_add_credits_nudge_email(config.chatgpt_base_url.clone(), auth_manager).await
}

pub async fn resolve_workspace_role_and_owner_for_auth(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> WorkspaceOwnership {
    codex_account::resolve_workspace_role_and_owner_for_auth(chatgpt_base_url, auth).await
}
