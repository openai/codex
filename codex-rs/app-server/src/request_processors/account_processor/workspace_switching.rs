//! Lists the active managed login and switches its workspace through the account-session API.

use super::*;
use codex_app_server_protocol::AccountSession;
use codex_app_server_protocol::AccountSessionWorkspace;
use codex_app_server_protocol::AccountSessionWorkspaceKind;
use codex_app_server_protocol::AccountSessionsListParams;
use codex_app_server_protocol::AccountSessionsResponse;
use codex_app_server_protocol::AccountSessionsSwitchParams;
use codex_app_server_protocol::AccountSessionsSwitchResponse;

impl AccountRequestProcessor {
    pub(crate) async fn list_account_sessions(
        &self,
        params: AccountSessionsListParams,
    ) -> Result<AccountSessionsResponse, JSONRPCErrorError> {
        let auth = if params.refresh_workspace_metadata {
            self.auth_manager.auth().await
        } else {
            self.auth_manager.auth_cached()
        }
        .filter(|_| !self.auth_manager.has_external_auth());
        let session_id = auth
            .as_ref()
            .and_then(CodexAuth::managed_account_session_id);
        let (Some(auth), Some(session_id)) = (auth, session_id) else {
            return Ok(AccountSessionsResponse {
                active_session_id: None,
                sessions: vec![],
            });
        };
        let account_id = auth.get_account_id();
        let mut workspaces = account_id
            .iter()
            .map(|id| AccountSessionWorkspace {
                account_id: id.clone(),
                name: None,
                image_url: None,
                kind: None,
            })
            .collect();
        if params.refresh_workspace_metadata {
            let accounts = self.workspace_memberships(&auth).await?;
            workspaces = accounts
                .accounts
                .into_iter()
                .map(|account| AccountSessionWorkspace {
                    account_id: account.id,
                    name: account.name,
                    image_url: account.profile_picture_url,
                    kind: match account.structure.as_str() {
                        "personal" => Some(AccountSessionWorkspaceKind::Personal),
                        "workspace" => Some(AccountSessionWorkspaceKind::Workspace),
                        _ => None,
                    },
                })
                .collect::<Vec<_>>();
            workspaces.sort_by_key(|workspace| {
                accounts
                    .account_ordering
                    .iter()
                    .position(|id| id == &workspace.account_id)
                    .unwrap_or(usize::MAX)
            });
            let current = self.auth_manager.auth_cached();
            if self.auth_manager.has_external_auth()
                || current
                    .as_ref()
                    .and_then(CodexAuth::managed_account_session_id)
                    .as_deref()
                    != Some(&session_id)
                || current.and_then(|auth| auth.get_account_id()) != account_id
            {
                return Err(invalid_request(
                    "Authentication changed while listing account sessions",
                ));
            }
        }
        Ok(AccountSessionsResponse {
            active_session_id: Some(session_id.clone()),
            sessions: vec![AccountSession {
                session_id,
                email: auth.get_account_email(),
                user_id: auth.get_chatgpt_user_id(),
                display_name: None,
                image_url: None,
                last_used_at: chrono::Utc::now().timestamp(),
                is_active: true,
                selected_workspace_account_id: account_id,
                workspaces,
            }],
        })
    }

    async fn workspace_memberships(
        &self,
        auth: &CodexAuth,
    ) -> Result<codex_backend_client::AccountsCheckResponse, JSONRPCErrorError> {
        let client = BackendClient::from_auth(
            self.config.chatgpt_base_url.clone(),
            auth,
            self.config.http_client_factory(),
        );
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 30),
            client.get_accounts_check(),
        )
        .await
        .map_err(|_| internal_error("Workspace metadata request timed out"))?
        .map_err(|_| internal_error("Failed to fetch workspace metadata"))
    }

    pub(crate) async fn switch_account_session(
        &self,
        params: AccountSessionsSwitchParams,
        threads: &crate::request_processors::ThreadRequestProcessor,
        admission: &crate::turn_admission::TurnAdmission,
    ) -> Result<AccountSessionsSwitchResponse, JSONRPCErrorError> {
        if self.active_login.lock().await.is_some() {
            return Err(invalid_request(
                "Finish or cancel the current login before switching workspaces",
            ));
        }
        self.auth_manager
            .validate_workspace_switch(&params.session_id, &params.account_id)
            .map_err(|error| invalid_request(error.to_string()))?;
        if self
            .auth_manager
            .auth_cached()
            .and_then(|auth| auth.get_account_id())
            .as_deref()
            == Some(&params.account_id)
        {
            return self.account_session_switch_response(&params).await;
        }
        let auth = self
            .auth_manager
            .auth_cached()
            .ok_or_else(|| invalid_request("Authentication is unavailable"))?;
        let memberships = self.workspace_memberships(&auth).await?;
        let mut matches = memberships
            .accounts
            .iter()
            .filter(|account| account.id == params.account_id);
        let Some(target) = matches.next() else {
            return Err(invalid_request("The selected workspace is unavailable"));
        };
        if matches.next().is_some() || target.is_deactivated {
            return Err(invalid_request("The selected workspace is unavailable"));
        }
        // The exchange strips the FedRAMP hint from the server session's token templates.
        if auth.is_fedramp_account()
            || target.is_fedramp_compliant_workspace
            || target.workspace_backend_origin.as_deref() == Some("https://gov.chatgpt.com")
        {
            return Err(invalid_request("Sign in to switch government workspaces"));
        }
        self.auth_manager
            .validate_workspace_switch(&params.session_id, &params.account_id)
            .map_err(|error| invalid_request(error.to_string()))?;
        if target.can_access_with_session == Some(false) {
            if !params.allow_browser_login {
                return Err(invalid_request(
                    "The selected workspace requires a new sign-in",
                ));
            }
            let mut opts = self
                .login_chatgpt_common(
                    /*codex_streamlined_login*/ true,
                    LoginSuccessPage::default(),
                )
                .await?;
            opts.workspace_login = Some(
                auth.workspace_login(
                    &params.session_id,
                    params.account_id.clone(),
                    target.sso_connection_name.clone(),
                )
                .map_err(|error| invalid_request(error.to_string()))?,
            );
            opts.forced_chatgpt_workspace_id = Some(vec![params.account_id]);
            let pause = admission.pause().await?;
            threads.unload_for_workspace_switch().await?;
            let (login_id, auth_url) = self.start_browser_login(opts, Some(pause)).await?;
            return Ok(AccountSessionsSwitchResponse::BrowserLogin { login_id, auth_url });
        }
        let _pause = admission.pause().await?;
        threads.unload_for_workspace_switch().await?;
        let switched = self
            .auth_manager
            .switch_workspace(&params.session_id, &params.account_id)
            .await
            .map_err(|error| invalid_request(error.to_string()))?;
        let auth_changes = self.auth_manager.auth_change_state_receiver();
        let owner_generation = auth_changes.borrow().owner_generation;
        self.reload_workspace_config().await;
        // Routing includes fatal authorization and policy checks. Restore the
        // previous login on any failure, unless another auth operation replaced it.
        if let Err(error) = self.read_account(/*request*/ None).await {
            let rollback = self.auth_manager.rollback_workspace_switch(switched).await;
            self.reload_workspace_config().await;
            rollback.map_err(|rollback| internal_error(format!("Workspace validation failed: {error}; could not restore the previous workspace: {rollback}")))?;
            return Err(internal_error(format!(
                "Workspace validation failed: {error}"
            )));
        }
        self.thread_manager.invalidate_mcp_runtimes().await;
        self.model_catalog.invalidate().await;
        Self::maybe_refresh_plugin_caches_for_current_config(
            &self.config_manager,
            &self.thread_manager,
            self.auth_manager.auth_cached(),
        )
        .await;
        self.outgoing
            .send_account_notification(
                /*connection_id*/ None,
                &auth_changes,
                owner_generation,
                AccountNotification::Updated(self.current_account_updated_notification()),
            )
            .await;
        self.account_session_switch_response(&params).await
    }

    async fn reload_workspace_config(&self) {
        self.config_manager
            .update_managed_config_loader(self.auth_manager.clone(), &self.config);
        self.config_manager
            .sync_default_client_residency_requirement()
            .await;
    }

    async fn account_session_switch_response(
        &self,
        params: &AccountSessionsSwitchParams,
    ) -> Result<AccountSessionsSwitchResponse, JSONRPCErrorError> {
        let response = self
            .list_account_sessions(AccountSessionsListParams {
                refresh_workspace_metadata: false,
            })
            .await?;
        if response.active_session_id.as_deref() != Some(&params.session_id)
            || !response.sessions.iter().any(|session| {
                session.session_id == params.session_id
                    && session.selected_workspace_account_id.as_deref() == Some(&params.account_id)
            })
        {
            return Err(invalid_request(
                "Authentication changed during workspace switching",
            ));
        }
        Ok(AccountSessionsSwitchResponse::Switched { sessions: response })
    }
}
