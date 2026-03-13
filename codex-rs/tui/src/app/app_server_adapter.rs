use super::App;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessServerEvent;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::ServerRequest;
use codex_core::AuthManager;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_protocol::account::PlanType as AccountPlanType;
use std::path::PathBuf;

impl App {
    pub(super) async fn handle_embedded_app_server_event(
        &mut self,
        app_server_client: &InProcessAppServerClient,
        event: InProcessServerEvent,
    ) {
        match event {
            InProcessServerEvent::Lagged { skipped } => {
                tracing::warn!(
                    skipped,
                    "embedded app-server event consumer lagged; dropping ignored events"
                );
            }
            InProcessServerEvent::ServerNotification(notification) => {
                self.handle_ignored_embedded_app_server_notification(notification);
            }
            InProcessServerEvent::LegacyNotification(notification) => {
                self.handle_ignored_embedded_app_server_legacy_notification(notification);
            }
            InProcessServerEvent::ServerRequest(request) => {
                self.handle_embedded_app_server_request(app_server_client, request)
                    .await;
            }
        }
    }

    async fn handle_embedded_app_server_request(
        &mut self,
        app_server_client: &InProcessAppServerClient,
        request: ServerRequest,
    ) {
        let method = Self::server_request_method_name(&request);
        match request {
            ServerRequest::ChatgptAuthTokensRefresh { request_id, params } => {
                let refresh_result = tokio::task::spawn_blocking({
                    let codex_home = self.config.codex_home.clone();
                    let auth_credentials_store_mode = self.config.cli_auth_credentials_store_mode;
                    let forced_chatgpt_workspace_id =
                        self.config.forced_chatgpt_workspace_id.clone();
                    move || {
                        Self::local_external_chatgpt_tokens(
                            codex_home,
                            auth_credentials_store_mode,
                            forced_chatgpt_workspace_id,
                        )
                    }
                })
                .await;

                let handle_result = match refresh_result {
                    Err(err) => {
                        self.reject_embedded_app_server_request(
                            app_server_client,
                            request_id,
                            &method,
                            format!("local chatgpt auth refresh task failed in TUI: {err}"),
                        )
                        .await
                    }
                    Ok(Err(reason)) => {
                        self.reject_embedded_app_server_request(
                            app_server_client,
                            request_id,
                            &method,
                            reason,
                        )
                        .await
                    }
                    Ok(Ok(response)) => {
                        if let Some(previous_account_id) = params.previous_account_id.as_deref()
                            && previous_account_id != response.chatgpt_account_id
                        {
                            tracing::warn!(
                                "local auth refresh account mismatch: expected `{previous_account_id}`, got `{}`",
                                response.chatgpt_account_id
                            );
                        }
                        match serde_json::to_value(response) {
                            Ok(value) => {
                                self.resolve_embedded_app_server_request(
                                    app_server_client,
                                    request_id,
                                    value,
                                    "account/chatgptAuthTokens/refresh",
                                )
                                .await
                            }
                            Err(err) => Err(format!(
                                "failed to serialize chatgpt auth refresh response: {err}"
                            )),
                        }
                    }
                };
                if let Err(err) = handle_result {
                    tracing::warn!("{err}");
                }
            }
            request => {
                let request_id = request.id().clone();
                tracing::warn!(
                    ?request_id,
                    method,
                    "rejecting embedded app-server request while TUI still uses direct core APIs"
                );
                if let Err(err) = self
                    .reject_embedded_app_server_request(
                        app_server_client,
                        request_id,
                        &method,
                        "embedded TUI client does not yet handle this app-server server request"
                            .to_string(),
                    )
                    .await
                {
                    tracing::warn!("{err}");
                }
            }
        }
    }

    async fn resolve_embedded_app_server_request(
        &self,
        app_server_client: &InProcessAppServerClient,
        request_id: codex_app_server_protocol::RequestId,
        value: serde_json::Value,
        method: &str,
    ) -> std::result::Result<(), String> {
        app_server_client
            .resolve_server_request(request_id, value)
            .await
            .map_err(|err| format!("failed to resolve `{method}` server request: {err}"))
    }

    async fn reject_embedded_app_server_request(
        &self,
        app_server_client: &InProcessAppServerClient,
        request_id: codex_app_server_protocol::RequestId,
        method: &str,
        reason: String,
    ) -> std::result::Result<(), String> {
        app_server_client
            .reject_server_request(
                request_id,
                JSONRPCErrorError {
                    code: -32000,
                    message: reason,
                    data: None,
                },
            )
            .await
            .map_err(|err| format!("failed to reject `{method}` server request: {err}"))
    }

    fn handle_ignored_embedded_app_server_notification(
        &mut self,
        _notification: codex_app_server_protocol::ServerNotification,
    ) {
    }

    fn handle_ignored_embedded_app_server_legacy_notification(
        &mut self,
        _notification: codex_app_server_protocol::JSONRPCNotification,
    ) {
    }

    fn server_request_method_name(request: &ServerRequest) -> String {
        serde_json::to_value(request)
            .ok()
            .and_then(|value| {
                value
                    .get("method")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn local_external_chatgpt_tokens(
        codex_home: PathBuf,
        auth_credentials_store_mode: AuthCredentialsStoreMode,
        forced_chatgpt_workspace_id: Option<String>,
    ) -> Result<ChatgptAuthTokensRefreshResponse, String> {
        let auth_manager = AuthManager::shared(codex_home, false, auth_credentials_store_mode);
        auth_manager.set_forced_chatgpt_workspace_id(forced_chatgpt_workspace_id);
        auth_manager.reload();

        let auth = auth_manager
            .auth_cached()
            .ok_or_else(|| "no cached auth available for local token refresh".to_string())?;
        if !auth.is_external_chatgpt_tokens() {
            return Err("external ChatGPT token auth is not active".to_string());
        }

        let access_token = auth
            .get_token()
            .map_err(|err| format!("failed to read external access token: {err}"))?;
        let chatgpt_account_id = auth
            .get_account_id()
            .ok_or_else(|| "external token auth is missing chatgpt account id".to_string())?;
        let chatgpt_plan_type = auth.account_plan_type().map(|plan_type| match plan_type {
            AccountPlanType::Free => "free".to_string(),
            AccountPlanType::Go => "go".to_string(),
            AccountPlanType::Plus => "plus".to_string(),
            AccountPlanType::Pro => "pro".to_string(),
            AccountPlanType::Team => "team".to_string(),
            AccountPlanType::Business => "business".to_string(),
            AccountPlanType::Enterprise => "enterprise".to_string(),
            AccountPlanType::Edu => "edu".to_string(),
            AccountPlanType::Unknown => "unknown".to_string(),
        });

        Ok(ChatgptAuthTokensRefreshResponse {
            access_token,
            chatgpt_account_id,
            chatgpt_plan_type,
        })
    }
}
