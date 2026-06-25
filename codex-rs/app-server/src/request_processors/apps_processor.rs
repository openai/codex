use super::*;
use crate::app_info::app_info_to_api;

pub(crate) struct AppsRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
    codex_apps: Arc<codex_mcp_extension::CodexAppsMcpExtension>,
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
        codex_apps: Arc<codex_mcp_extension::CodexAppsMcpExtension>,
        shutdown_token: CancellationToken,
    ) -> Self {
        let shutdown_drop_guard = shutdown_token.clone().drop_guard();
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            config_manager,
            workspace_settings_cache,
            codex_apps,
            shutdown_token,
            _shutdown_drop_guard: shutdown_drop_guard,
        }
    }

    pub(crate) async fn apps_list(
        &self,
        request_id: &ConnectionRequestId,
        params: AppsListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.apps_list_inner(request_id, params)
            .await
            .map(|response| response.map(Into::into))
    }

    async fn apps_list_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: AppsListParams,
    ) -> Result<Option<AppsListResponse>, JSONRPCErrorError> {
        let thread = if let Some(thread_id) = params.thread_id.as_deref() {
            let (_, loaded_thread) = self.load_thread(thread_id).await?;
            Some(loaded_thread)
        } else {
            None
        };
        let fallback_cwd = match thread.as_ref() {
            Some(thread) => Some(thread.config_snapshot().await.cwd().to_path_buf()),
            None => None,
        };
        let mut config = self.load_latest_config(fallback_cwd).await?;

        if let Some(thread) = thread {
            let _ = config
                .features
                .set_enabled(Feature::Apps, thread.enabled(Feature::Apps));
        }

        let auth = self.auth_manager.auth().await;
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
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await
        {
            return Ok(Some(AppsListResponse {
                data: Vec::new(),
                next_cursor: None,
            }));
        }

        let request = request_id.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let plugins_manager = self.thread_manager.plugins_manager();
        let codex_apps = Arc::clone(&self.codex_apps);
        let shutdown_token = self.shutdown_token.child_token();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_token.cancelled() => {}
                _ = Self::apps_list_task(
                    outgoing,
                    request,
                    params,
                    config,
                    plugins_manager,
                    codex_apps,
                ) => {}
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
        plugins_manager: Arc<PluginsManager>,
        codex_apps: Arc<codex_mcp_extension::CodexAppsMcpExtension>,
    ) {
        let retry_params = params.clone();
        let retry_config = config.clone();
        let retry_plugins_manager = Arc::clone(&plugins_manager);
        let retry_codex_apps = Arc::clone(&codex_apps);
        let result = Self::apps_list_response(
            &outgoing,
            params,
            config,
            plugins_manager,
            codex_apps,
            /*join_cached_apps_refresh*/ false,
        )
        .await;
        let should_retry = result
            .as_ref()
            .is_ok_and(|(_, live_inventory)| !live_inventory);
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
                retry_plugins_manager,
                retry_codex_apps,
                /*join_cached_apps_refresh*/ true,
            )
            .await
            {
                warn!("failed to refresh app list after cached Apps inventory: {err:?}");
            }
        }
    }

    async fn apps_list_response(
        outgoing: &Arc<OutgoingMessageSender>,
        params: AppsListParams,
        config: Config,
        plugins_manager: Arc<PluginsManager>,
        codex_apps: Arc<codex_mcp_extension::CodexAppsMcpExtension>,
        join_cached_apps_refresh: bool,
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

        let loaded_plugins = plugins_manager
            .plugins_for_config(&config.plugins_config_input())
            .await;
        let connector_snapshot =
            codex_connectors::ConnectorSnapshot::from_plugin_capability_summaries(
                loaded_plugins.capability_summaries(),
            );
        let plugin_apps = connector_snapshot.connector_ids().to_vec();
        let (current_snapshot, mut all_connectors) = tokio::join!(
            codex_apps.current_snapshot(&config),
            connectors::list_cached_all_connectors(&config, &plugin_apps)
        );
        let mut accessible_connectors = current_snapshot
            .as_ref()
            .map(|snapshot| app_infos_from_snapshot(snapshot, &connector_snapshot));
        let cached_all_connectors = all_connectors.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let accessible_config = config.clone();
        let accessible_connector_snapshot = connector_snapshot.clone();
        let accessible_tx = tx.clone();
        tokio::spawn(async move {
            let snapshot = if join_cached_apps_refresh {
                codex_apps.snapshot(&accessible_config).await
            } else if force_refetch {
                codex_apps.refresh_snapshot(&accessible_config).await
            } else {
                match current_snapshot {
                    Some(snapshot) if !snapshot.is_live_inventory() => Ok(Some(snapshot)),
                    Some(_) => codex_apps.snapshot(&accessible_config).await,
                    None => {
                        codex_apps
                            .snapshot_allowing_cached(&accessible_config)
                            .await
                    }
                }
            };
            let result = snapshot
                .map(|snapshot| {
                    let live_inventory = snapshot
                        .as_ref()
                        .is_none_or(codex_apps::CodexAppsSnapshot::is_live_inventory);
                    AccessibleApps {
                        apps: snapshot
                            .as_ref()
                            .map(|snapshot| {
                                app_infos_from_snapshot(snapshot, &accessible_connector_snapshot)
                            })
                            .unwrap_or_default(),
                        live_inventory,
                    }
                })
                .map_err(|err| format!("failed to load accessible apps: {err}"));
            let _ = accessible_tx.send(AppListLoadResult::Accessible(result));
        });

        let all_config = config.clone();
        let all_plugin_apps = plugin_apps.clone();
        tokio::spawn(async move {
            let result = connectors::list_all_connectors_with_options(
                &all_config,
                force_refetch,
                &all_plugin_apps,
            )
            .await
            .map_err(|err| format!("failed to list apps: {err}"));
            let _ = tx.send(AppListLoadResult::Directory(result));
        });

        let app_list_deadline = tokio::time::Instant::now() + APP_LIST_LOAD_TIMEOUT;
        let mut accessible_loaded = false;
        let mut all_loaded = false;
        let mut live_inventory = true;
        let mut last_notified_apps = None;

        if accessible_connectors.is_some() || all_connectors.is_some() {
            let merged = with_app_enabled_state(
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
                AppListLoadResult::Accessible(Ok(accessible)) => {
                    accessible_connectors = Some(accessible.apps);
                    live_inventory = accessible.live_inventory;
                    accessible_loaded = true;
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
            let merged = with_app_enabled_state(
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
                return paginate_apps(merged.as_slice(), start, limit)
                    .map(|response| (response, live_inventory));
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
    ) -> bool {
        match workspace_settings::codex_plugins_enabled_for_workspace(
            config,
            auth,
            Some(&self.workspace_settings_cache),
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
    Accessible(Result<AccessibleApps, String>),
    Directory(Result<Vec<AppInfo>, String>),
}

struct AccessibleApps {
    apps: Vec<AppInfo>,
    live_inventory: bool,
}

pub(super) fn app_infos_from_snapshot(
    snapshot: &codex_apps::CodexAppsSnapshot,
    plugin_connectors: &codex_connectors::ConnectorSnapshot,
) -> Vec<AppInfo> {
    app_infos_from_connectors(snapshot.apps(), plugin_connectors)
}

pub(super) fn accessible_app_infos_from_snapshot(
    snapshot: &codex_apps::CodexAppsSnapshot,
    plugin_connectors: &codex_connectors::ConnectorSnapshot,
) -> Vec<AppInfo> {
    app_infos_from_connectors(snapshot.all_connectors(), plugin_connectors)
}

fn app_infos_from_connectors(
    connectors: &[codex_apps::CodexApp],
    plugin_connectors: &codex_connectors::ConnectorSnapshot,
) -> Vec<AppInfo> {
    connectors
        .iter()
        .map(|app| AppInfo {
            id: app.id().to_string(),
            name: app.name().to_string(),
            description: app.description().map(str::to_string),
            logo_url: None,
            logo_url_dark: None,
            icon_assets: None,
            icon_dark_assets: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(codex_connectors::metadata::connector_install_url(
                app.name(),
                app.id(),
            )),
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: plugin_connectors
                .plugin_display_names_for_connector_id(app.id())
                .to_vec(),
        })
        .collect()
}

fn with_app_enabled_state(mut apps: Vec<AppInfo>, config: &Config) -> Vec<AppInfo> {
    let evaluator = codex_connectors::AppToolPolicyEvaluator::new(&config.config_layer_stack);
    for app in &mut apps {
        app.is_enabled = evaluator.app_is_enabled(&app.id);
    }
    apps
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
    let data = connectors[start..end]
        .iter()
        .cloned()
        .map(app_info_to_api)
        .collect();
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
    let data = data.into_iter().map(app_info_to_api).collect();
    outgoing
        .send_server_notification(ServerNotification::AppListUpdated(
            AppListUpdatedNotification { data },
        ))
        .await;
}
