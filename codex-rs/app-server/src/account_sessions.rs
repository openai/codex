use base64::Engine;
use chrono::Utc;
use codex_app_server_protocol::AccountSession;
use codex_app_server_protocol::AccountSessionWorkspace;
use codex_app_server_protocol::AccountSessionWorkspaceKind;
use codex_app_server_protocol::AccountSessionWorkspaceStatus;
use codex_app_server_protocol::AccountSessionsResponse;
use codex_backend_client::AccountEntry;
use codex_backend_client::Client as BackendClient;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthDotJson;
use codex_login::CodexAuth;
use codex_login::delete_account_session_auth;
use codex_login::load_account_session_auth;
use codex_login::load_auth_dot_json;
use codex_login::logout;
use codex_login::revoke_account_session_auth;
use codex_login::save_account_session_auth;
use codex_login::save_auth;
use codex_login::token_data::parse_chatgpt_jwt_claims;
use serde::Deserialize;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

const ACCOUNT_SESSIONS_FILE: &str = "account-sessions.json";

pub(crate) struct AccountSessionsStore<'a> {
    codex_home: &'a Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    chatgpt_base_url: &'a str,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredAccountSessions {
    active_session_id: Option<String>,
    sessions: Vec<StoredAccountSession>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredAccountSession {
    session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth_json: Option<AuthDotJson>,
    email: Option<String>,
    user_id: Option<String>,
    display_name: Option<String>,
    image_url: Option<String>,
    plan: Option<String>,
    last_used_at: i64,
    selected_workspace_account_id: Option<String>,
    workspaces: Vec<AccountSessionWorkspace>,
}

#[derive(Default, Deserialize)]
struct AccessTokenClaims {
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: AccessTokenProfileClaims,
}

#[derive(Default, Deserialize)]
struct AccessTokenProfileClaims {
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    picture: Option<String>,
}

impl<'a> AccountSessionsStore<'a> {
    pub(crate) fn new(
        codex_home: &'a Path,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        chatgpt_base_url: &'a str,
    ) -> Self {
        Self {
            codex_home,
            auth_credentials_store_mode,
            chatgpt_base_url,
        }
    }

    pub(crate) async fn add(
        &self,
        switch_to_added_account: bool,
    ) -> std::io::Result<AccountSessionsResponse> {
        let mut stored = self.load()?;
        let mut auth_json = load_auth_dot_json(self.codex_home, self.auth_credentials_store_mode)?
            .ok_or_else(|| std::io::Error::other("No active ChatGPT auth session to add"))?;
        let mut session = Self::session_from_auth_json(&auth_json)
            .ok_or_else(|| std::io::Error::other("No active ChatGPT auth session to add"))?;
        self.refresh_workspace_metadata(&mut session, &mut auth_json)
            .await;

        let existing_index = stored.sessions.iter().position(|saved| {
            session
                .email
                .as_ref()
                .is_some_and(|email| saved.email.as_ref() == Some(email))
                || session
                    .user_id
                    .as_ref()
                    .is_some_and(|user_id| saved.user_id.as_ref() == Some(user_id))
        });
        if let Some(index) = existing_index {
            session
                .session_id
                .clone_from(&stored.sessions[index].session_id);
        }
        let added_session_id = session.session_id.clone();
        self.save_session_auth(&added_session_id, &auth_json)?;
        if let Some(index) = existing_index {
            stored.sessions[index] = session;
        } else {
            stored.sessions.push(session);
        }

        if switch_to_added_account || stored.active_session_id.is_none() {
            stored.active_session_id = Some(added_session_id);
            save_auth(
                self.codex_home,
                &auth_json,
                self.auth_credentials_store_mode,
            )?;
        } else if let Some(active_session_id) = stored.active_session_id.as_deref() {
            self.save_active_session_auth(active_session_id)?;
        }

        self.save(&stored)?;
        Ok(Self::response(stored))
    }

    pub(crate) async fn list(
        &self,
        refresh_workspace_metadata: bool,
    ) -> std::io::Result<AccountSessionsResponse> {
        self.sync_active_auth()?;
        let mut stored = self.load()?;
        if refresh_workspace_metadata {
            for session in &mut stored.sessions {
                let Some(mut auth_json) = self.load_session_auth(&session.session_id)? else {
                    continue;
                };
                self.refresh_workspace_metadata(session, &mut auth_json)
                    .await;
                self.save_session_auth(&session.session_id, &auth_json)?;
            }
            self.save(&stored)?;
        }
        Ok(Self::response(stored))
    }

    pub(crate) async fn logout(
        &self,
        session_id: &str,
    ) -> std::io::Result<AccountSessionsResponse> {
        self.sync_active_auth()?;
        let mut stored = self.load()?;
        let index = stored
            .sessions
            .iter()
            .position(|session| session.session_id == session_id)
            .ok_or_else(|| std::io::Error::other("Saved ChatGPT account session not found"))?;
        let removed = stored.sessions.remove(index);
        if let Err(err) = revoke_account_session_auth(
            self.codex_home,
            &removed.session_id,
            self.auth_credentials_store_mode,
        )
        .await
        {
            tracing::warn!("failed to revoke saved account session during logout: {err}");
        }
        delete_account_session_auth(
            self.codex_home,
            &removed.session_id,
            self.auth_credentials_store_mode,
        )?;

        if stored.active_session_id.as_deref() == Some(session_id) {
            let newest = stored
                .sessions
                .iter()
                .max_by_key(|session| session.last_used_at);
            stored.active_session_id = newest.map(|session| session.session_id.clone());
            match newest {
                Some(session) => self.save_active_session_auth(&session.session_id)?,
                None => {
                    logout(self.codex_home, self.auth_credentials_store_mode)?;
                }
            }
        }

        self.save(&stored)?;
        Ok(Self::response(stored))
    }

    pub(crate) async fn switch(
        &self,
        session_id: &str,
        account_id: &str,
    ) -> std::io::Result<AccountSessionsResponse> {
        self.sync_active_auth()?;
        let mut stored = self.load()?;
        let index = stored
            .sessions
            .iter()
            .position(|session| session.session_id == session_id)
            .ok_or_else(|| std::io::Error::other("Saved ChatGPT account session not found"))?;
        let mut auth_json = self
            .load_session_auth(&stored.sessions[index].session_id)?
            .ok_or_else(|| std::io::Error::other("Saved ChatGPT account session has no tokens"))?;
        let auth = CodexAuth::from_account_session_auth_dot_json(
            self.codex_home,
            &stored.sessions[index].session_id,
            auth_json.clone(),
            self.auth_credentials_store_mode,
            Some(self.chatgpt_base_url),
        )
        .await?;
        let client = BackendClient::from_auth(self.chatgpt_base_url, &auth)
            .map_err(std::io::Error::other)?;
        let replacement = client
            .switch_workspace_token(account_id)
            .await
            .map_err(std::io::Error::other)?;
        let plan = parse_chatgpt_jwt_claims(&replacement.access_token)
            .ok()
            .and_then(|claims| claims.get_chatgpt_plan_type_raw());
        let tokens = auth_json
            .tokens
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Saved ChatGPT account session has no tokens"))?;
        tokens.access_token = replacement.access_token;
        if let Some(refresh_token) = replacement.refresh_token {
            tokens.refresh_token = refresh_token;
        }
        tokens.account_id = Some(account_id.to_string());
        auth_json.last_refresh = Some(Utc::now());
        let session = &mut stored.sessions[index];
        if let Some(plan) = plan {
            session.plan = Some(plan);
        }
        session.selected_workspace_account_id = Some(account_id.to_string());
        session.last_used_at = Utc::now().timestamp();
        stored.active_session_id = Some(session_id.to_string());
        self.save_session_auth(session_id, &auth_json)?;
        save_auth(
            self.codex_home,
            &auth_json,
            self.auth_credentials_store_mode,
        )?;
        self.save(&stored)?;
        Ok(Self::response(stored))
    }

    pub(crate) fn sync_active_auth(&self) -> std::io::Result<()> {
        let mut stored = self.load()?;
        if self.migrate_legacy_auth(&mut stored)? {
            self.save(&stored)?;
        }
        let Some(active_session_id) = stored.active_session_id.as_ref() else {
            return Ok(());
        };
        let Some(auth_json) =
            load_auth_dot_json(self.codex_home, self.auth_credentials_store_mode)?
        else {
            return Ok(());
        };
        let Some(session) = stored
            .sessions
            .iter_mut()
            .find(|session| &session.session_id == active_session_id)
        else {
            return Ok(());
        };
        if !Self::same_identity(session, &auth_json) {
            return Ok(());
        }

        let saved_auth_json = self.load_session_auth(&session.session_id)?;
        let selected_workspace_account_id = auth_json
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.account_id.clone());
        if saved_auth_json.as_ref() == Some(&auth_json)
            && (selected_workspace_account_id.is_none()
                || session.selected_workspace_account_id == selected_workspace_account_id)
        {
            return Ok(());
        }

        self.save_session_auth(&session.session_id, &auth_json)?;
        if selected_workspace_account_id.is_some() {
            session.selected_workspace_account_id = selected_workspace_account_id;
        }
        self.save(&stored)
    }

    pub(crate) async fn revoke_all_and_clear(&self) -> std::io::Result<()> {
        let Some(mut stored) = self.read()? else {
            return self.clear();
        };
        self.migrate_legacy_auth(&mut stored)?;
        for session in stored.sessions {
            if let Err(err) = revoke_account_session_auth(
                self.codex_home,
                &session.session_id,
                self.auth_credentials_store_mode,
            )
            .await
            {
                tracing::warn!("failed to revoke saved account session during logout: {err}");
            }
            delete_account_session_auth(
                self.codex_home,
                &session.session_id,
                self.auth_credentials_store_mode,
            )?;
        }
        self.clear()
    }

    fn load(&self) -> std::io::Result<StoredAccountSessions> {
        match self.read()? {
            Some(mut stored) => {
                if self.migrate_legacy_auth(&mut stored)? {
                    self.save(&stored)?;
                }
                Ok(stored)
            }
            None => {
                let auth_json =
                    load_auth_dot_json(self.codex_home, self.auth_credentials_store_mode)?;
                let Some(auth_json) = auth_json else {
                    return Ok(StoredAccountSessions::default());
                };
                let Some(session) = Self::session_from_auth_json(&auth_json) else {
                    return Ok(StoredAccountSessions::default());
                };
                self.save_session_auth(&session.session_id, &auth_json)?;
                let stored = StoredAccountSessions {
                    active_session_id: Some(session.session_id.clone()),
                    sessions: vec![session],
                };
                self.save(&stored)?;
                Ok(stored)
            }
        }
    }

    fn migrate_legacy_auth(&self, stored: &mut StoredAccountSessions) -> std::io::Result<bool> {
        let mut migrated = false;
        for session in &mut stored.sessions {
            if let Some(auth_json) = session.auth_json.take() {
                self.save_session_auth(&session.session_id, &auth_json)?;
                migrated = true;
            }
        }
        Ok(migrated)
    }

    fn load_session_auth(&self, session_id: &str) -> std::io::Result<Option<AuthDotJson>> {
        load_account_session_auth(
            self.codex_home,
            session_id,
            self.auth_credentials_store_mode,
        )
    }

    fn save_session_auth(&self, session_id: &str, auth_json: &AuthDotJson) -> std::io::Result<()> {
        save_account_session_auth(
            self.codex_home,
            session_id,
            auth_json,
            self.auth_credentials_store_mode,
        )
    }

    fn save_active_session_auth(&self, session_id: &str) -> std::io::Result<()> {
        let auth_json = self
            .load_session_auth(session_id)?
            .ok_or_else(|| std::io::Error::other("Saved ChatGPT account session has no tokens"))?;
        save_auth(
            self.codex_home,
            &auth_json,
            self.auth_credentials_store_mode,
        )
    }

    fn clear(&self) -> std::io::Result<()> {
        match std::fs::remove_file(self.path()) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn read(&self) -> std::io::Result<Option<StoredAccountSessions>> {
        match std::fs::read_to_string(self.path()) {
            Ok(payload) => serde_json::from_str(&payload)
                .map(Some)
                .map_err(std::io::Error::other),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn save(&self, sessions: &StoredAccountSessions) -> std::io::Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(serde_json::to_string_pretty(sessions)?.as_bytes())?;
        file.flush()
    }

    async fn refresh_workspace_metadata(
        &self,
        session: &mut StoredAccountSession,
        auth_json: &mut AuthDotJson,
    ) {
        let Ok(auth) = CodexAuth::from_account_session_auth_dot_json(
            self.codex_home,
            &session.session_id,
            auth_json.clone(),
            self.auth_credentials_store_mode,
            Some(self.chatgpt_base_url),
        )
        .await
        else {
            return;
        };
        let Ok(client) = BackendClient::from_auth(self.chatgpt_base_url, &auth) else {
            return;
        };
        let Ok(accounts) = client.get_accounts_check().await else {
            return;
        };
        session.selected_workspace_account_id = session
            .selected_workspace_account_id
            .clone()
            .or(accounts.default_account_id)
            .or_else(|| accounts.account_ordering.first().cloned());
        if let Some(account_id) = session.selected_workspace_account_id.as_ref()
            && let Some(tokens) = auth_json.tokens.as_mut()
        {
            tokens.account_id = Some(account_id.clone());
        }
        session.workspaces = accounts
            .accounts
            .into_iter()
            .map(Self::workspace_from_account)
            .collect();
    }

    fn session_from_auth_json(auth_json: &AuthDotJson) -> Option<StoredAccountSession> {
        let tokens = auth_json.tokens.as_ref()?;
        let claims = Self::access_token_claims(&tokens.access_token);
        let selected_workspace_account_id = tokens
            .account_id
            .clone()
            .or_else(|| tokens.id_token.chatgpt_account_id.clone());
        let workspaces = selected_workspace_account_id
            .as_ref()
            .map(|account_id| {
                vec![AccountSessionWorkspace {
                    account_id: account_id.clone(),
                    name: None,
                    image_url: None,
                    kind: None,
                    status: AccountSessionWorkspaceStatus::Active,
                }]
            })
            .unwrap_or_default();
        Some(StoredAccountSession {
            session_id: Uuid::now_v7().to_string(),
            auth_json: None,
            email: tokens.id_token.email.clone(),
            user_id: tokens.id_token.chatgpt_user_id.clone(),
            display_name: claims.profile.name,
            image_url: claims.profile.picture.or(claims.profile.image),
            plan: tokens.id_token.get_chatgpt_plan_type_raw(),
            last_used_at: Utc::now().timestamp(),
            selected_workspace_account_id,
            workspaces,
        })
    }

    fn access_token_claims(access_token: &str) -> AccessTokenClaims {
        let Some(payload) = access_token.split('.').nth(1) else {
            return AccessTokenClaims::default();
        };
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .ok()
            .and_then(|payload| serde_json::from_slice(&payload).ok())
            .unwrap_or_default()
    }

    fn same_identity(session: &StoredAccountSession, auth_json: &AuthDotJson) -> bool {
        let Some(tokens) = auth_json.tokens.as_ref() else {
            return false;
        };
        session
            .email
            .as_ref()
            .zip(tokens.id_token.email.as_ref())
            .is_some_and(|(saved, active)| saved == active)
            || session
                .user_id
                .as_ref()
                .zip(tokens.id_token.chatgpt_user_id.as_ref())
                .is_some_and(|(saved, active)| saved == active)
    }

    fn workspace_from_account(account: AccountEntry) -> AccountSessionWorkspace {
        let kind = match account.structure.as_str() {
            "personal" => Some(AccountSessionWorkspaceKind::Personal),
            "workspace" => Some(AccountSessionWorkspaceKind::Workspace),
            _ => None,
        };
        AccountSessionWorkspace {
            account_id: account.id,
            name: account.name,
            image_url: account.profile_picture_url,
            kind,
            status: AccountSessionWorkspaceStatus::Active,
        }
    }

    fn response(stored: StoredAccountSessions) -> AccountSessionsResponse {
        let active_session_id = stored.active_session_id;
        let mut sessions = stored
            .sessions
            .into_iter()
            .map(|session| AccountSession {
                is_active: Some(&session.session_id) == active_session_id.as_ref(),
                session_id: session.session_id,
                email: session.email,
                user_id: session.user_id,
                display_name: session.display_name,
                image_url: session.image_url,
                plan: session.plan,
                last_used_at: session.last_used_at,
                selected_workspace_account_id: session.selected_workspace_account_id,
                workspaces: session.workspaces,
            })
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.last_used_at));
        AccountSessionsResponse {
            active_session_id,
            sessions,
        }
    }

    fn path(&self) -> PathBuf {
        self.codex_home.join(ACCOUNT_SESSIONS_FILE)
    }
}
