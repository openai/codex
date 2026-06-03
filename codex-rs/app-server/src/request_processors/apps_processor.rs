use super::*;
use codex_http_state::HttpStateContext;

pub(crate) struct AppsRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
    shutdown_token: CancellationToken,
    _shutdown_drop_guard: DropGuard,
}

impl AppsRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
        workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
        shutdown_token: CancellationToken,
    ) -> Self {
        let shutdown_drop_guard = shutdown_token.clone().drop_guard();
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            config_manager,
            workspace_settings_cache,
            shutdown_token,
            _shutdown_drop_guard: shutdown_drop_guard,
        }
    }

    pub(crate) async fn apps_list(
        &self,
        request_id: &ConnectionRequestId,
        params: AppsListParams,
        app_server_client_name: Option<&str>,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.apps_list_inner(request_id, params, app_server_client_name)
            .await
            .map(|response| response.map(Into::into))
    }

    async fn apps_list_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: AppsListParams,
        app_server_client_name: Option<&str>,
    ) -> Result<Option<AppsListResponse>, JSONRPCErrorError> {
        let thread = if let Some(thread_id) = params.thread_id.as_deref() {
            let (_, loaded_thread) = self.load_thread(thread_id).await?;
            Some(loaded_thread)
        } else {
            None
        };
        let fallback_cwd = match thread.as_ref() {
            Some(thread) => Some(thread.config_snapshot().await.cwd.to_path_buf()),
            None => None,
        };
        let mut config = self.load_latest_config(fallback_cwd).await?;

        if let Some(thread) = thread {
            let _ = config
                .features
                .set_enabled(Feature::Apps, thread.enabled(Feature::Apps));
        }

        let auth = self
            .auth_manager
            .auth_for_surface(http_state_surface(app_server_client_name))
            .await;
        let http_state = http_state_context(&config, app_server_client_name);
        if !config
            .features
            .apps_enabled_for_auth(auth.as_ref().is_some_and(CodexAuth::uses_codex_backend))
        {
            return Ok(Some(AppsListResponse {
                data: Vec::new(),
                next_cursor: None,
            }));
        }

        if !self
            .workspace_codex_plugins_enabled(&config, auth.as_ref(), &http_state)
            .await
        {
            return Ok(Some(AppsListResponse {
                data: Vec::new(),
                next_cursor: None,
            }));
        }

        let request = request_id.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let environment_manager = self.thread_manager.environment_manager();
        let shutdown_token = self.shutdown_token.child_token();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_token.cancelled() => {}
                _ = Self::apps_list_task(outgoing, request, params, config, environment_manager, http_state) => {}
            }
        });
        Ok(None)
    }

    pub(crate) fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    async fn apps_list_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: AppsListParams,
        config: Config,
        environment_manager: Arc<EnvironmentManager>,
        http_state: HttpStateContext,
    ) {
        let retry_params = params.clone();
        let retry_config = config.clone();
        let retry_environment_manager = Arc::clone(&environment_manager);
        let retry_http_state = http_state.clone();
        let result =
            Self::apps_list_response(&outgoing, params, config, environment_manager, http_state)
                .await;
        let should_retry = result
            .as_ref()
            .is_ok_and(|(_, codex_apps_ready)| !codex_apps_ready);
        outgoing
            .send_result(request_id, result.map(|(response, _)| response))
            .await;

        if should_retry && !retry_params.force_refetch {
            let mut retry_params = retry_params;
            retry_params.force_refetch = true;
            if let Err(err) = Self::apps_list_response(
                &outgoing,
                retry_params,
                retry_config,
                retry_environment_manager,
                retry_http_state,
            )
            .await
            {
                warn!("failed to refresh app list after codex-apps readiness retry: {err:?}");
            }
        }
    }

    async fn apps_list_response(
        outgoing: &Arc<OutgoingMessageSender>,
        params: AppsListParams,
        config: Config,
        environment_manager: Arc<EnvironmentManager>,
        http_state: HttpStateContext,
    ) -> Result<(AppsListResponse, bool), JSONRPCErrorError> {
        let AppsListParams {
            cursor,
            limit,
            thread_id: _,
            force_refetch,
        } = params;
        let start = match cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => return Err(invalid_request(format!("invalid cursor: {cursor}"))),
            },
            None => 0,
        };

        let (mut accessible_connectors, mut all_connectors) = tokio::join!(
            connectors::list_cached_accessible_connectors_from_mcp_tools_with_http_state(
                &config,
                Some(&http_state),
            ),
            connectors::list_cached_all_connectors_with_http_state(&config, Some(&http_state))
        );
        let cached_all_connectors = all_connectors.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let accessible_config = config.clone();
        let accessible_tx = tx.clone();
        let accessible_http_state = http_state.clone();
        tokio::spawn(async move {
            let result =
                connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager_and_http_state(
                    &accessible_config,
                    force_refetch,
                    Arc::clone(&environment_manager),
                    Some(accessible_http_state),
                )
                .await
                .map_err(|err| format!("failed to load accessible apps: {err}"));
            let _ = accessible_tx.send(AppListLoadResult::Accessible(result));
        });

        let all_config = config.clone();
        tokio::spawn(async move {
            let result = connectors::list_all_connectors_with_options_and_http_state(
                &all_config,
                force_refetch,
                Some(http_state),
            )
            .await
            .map_err(|err| format!("failed to list apps: {err}"));
            let _ = tx.send(AppListLoadResult::Directory(result));
        });

        let app_list_deadline = tokio::time::Instant::now() + APP_LIST_LOAD_TIMEOUT;
        let mut accessible_loaded = false;
        let mut all_loaded = false;
        let mut codex_apps_ready = true;
        let mut last_notified_apps = None;

        if accessible_connectors.is_some() || all_connectors.is_some() {
            let merged = connectors::with_app_enabled_state(
                merge_loaded_apps(all_connectors.as_deref(), accessible_connectors.as_deref()),
                &config,
            );
            if should_send_app_list_updated_notification(
                merged.as_slice(),
                accessible_loaded,
                all_loaded,
            ) {
                send_app_list_updated_notification(outgoing, merged.clone()).await;
                last_notified_apps = Some(merged);
            }
        }

        loop {
            let result = match tokio::time::timeout_at(app_list_deadline, rx.recv()).await {
                Ok(Some(result)) => result,
                Ok(None) => {
                    return Err(internal_error("failed to load app lists"));
                }
                Err(_) => {
                    let timeout_seconds = APP_LIST_LOAD_TIMEOUT.as_secs();
                    return Err(internal_error(format!(
                        "timed out waiting for app lists after {timeout_seconds} seconds"
                    )));
                }
            };

            match result {
                AppListLoadResult::Accessible(Ok(status)) => {
                    accessible_connectors = Some(status.connectors);
                    accessible_loaded = true;
                    codex_apps_ready = status.codex_apps_ready;
                }
                AppListLoadResult::Accessible(Err(err)) => {
                    return Err(internal_error(err));
                }
                AppListLoadResult::Directory(Ok(connectors)) => {
                    all_connectors = Some(connectors);
                    all_loaded = true;
                }
                AppListLoadResult::Directory(Err(err)) => {
                    return Err(internal_error(err));
                }
            }

            let showing_interim_force_refetch = force_refetch && !(accessible_loaded && all_loaded);
            let all_connectors_for_update =
                if showing_interim_force_refetch && cached_all_connectors.is_some() {
                    cached_all_connectors.as_deref()
                } else {
                    all_connectors.as_deref()
                };
            let accessible_connectors_for_update =
                if showing_interim_force_refetch && !accessible_loaded {
                    None
                } else {
                    accessible_connectors.as_deref()
                };
            let merged = connectors::with_app_enabled_state(
                merge_loaded_apps(all_connectors_for_update, accessible_connectors_for_update),
                &config,
            );
            if should_send_app_list_updated_notification(
                merged.as_slice(),
                accessible_loaded,
                all_loaded,
            ) && last_notified_apps.as_ref() != Some(&merged)
            {
                send_app_list_updated_notification(outgoing, merged.clone()).await;
                last_notified_apps = Some(merged.clone());
            }

            if accessible_loaded && all_loaded {
                let response = paginate_apps(merged.as_slice(), start, limit)?;
                return Ok((response, codex_apps_ready));
            }
        }
    }

    async fn load_thread(
        &self,
        thread_id: &str,
    ) -> Result<(ThreadId, Arc<CodexThread>), JSONRPCErrorError> {
        let thread_id = ThreadId::from_string(thread_id)
            .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;

        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;

        Ok((thread_id, thread))
    }

    async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> Result<Config, JSONRPCErrorError> {
        self.config_manager
            .load_latest_config(fallback_cwd)
            .await
            .map_err(|err| internal_error(format!("failed to reload config: {err}")))
    }

    async fn workspace_codex_plugins_enabled(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        http_state: &HttpStateContext,
    ) -> bool {
        match workspace_settings::codex_plugins_enabled_for_workspace_with_http_state(
            config,
            auth,
            Some(&self.workspace_settings_cache),
            Some(http_state.clone()),
        )
        .await
        {
            Ok(enabled) => enabled,
            Err(err) => {
                warn!(
                    "failed to fetch workspace Codex plugins setting; allowing Codex plugins: {err:#}"
                );
                true
            }
        }
    }
}

const APP_LIST_LOAD_TIMEOUT: Duration = Duration::from_secs(90);

enum AppListLoadResult {
    Accessible(Result<AccessibleConnectorsStatus, String>),
    Directory(Result<Vec<AppInfo>, String>),
}

fn merge_loaded_apps(
    all_connectors: Option<&[AppInfo]>,
    accessible_connectors: Option<&[AppInfo]>,
) -> Vec<AppInfo> {
    let all_connectors_loaded = all_connectors.is_some();
    let all = all_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
    let accessible = accessible_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec);
    connectors::merge_connectors_with_accessible(all, accessible, all_connectors_loaded)
}

fn should_send_app_list_updated_notification(
    connectors: &[AppInfo],
    accessible_loaded: bool,
    all_loaded: bool,
) -> bool {
    connectors.iter().any(|connector| connector.is_accessible) || (accessible_loaded && all_loaded)
}

fn paginate_apps(
    connectors: &[AppInfo],
    start: usize,
    limit: Option<u32>,
) -> Result<AppsListResponse, JSONRPCErrorError> {
    let total = connectors.len();
    if start > total {
        return Err(invalid_request(format!(
            "cursor {start} exceeds total apps {total}"
        )));
    }

    let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
    let end = start.saturating_add(effective_limit).min(total);
    let data = connectors[start..end].to_vec();
    let next_cursor = if end < total {
        Some(end.to_string())
    } else {
        None
    };

    Ok(AppsListResponse { data, next_cursor })
}

async fn send_app_list_updated_notification(
    outgoing: &Arc<OutgoingMessageSender>,
    data: Vec<AppInfo>,
) {
    outgoing
        .send_server_notification(ServerNotification::AppListUpdated(
            AppListUpdatedNotification { data },
        ))
        .await;
}
