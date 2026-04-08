use codex_backend_client::Client as BackendClient;
use codex_backend_client::RequestError;
use codex_backend_client::WorkspaceRole as BackendWorkspaceRole;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRole {
    AccountOwner,
    AccountAdmin,
    StandardUser,
}

impl WorkspaceRole {
    fn is_workspace_owner(self) -> bool {
        matches!(self, Self::AccountOwner | Self::AccountAdmin)
    }
}

impl From<BackendWorkspaceRole> for WorkspaceRole {
    fn from(value: BackendWorkspaceRole) -> Self {
        match value {
            BackendWorkspaceRole::AccountOwner => Self::AccountOwner,
            BackendWorkspaceRole::AccountAdmin => Self::AccountAdmin,
            BackendWorkspaceRole::StandardUser => Self::StandardUser,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkspaceOwnership {
    pub workspace_role: Option<WorkspaceRole>,
    pub is_workspace_owner: Option<bool>,
}

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

pub async fn resolve_workspace_role_and_owner_for_auth(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> WorkspaceOwnership {
    let token_is_workspace_owner = auth.and_then(CodexAuth::is_workspace_owner);
    let Some(auth) = auth else {
        return WorkspaceOwnership::default();
    };

    let workspace_role = fetch_current_workspace_role_for_auth(chatgpt_base_url, auth).await;
    let is_workspace_owner = workspace_role
        .map(WorkspaceRole::is_workspace_owner)
        .or(token_is_workspace_owner);
    WorkspaceOwnership {
        workspace_role,
        is_workspace_owner,
    }
}

async fn fetch_current_workspace_role_for_auth(
    chatgpt_base_url: &str,
    auth: &CodexAuth,
) -> Option<WorkspaceRole> {
    if !auth.is_chatgpt_auth() || auth.get_account_id().is_none() {
        return None;
    }

    let client = BackendClient::from_auth(chatgpt_base_url.to_string(), auth).ok()?;
    client
        .get_current_workspace_role()
        .await
        .ok()
        .flatten()
        .map(WorkspaceRole::from)
}
