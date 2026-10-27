//! Switches the workspace of managed ChatGPT auth without a new login.
//! Refresh credentials remain owned by the existing auth store.

use super::*;
use codex_http_client::ClientRouteClass;
use codex_http_client::RouteAwareClientPool;
use serde_json::json;
use std::io;

#[derive(Deserialize)]
struct WorkspaceTokenResponse {
    access_token: String,
}

/// Allows the caller to restore credentials if target workspace validation fails.
/// Restoration is conditional so it cannot overwrite a newer login or logout.
pub struct WorkspaceSwitch {
    storage: Arc<dyn AuthStorageBackend>,
    previous: AuthDotJson,
    selected: AuthDotJson,
}

impl std::fmt::Debug for WorkspaceSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceSwitch").finish_non_exhaustive()
    }
}

/// Binds a browser callback to the managed login that requested the workspace.
#[derive(Clone)]
pub struct WorkspaceLogin {
    storage: Arc<dyn AuthStorageBackend>,
    original: AuthDotJson,
    pub(crate) workspace_id: String,
    pub(crate) email: Option<String>,
    pub(crate) connection: Option<String>,
}

impl std::fmt::Debug for WorkspaceLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceLogin").finish_non_exhaustive()
    }
}

impl WorkspaceLogin {
    pub(crate) fn save(&self, mut updated: AuthDotJson) -> io::Result<()> {
        let previous = self
            .original
            .tokens
            .as_ref()
            .ok_or_else(|| io::Error::other("Missing managed login"))?;
        let tokens = updated
            .tokens
            .as_mut()
            .ok_or_else(|| io::Error::other("Missing sign-in credentials"))?;
        let access = parse_chatgpt_jwt_claims(&tokens.access_token).map_err(io::Error::other)?;
        if previous.id_token.chatgpt_user_id.is_none()
            || tokens.id_token.chatgpt_user_id != previous.id_token.chatgpt_user_id
            || access.chatgpt_user_id != previous.id_token.chatgpt_user_id
            || tokens.account_id.as_deref() != Some(&self.workspace_id)
            || access.chatgpt_account_id != tokens.account_id
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Sign-in returned a different user or workspace",
            ));
        }
        tokens.login_owner_id.clone_from(&previous.login_owner_id);
        replace_if_current(self.storage.as_ref(), &self.original, &updated)
    }
}

impl CodexAuth {
    pub fn workspace_login(
        &self,
        session_id: &str,
        workspace_id: String,
        connection: Option<String>,
    ) -> io::Result<WorkspaceLogin> {
        let Self::Chatgpt(auth) = self else {
            return Err(io::Error::other(
                "Workspace sign-in requires a managed login",
            ));
        };
        let original = auth
            .current_auth_json()
            .ok_or_else(|| io::Error::other("Missing managed login"))?;
        if original.tokens.as_ref().and_then(managed_session_id) != Some(session_id) {
            return Err(io::Error::other("The active login changed"));
        }
        Ok(WorkspaceLogin {
            storage: auth.storage().clone(),
            original,
            workspace_id,
            email: self.get_account_email(),
            connection,
        })
    }

    /// Stable across token rotation and workspace changes for the managed login.
    pub fn managed_account_session_id(&self) -> Option<String> {
        if !matches!(self, Self::Chatgpt(_)) {
            return None;
        }
        let tokens = self.get_current_token_data()?;
        managed_session_id(&tokens).map(str::to_owned)
    }
}

impl AuthManager {
    /// Checks local eligibility without changing credentials or an in-progress login.
    pub fn validate_workspace_switch(
        &self,
        session_id: &str,
        workspace_id: &str,
    ) -> io::Result<()> {
        if workspace_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "accountId must not be empty",
            ));
        }
        ensure_auth_workspace_allowed(
            self.effective_chatgpt_workspaces().as_deref(),
            workspace_id,
        )?;
        let auth = self.auth_cached();
        if !matches!(auth, Some(CodexAuth::Chatgpt(_))) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Workspace switching requires a managed ChatGPT login",
            ));
        }
        if self.has_external_auth() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Workspace switching is owned by the external authentication provider",
            ));
        }
        if auth
            .and_then(|auth| auth.managed_account_session_id())
            .as_deref()
            != Some(session_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Only the active managed account session can be switched",
            ));
        }
        Ok(())
    }

    /// Exchanges the current session into another workspace without exporting credentials.
    pub async fn switch_workspace(
        &self,
        session_id: &str,
        workspace_id: &str,
    ) -> io::Result<WorkspaceSwitch> {
        self.validate_workspace_switch(session_id, workspace_id)?;
        self.auth().await;
        let _guard = self
            .refresh_lock
            .acquire()
            .await
            .map_err(io::Error::other)?;
        self.validate_workspace_switch(session_id, workspace_id)?;
        let Some(CodexAuth::Chatgpt(auth)) = self.auth_cached() else {
            return Err(io::Error::other("Authentication is unavailable"));
        };
        let snapshot = auth
            .current_auth_json()
            .ok_or_else(|| io::Error::other("Authentication is unavailable"))?;
        let tokens = snapshot
            .tokens
            .as_ref()
            .ok_or_else(|| io::Error::other("Authentication is unavailable"))?;
        let user_id = tokens
            .id_token
            .chatgpt_user_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| io::Error::other("Authentication is missing a user identity"))?;
        // reload() may replace cached auth without the refresh lock. Bind the
        // request to the exact snapshot used by both the exchange and conditional save.
        if managed_session_id(tokens) != Some(session_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Only the active managed account session can be switched",
            ));
        }
        if tokens.account_id.as_deref() == Some(workspace_id) {
            return Ok(WorkspaceSwitch {
                storage: auth.storage().clone(),
                previous: snapshot.clone(),
                selected: snapshot,
            });
        }
        let access_token = self
            .exchange_workspace_token(&tokens.access_token, workspace_id, user_id)
            .await?;
        ensure_auth_workspace_allowed(
            self.effective_chatgpt_workspaces().as_deref(),
            workspace_id,
        )?;
        if self.has_external_auth() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Authentication changed during workspace switching",
            ));
        }
        let mut updated = snapshot.clone();
        let tokens = updated
            .tokens
            .as_mut()
            .ok_or_else(|| io::Error::other("Authentication is unavailable"))?;
        tokens.access_token = access_token;
        tokens.account_id = Some(workspace_id.to_owned());
        // The exchange does not renew the session or issue refresh/ID tokens.
        updated.agent_identity = None;
        replace_if_current(auth.storage().as_ref(), &snapshot, &updated)?;
        self.reload().await;
        Ok(WorkspaceSwitch {
            storage: auth.storage().clone(),
            previous: snapshot,
            selected: updated,
        })
    }

    /// Restores a rejected workspace change only while its credentials are still current.
    pub async fn rollback_workspace_switch(&self, switched: WorkspaceSwitch) -> io::Result<()> {
        let _guard = self
            .refresh_lock
            .acquire()
            .await
            .map_err(io::Error::other)?;
        if self.has_external_auth()
            || switched.storage.load_account_grant()?.as_ref() != Some(&switched.selected)
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Account credentials changed",
            ));
        }
        let selected = switched
            .selected
            .tokens
            .as_ref()
            .ok_or_else(|| io::Error::other("Missing selected credentials"))?;
        let previous_workspace = switched
            .previous
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.account_id.as_deref())
            .ok_or_else(|| io::Error::other("Missing previous workspace"))?;
        let user = selected
            .id_token
            .chatgpt_user_id
            .as_deref()
            .ok_or_else(|| io::Error::other("Missing selected user"))?;
        // The exchange also selects the workspace used by the server on the next refresh.
        self.exchange_workspace_token(&selected.access_token, previous_workspace, user)
            .await?;
        let result = replace_if_current(
            switched.storage.as_ref(),
            &switched.selected,
            &switched.previous,
        );
        self.reload().await;
        result
    }

    pub(super) async fn exchange_workspace_token(
        &self,
        access_token: &str,
        workspace_id: &str,
        user_id: &str,
    ) -> io::Result<String> {
        let base_url = self
            .chatgpt_base_url
            .as_deref()
            .unwrap_or(ChatGptEnvironment::default().chatgpt_base_url())
            .trim_end_matches('/');
        let endpoint = format!("{base_url}/accounts/switch-workspace-token");
        let client = RouteAwareClientPool::with_chatgpt_cloudflare_cookies_without_redirects_or_request_logging(
                self.auth_route_config.http_client_factory().clone(), ClientRouteClass::Auth,
            );
        let response = client
            .post(&endpoint)
            .headers(crate::default_client::default_headers())
            .header(
                http::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .json(&json!({ "workspace_id": workspace_id }))
            .timeout(Duration::from_secs(/*secs*/ 30))
            .send()
            .await
            .map_err(io::Error::other)?;
        if !response.status().is_success() {
            let kind = if response.status().is_client_error() {
                io::ErrorKind::PermissionDenied
            } else {
                io::ErrorKind::Other
            };
            return Err(io::Error::new(
                kind,
                format!("Workspace switch was rejected (HTTP {})", response.status()),
            ));
        }
        let response: WorkspaceTokenResponse = response.json().await.map_err(io::Error::other)?;
        let claims = parse_chatgpt_jwt_claims(&response.access_token).map_err(io::Error::other)?;
        if claims.chatgpt_account_id.as_deref() != Some(workspace_id)
            || claims.chatgpt_user_id.as_deref() != Some(user_id)
        {
            return Err(io::Error::other(
                "Workspace exchange returned a different user or workspace",
            ));
        }
        Ok(response.access_token)
    }
}

// Older credentials predate login_owner_id. Keep them usable without migrating the store.
fn managed_session_id(tokens: &TokenData) -> Option<&str> {
    let user_id = tokens
        .id_token
        .chatgpt_user_id
        .as_deref()
        .filter(|id| !id.is_empty())?;
    Some(
        tokens
            .login_owner_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .unwrap_or(user_id),
    )
}

fn replace_if_current(
    storage: &dyn AuthStorageBackend,
    expected: &AuthDotJson,
    updated: &AuthDotJson,
) -> io::Result<()> {
    let _commit = storage.lock_for_write()?;
    if storage.load_account_grant()?.as_ref() != Some(expected) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Account credentials changed",
        ));
    }
    storage.save(updated)
}

/// The retained ID token can still describe the previous workspace until refresh.
pub(super) fn selected_workspace_tokens(mut tokens: TokenData) -> TokenData {
    if tokens.account_id != tokens.id_token.chatgpt_account_id
        && let Ok(claims) = parse_chatgpt_jwt_claims(&tokens.access_token)
        && claims.chatgpt_account_id == tokens.account_id
        && claims.chatgpt_user_id == tokens.id_token.chatgpt_user_id
    {
        tokens.id_token.chatgpt_account_id = claims.chatgpt_account_id;
        tokens.id_token.chatgpt_plan_type = claims.chatgpt_plan_type;
        tokens.id_token.chatgpt_account_is_fedramp = claims.chatgpt_account_is_fedramp;
    }
    tokens
}

#[cfg(test)]
#[path = "workspace_switching_tests.rs"]
mod tests;
