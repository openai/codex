use super::*;
use crate::app_info::app_info_to_api;
use codex_core::connectors as core_connectors;

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

    pub(crate) async fn apps_installed(
        &self,
        params: AppsInstalledParams,
    ) -> Result<AppsInstalledResponse, JSONRPCErrorError> {
        let started_at = Instant::now();
        let reload = params.reload;
        let mut retained_previous_snapshot = false;
        let mut refresh_disposition = if reload {
            "not_started"
        } else {
            "not_requested"
        };
        let mut snapshot_age = None;
        let mut snapshot_tool_count = 0;
        let result = async {
            let config = self
                .load_apps_installed_config(params.thread_id.as_deref())
                .await?;
            if !config.features.enabled(Feature::AppsRuntimeStateRefactor) {
                return Err(method_not_found(
                    "app/installed is not enabled for this app-server",
                ));
            }

            let auth = self.auth_manager.auth().await;
            let apps_enabled = config
                .features
                .apps_enabled_for_auth(auth.as_ref().is_some_and(CodexAuth::uses_codex_backend));

            // A cached read must not turn into a workspace-settings network request. Unknown
            // workspace policy fails closed while retaining installed identities; an explicit
            // reload bypasses the policy cache.
            let workspace_policy = if !apps_enabled {
                Ok(false)
            } else if reload {
                workspace_settings::refresh_codex_plugins_enabled_for_workspace(
                    &config,
                    auth.as_ref(),
                    Some(&self.workspace_settings_cache),
                )
                .await
            } else {
                Ok(workspace_settings::cached_codex_plugins_enabled_for_workspace(
                    &config,
                    auth.as_ref(),
                    &self.workspace_settings_cache,
                )
                .unwrap_or(false))
            };
            if let Err(err) = &workspace_policy {
                warn!(
                    "failed to refresh workspace Codex plugins setting; disabling Codex plugins for this read: {err:#}"
                );
            }
            let workspace_enabled = workspace_policy.as_ref().copied().unwrap_or(false);
            let runtime_enabled = apps_enabled && workspace_enabled;

            let mcp_manager = self.thread_manager.mcp_manager();
            let previous_snapshot = core_connectors::connector_runtime_snapshot(
                &config,
                auth.as_ref(),
                mcp_manager.as_ref(),
            );
            let snapshot = if reload && runtime_enabled {
                match core_connectors::refresh_connector_runtime_snapshot(
                        &config,
                        auth.as_ref(),
                        self.thread_manager.environment_manager(),
                        mcp_manager.as_ref(),
                    )
                    .await
                {
                    Ok(snapshot) => {
                        refresh_disposition = "success";
                        Some(snapshot)
                    }
                    Err(err) => {
                        refresh_disposition = "error";
                        retained_previous_snapshot =
                            core_connectors::peek_connector_runtime_snapshot(
                                &config,
                                auth.as_ref(),
                                mcp_manager.as_ref(),
                            )
                            .is_some();
                        return Err(internal_error(format!(
                            "failed to refresh installed connector runtime state: {err:#}"
                        )));
                    }
                }
            } else {
                if reload {
                    refresh_disposition = if !apps_enabled {
                        "skipped_apps_disabled"
                    } else if workspace_policy.is_err() {
                        "skipped_workspace_policy_error"
                    } else {
                        "skipped_workspace_disabled"
                    };
                    retained_previous_snapshot = previous_snapshot.is_some();
                }
                previous_snapshot
            };
            let Some(snapshot) = snapshot else {
                return Ok(AppsInstalledResponse { apps: Vec::new() });
            };

            snapshot_age = Some(snapshot.age());
            snapshot_tool_count = snapshot.tools().len();
            let connector_ids = core_connectors::installed_app_ids(snapshot.as_ref());
            let callable_connector_ids = if runtime_enabled {
                core_connectors::callable_app_ids(
                    &config,
                    auth.as_ref(),
                    mcp_manager.as_ref(),
                    snapshot.as_ref(),
                )
                .await
            } else {
                Default::default()
            };
            let apps = connector_ids
                .into_iter()
                .map(|id| {
                    let enabled = runtime_enabled && core_connectors::app_is_enabled(&config, &id);
                    InstalledApp {
                        id: id.clone(),
                        enabled,
                        callable: runtime_enabled && callable_connector_ids.contains(&id),
                    }
                })
                .collect::<Vec<_>>();
            Ok(AppsInstalledResponse { apps })
        }
        .await;

        record_apps_installed_metrics(
            started_at,
            reload,
            retained_previous_snapshot,
            refresh_disposition,
            snapshot_age,
            snapshot_tool_count,
            result.as_ref().ok(),
        );
        result
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
        let installed_start = Instant::now();
        let reload = params.force_refetch;
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
            let response = AppsListResponse {
                data: Vec::new(),
                next_cursor: None,
            };
            record_legacy_apps_installed_duration(installed_start, reload);
            return Ok(Some(response));
        }

        if !self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await
        {
            let response = AppsListResponse {
                data: Vec::new(),
                next_cursor: None,
            };
            record_legacy_apps_installed_duration(installed_start, reload);
            return Ok(Some(response));
        }

        let request = request_id.clone();
        let outgoing = Arc::clone(&self.outgoing);
        let environment_manager = self.thread_manager.environment_manager();
        let mcp_manager = self.thread_manager.mcp_manager();
        let plugins_manager = self.thread_manager.plugins_manager();
        let shutdown_token = self.shutdown_token.child_token();
        tokio::spawn(async move {
            tokio::select! {
                _ = shutdown_token.cancelled() => {}
                _ = Self::apps_list_task(
                    outgoing,
                    request,
                    params,
                    config,
                    environment_manager,
                    mcp_manager,
                    plugins_manager,
                    installed_start,
                ) => {}
            }
        });
        Ok(None)
    }

    pub(crate) fn shutdown(&self) {
        self.shutdown_token.cancel();
    }

    #[allow(clippy::too_many_arguments)]
    async fn apps_list_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: AppsListParams,
        config: Config,
        environment_manager: Arc<EnvironmentManager>,
        mcp_manager: Arc<McpManager>,
        plugins_manager: Arc<PluginsManager>,
        installed_start: Instant,
    ) {
        let reload = params.force_refetch;
        let retry_params = params.clone();
        let retry_config = config.clone();
        let retry_environment_manager = Arc::clone(&environment_manager);
        let retry_mcp_manager = Arc::clone(&mcp_manager);
        let retry_plugins_manager = Arc::clone(&plugins_manager);
        let result = Self::apps_list_response(
            &outgoing,
            params,
            config,
            environment_manager,
            mcp_manager,
            plugins_manager,
        )
        .await;
        if result.is_ok() {
            record_legacy_apps_installed_duration(installed_start, reload);
        }
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
                retry_mcp_manager,
                retry_plugins_manager,
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
        mcp_manager: Arc<McpManager>,
        plugins_manager: Arc<PluginsManager>,
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
        let (mut accessible_connectors, mut all_connectors) = tokio::join!(
            connectors::list_cached_accessible_connectors_from_mcp_tools(&config),
            connectors::list_cached_all_connectors(&config, &plugin_apps)
        );
        let cached_all_connectors = all_connectors.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let accessible_config = config.clone();
        let accessible_tx = tx.clone();
        tokio::spawn(async move {
            let result = connectors::list_accessible_connectors_from_mcp_tools_with_mcp_manager(
                &accessible_config,
                force_refetch,
                Arc::clone(&environment_manager),
                mcp_manager,
            )
            .await
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

    async fn load_apps_installed_config(
        &self,
        thread_id: Option<&str>,
    ) -> Result<Config, JSONRPCErrorError> {
        let Some(thread_id) = thread_id else {
            return self.load_latest_config(/*fallback_cwd*/ None).await;
        };
        let (_, thread) = self.load_thread(thread_id).await?;
        let thread_config = thread.config().await;
        self.config_manager
            .load_latest_config_for_thread(thread_config.as_ref())
            .await
            .map_err(|err| internal_error(format!("failed to reload config: {err}")))
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
// `app/list` is the legacy request-path baseline for the future `app/installed` endpoint;
// `path=legacy` keeps it separate from the new snapshot-backed implementation in dashboards.
const APPS_INSTALLED_DURATION_METRIC: &str = "codex.apps.installed.duration_ms";
const APPS_INSTALLED_RESPONSE_BYTES_METRIC: &str = "codex.apps.installed.response_bytes";
const APPS_INSTALLED_CONNECTOR_COUNT_METRIC: &str = "codex.apps.installed.connector_count";
const APPS_INSTALLED_TOOL_COUNT_METRIC: &str = "codex.apps.installed.tool_count";
const APPS_SNAPSHOT_AGE_METRIC: &str = "codex.apps.snapshot.age_ms";

fn record_apps_installed_metrics(
    started_at: Instant,
    reload: bool,
    retained_previous_snapshot: bool,
    refresh_disposition: &'static str,
    snapshot_age: Option<Duration>,
    snapshot_tool_count: usize,
    response: Option<&AppsInstalledResponse>,
) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let reload = if reload { "true" } else { "false" };
    let outcome = if response.is_some() {
        "success"
    } else {
        "error"
    };
    let retained_previous_snapshot = if retained_previous_snapshot {
        "true"
    } else {
        "false"
    };
    let _ = metrics.record_duration(
        APPS_INSTALLED_DURATION_METRIC,
        started_at.elapsed(),
        &[
            ("path", "new"),
            ("reload", reload),
            ("refresh", refresh_disposition),
            ("outcome", outcome),
            ("retained_previous_snapshot", retained_previous_snapshot),
        ],
    );
    let Some(response) = response else {
        return;
    };
    if let Ok(bytes) = serde_json::to_vec(response) {
        let _ = metrics.histogram(
            APPS_INSTALLED_RESPONSE_BYTES_METRIC,
            usize_to_i64(bytes.len()),
            &[("path", "new")],
        );
    }
    let _ = metrics.histogram(
        APPS_INSTALLED_CONNECTOR_COUNT_METRIC,
        usize_to_i64(response.apps.len()),
        &[("path", "new")],
    );
    let _ = metrics.histogram(
        APPS_INSTALLED_TOOL_COUNT_METRIC,
        usize_to_i64(snapshot_tool_count),
        &[("path", "new")],
    );
    if let Some(snapshot_age) = snapshot_age {
        let _ = metrics.record_duration(
            APPS_SNAPSHOT_AGE_METRIC,
            snapshot_age,
            &[("path", "new"), ("observation", "installed")],
        );
    }
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn record_legacy_apps_installed_duration(started_at: Instant, reload: bool) {
    let reload = if reload { "true" } else { "false" };
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.record_duration(
            APPS_INSTALLED_DURATION_METRIC,
            started_at.elapsed(),
            &[("path", "legacy"), ("reload", reload)],
        );
    }
}

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
