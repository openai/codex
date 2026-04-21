//! Background app-server requests launched by the TUI app.
//!
//! This module owns fire-and-forget fetch/write helpers for MCP inventory, skills, plugins, rate
//! limits, add-credit nudges, and feedback uploads. Results are routed back through `AppEvent` so
//! the main event loop remains single-threaded.

use super::*;

impl App {
    pub(super) fn fetch_mcp_inventory(
        &mut self,
        app_server: &AppServerSession,
        detail: McpServerStatusDetail,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_all_mcp_server_statuses(request_handle, detail)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::McpInventoryLoaded { result, detail });
        });
    }

    /// Spawns a background task to fetch account rate limits and deliver the
    /// result as a `RateLimitsLoaded` event.
    ///
    /// The `origin` is forwarded to the completion handler so it can distinguish
    /// a startup prefetch (which only updates cached snapshots and schedules a
    /// frame) from a `/status`-triggered refresh (which must finalize the
    /// corresponding status card).
    pub(super) fn refresh_rate_limits(
        &mut self,
        app_server: &AppServerSession,
        origin: RateLimitRefreshOrigin,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_account_rate_limits(request_handle)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::RateLimitsLoaded { origin, result });
        });
    }

    pub(super) fn send_add_credits_nudge_email(
        &mut self,
        app_server: &AppServerSession,
        credit_type: AddCreditsNudgeCreditType,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = send_add_credits_nudge_email(request_handle, credit_type)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::AddCreditsNudgeEmailFinished { result });
        });
    }

    /// Starts the initial skills refresh without delaying the first interactive frame.
    ///
    /// Startup only needs skill metadata to populate skill mentions and the skills UI; the prompt can be
    /// rendered before that metadata arrives. The result is routed through the normal app event queue so
    /// the same response handler updates the chat widget and emits invalid `SKILL.md` warnings once the
    /// app-server RPC finishes. User-initiated skills refreshes still use the blocking app command path so
    /// callers that explicitly asked for fresh skill state do not race ahead of their own refresh.
    pub(super) fn refresh_startup_skills(&mut self, app_server: &AppServerSession) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let cwd = self.config.cwd.to_path_buf();
        tokio::spawn(async move {
            let result = fetch_skills_list(request_handle, cwd)
                .await
                .map_err(|err| format!("{err:#}"));
            app_event_tx.send(AppEvent::SkillsListLoaded { result });
        });
    }

    pub(super) fn fetch_plugins_list(&mut self, app_server: &AppServerSession, cwd: PathBuf) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_plugins_list(request_handle, cwd.clone())
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::PluginsLoaded { cwd, result });
        });
    }

    pub(super) fn fetch_plugin_detail(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        params: PluginReadParams,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = fetch_plugin_detail(request_handle, params)
                .await
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::PluginDetailLoaded { cwd, result });
        });
    }

    pub(super) fn fetch_plugin_install(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        marketplace_path: AbsolutePathBuf,
        plugin_name: String,
        plugin_display_name: String,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cwd_for_event = cwd.clone();
            let marketplace_path_for_event = marketplace_path.clone();
            let plugin_name_for_event = plugin_name.clone();
            let result = fetch_plugin_install(request_handle, marketplace_path, plugin_name)
                .await
                .map_err(|err| format!("Failed to install plugin: {err}"));
            app_event_tx.send(AppEvent::PluginInstallLoaded {
                cwd: cwd_for_event,
                marketplace_path: marketplace_path_for_event,
                plugin_name: plugin_name_for_event,
                plugin_display_name,
                result,
            });
        });
    }

    pub(super) fn fetch_plugin_uninstall(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        plugin_id: String,
        plugin_display_name: String,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cwd_for_event = cwd.clone();
            let plugin_id_for_event = plugin_id.clone();
            let result = fetch_plugin_uninstall(request_handle, plugin_id)
                .await
                .map_err(|err| format!("Failed to uninstall plugin: {err}"));
            app_event_tx.send(AppEvent::PluginUninstallLoaded {
                cwd: cwd_for_event,
                plugin_id: plugin_id_for_event,
                plugin_display_name,
                result,
            });
        });
    }

    pub(super) fn set_plugin_enabled(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        plugin_id: String,
        enabled: bool,
    ) {
        if let Some(queued_enabled) = self.pending_plugin_enabled_writes.get_mut(&plugin_id) {
            *queued_enabled = Some(enabled);
            return;
        }

        self.pending_plugin_enabled_writes
            .insert(plugin_id.clone(), None);
        self.spawn_plugin_enabled_write(app_server, cwd, plugin_id, enabled);
    }

    pub(super) fn spawn_plugin_enabled_write(
        &mut self,
        app_server: &AppServerSession,
        cwd: PathBuf,
        plugin_id: String,
        enabled: bool,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let cwd_for_event = cwd.clone();
            let plugin_id_for_event = plugin_id.clone();
            let result = write_plugin_enabled(request_handle, plugin_id, enabled)
                .await
                .map(|_| ())
                .map_err(|err| format!("Failed to update plugin config: {err}"));
            app_event_tx.send(AppEvent::PluginEnabledSet {
                cwd: cwd_for_event,
                plugin_id: plugin_id_for_event,
                enabled,
                result,
            });
        });
    }

    pub(super) fn refresh_plugin_mentions(&mut self) {
        let config = self.config.clone();
        let app_event_tx = self.app_event_tx.clone();
        if !config.features.enabled(Feature::Plugins) {
            app_event_tx.send(AppEvent::PluginMentionsLoaded { plugins: None });
            return;
        }

        tokio::spawn(async move {
            let plugins = PluginsManager::new(config.codex_home.to_path_buf())
                .plugins_for_config(&config)
                .await
                .capability_summaries()
                .to_vec();
            app_event_tx.send(AppEvent::PluginMentionsLoaded {
                plugins: Some(plugins),
            });
        });
    }

    pub(super) fn submit_feedback(
        &mut self,
        app_server: &AppServerSession,
        category: FeedbackCategory,
        reason: Option<String>,
        turn_id: Option<String>,
        include_logs: bool,
    ) {
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        let origin_thread_id = self.chat_widget.thread_id();
        let rollout_path = if include_logs {
            self.chat_widget.rollout_path()
        } else {
            None
        };
        let params = build_feedback_upload_params(
            origin_thread_id,
            rollout_path,
            category,
            reason,
            turn_id,
            include_logs,
        );
        tokio::spawn(async move {
            let result = fetch_feedback_upload(request_handle, params)
                .await
                .map(|response| response.thread_id)
                .map_err(|err| err.to_string());
            app_event_tx.send(AppEvent::FeedbackSubmitted {
                origin_thread_id,
                category,
                include_logs,
                result,
            });
        });
    }

    pub(super) fn handle_feedback_thread_event(&mut self, event: FeedbackThreadEvent) {
        match event.result {
            Ok(thread_id) => {
                self.chat_widget
                    .add_to_history(crate::bottom_pane::feedback_success_cell(
                        event.category,
                        event.include_logs,
                        &thread_id,
                        event.feedback_audience,
                    ))
            }
            Err(err) => self
                .chat_widget
                .add_to_history(history_cell::new_error_event(format!(
                    "Failed to upload feedback: {err}"
                ))),
        }
    }

    pub(super) async fn enqueue_thread_feedback_event(
        &mut self,
        thread_id: ThreadId,
        event: FeedbackThreadEvent,
    ) {
        let (sender, store) = {
            let channel = self.ensure_thread_channel(thread_id);
            (channel.sender.clone(), Arc::clone(&channel.store))
        };

        let should_send = {
            let mut guard = store.lock().await;
            guard
                .buffer
                .push_back(ThreadBufferedEvent::FeedbackSubmission(event.clone()));
            if guard.buffer.len() > guard.capacity
                && let Some(removed) = guard.buffer.pop_front()
                && let ThreadBufferedEvent::Request(request) = &removed
            {
                guard
                    .pending_interactive_replay
                    .note_evicted_server_request(request);
            }
            guard.active
        };

        if should_send {
            match sender.try_send(ThreadBufferedEvent::FeedbackSubmission(event)) {
                Ok(()) => {}
                Err(TrySendError::Full(event)) => {
                    tokio::spawn(async move {
                        if let Err(err) = sender.send(event).await {
                            tracing::warn!("thread {thread_id} event channel closed: {err}");
                        }
                    });
                }
                Err(TrySendError::Closed(_)) => {
                    tracing::warn!("thread {thread_id} event channel closed");
                }
            }
        }
    }

    pub(super) async fn handle_feedback_submitted(
        &mut self,
        origin_thread_id: Option<ThreadId>,
        category: FeedbackCategory,
        include_logs: bool,
        result: Result<String, String>,
    ) {
        let event = FeedbackThreadEvent {
            category,
            include_logs,
            feedback_audience: self.feedback_audience,
            result,
        };
        if let Some(thread_id) = origin_thread_id {
            self.enqueue_thread_feedback_event(thread_id, event).await;
        } else {
            self.handle_feedback_thread_event(event);
        }
    }

    /// Process the completed MCP inventory fetch: clear the loading spinner, then
    /// render either the full tool/resource listing or an error into chat history.
    ///
    /// When both the local config and the app-server report zero servers, a special
    /// "empty" cell is shown instead of the full table.
    pub(super) fn handle_mcp_inventory_result(
        &mut self,
        result: Result<Vec<McpServerStatus>, String>,
        detail: McpServerStatusDetail,
    ) {
        let config = self.chat_widget.config_ref().clone();
        self.chat_widget.clear_mcp_inventory_loading();
        self.clear_committed_mcp_inventory_loading();

        let statuses = match result {
            Ok(statuses) => statuses,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to load MCP inventory: {err}"));
                return;
            }
        };

        if config.mcp_servers.get().is_empty() && statuses.is_empty() {
            self.chat_widget
                .add_to_history(history_cell::empty_mcp_output());
            return;
        }

        self.chat_widget
            .add_to_history(history_cell::new_mcp_tools_output_from_statuses(
                &config, &statuses, detail,
            ));
    }

    pub(super) fn clear_committed_mcp_inventory_loading(&mut self) {
        let Some(index) = self
            .transcript_cells
            .iter()
            .rposition(|cell| cell.as_any().is::<history_cell::McpInventoryLoadingCell>())
        else {
            return;
        };

        self.transcript_cells.remove(index);
        if let Some(Overlay::Transcript(overlay)) = &mut self.overlay {
            overlay.replace_cells(self.transcript_cells.clone());
        }
    }
}

pub(super) async fn fetch_all_mcp_server_statuses(
    request_handle: AppServerRequestHandle,
    detail: McpServerStatusDetail,
) -> Result<Vec<McpServerStatus>> {
    let mut cursor = None;
    let mut statuses = Vec::new();

    loop {
        let request_id = RequestId::String(format!("mcp-inventory-{}", Uuid::new_v4()));
        let response: ListMcpServerStatusResponse = request_handle
            .request_typed(ClientRequest::McpServerStatusList {
                request_id,
                params: ListMcpServerStatusParams {
                    cursor: cursor.clone(),
                    limit: Some(100),
                    detail: Some(detail),
                },
            })
            .await
            .wrap_err("mcpServerStatus/list failed in TUI")?;
        statuses.extend(response.data);
        if let Some(next_cursor) = response.next_cursor {
            cursor = Some(next_cursor);
        } else {
            break;
        }
    }

    Ok(statuses)
}

pub(super) async fn fetch_account_rate_limits(
    request_handle: AppServerRequestHandle,
) -> Result<Vec<RateLimitSnapshot>> {
    let request_id = RequestId::String(format!("account-rate-limits-{}", Uuid::new_v4()));
    let response: GetAccountRateLimitsResponse = request_handle
        .request_typed(ClientRequest::GetAccountRateLimits {
            request_id,
            params: None,
        })
        .await
        .wrap_err("account/rateLimits/read failed in TUI")?;

    Ok(app_server_rate_limit_snapshots_to_core(response))
}

pub(super) async fn send_add_credits_nudge_email(
    request_handle: AppServerRequestHandle,
    credit_type: AddCreditsNudgeCreditType,
) -> Result<codex_app_server_protocol::AddCreditsNudgeEmailStatus> {
    let request_id = RequestId::String(format!("add-credits-nudge-{}", Uuid::new_v4()));
    let response: codex_app_server_protocol::SendAddCreditsNudgeEmailResponse = request_handle
        .request_typed(ClientRequest::SendAddCreditsNudgeEmail {
            request_id,
            params: SendAddCreditsNudgeEmailParams { credit_type },
        })
        .await
        .wrap_err("account/sendAddCreditsNudgeEmail failed in TUI")?;

    Ok(response.status)
}

pub(super) async fn fetch_skills_list(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<SkillsListResponse> {
    let request_id = RequestId::String(format!("startup-skills-list-{}", Uuid::new_v4()));
    // Use the cloneable request handle so startup can issue this RPC from a background task without
    // extending a borrow of `AppServerSession` across the first frame render.
    request_handle
        .request_typed(ClientRequest::SkillsList {
            request_id,
            params: SkillsListParams {
                cwds: vec![cwd],
                force_reload: true,
                per_cwd_extra_user_roots: None,
            },
        })
        .await
        .wrap_err("skills/list failed in TUI")
}

pub(super) async fn fetch_plugins_list(
    request_handle: AppServerRequestHandle,
    cwd: PathBuf,
) -> Result<PluginListResponse> {
    let cwd = AbsolutePathBuf::try_from(cwd).wrap_err("plugin list cwd must be absolute")?;
    let request_id = RequestId::String(format!("plugin-list-{}", Uuid::new_v4()));
    let mut response = request_handle
        .request_typed(ClientRequest::PluginList {
            request_id,
            params: PluginListParams {
                cwds: Some(vec![cwd]),
            },
        })
        .await
        .wrap_err("plugin/list failed in TUI")?;
    hide_cli_only_plugin_marketplaces(&mut response);
    Ok(response)
}

const CLI_HIDDEN_PLUGIN_MARKETPLACES: &[&str] = &["openai-bundled"];

pub(super) fn hide_cli_only_plugin_marketplaces(response: &mut PluginListResponse) {
    response
        .marketplaces
        .retain(|marketplace| !CLI_HIDDEN_PLUGIN_MARKETPLACES.contains(&marketplace.name.as_str()));
}

pub(super) async fn fetch_plugin_detail(
    request_handle: AppServerRequestHandle,
    params: PluginReadParams,
) -> Result<PluginReadResponse> {
    let request_id = RequestId::String(format!("plugin-read-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginRead { request_id, params })
        .await
        .wrap_err("plugin/read failed in TUI")
}

pub(super) async fn fetch_plugin_install(
    request_handle: AppServerRequestHandle,
    marketplace_path: AbsolutePathBuf,
    plugin_name: String,
) -> Result<PluginInstallResponse> {
    let request_id = RequestId::String(format!("plugin-install-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginInstall {
            request_id,
            params: PluginInstallParams {
                marketplace_path: Some(marketplace_path),
                remote_marketplace_name: None,
                plugin_name,
            },
        })
        .await
        .wrap_err("plugin/install failed in TUI")
}

pub(super) async fn fetch_plugin_uninstall(
    request_handle: AppServerRequestHandle,
    plugin_id: String,
) -> Result<PluginUninstallResponse> {
    let request_id = RequestId::String(format!("plugin-uninstall-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::PluginUninstall {
            request_id,
            params: PluginUninstallParams { plugin_id },
        })
        .await
        .wrap_err("plugin/uninstall failed in TUI")
}

pub(super) async fn write_plugin_enabled(
    request_handle: AppServerRequestHandle,
    plugin_id: String,
    enabled: bool,
) -> Result<ConfigWriteResponse> {
    let request_id = RequestId::String(format!("plugin-enable-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::ConfigValueWrite {
            request_id,
            params: ConfigValueWriteParams {
                key_path: format!("plugins.{plugin_id}"),
                value: serde_json::json!({ "enabled": enabled }),
                merge_strategy: MergeStrategy::Upsert,
                file_path: None,
                expected_version: None,
            },
        })
        .await
        .wrap_err("config/value/write failed while updating plugin enablement in TUI")
}

pub(super) fn build_feedback_upload_params(
    origin_thread_id: Option<ThreadId>,
    rollout_path: Option<PathBuf>,
    category: FeedbackCategory,
    reason: Option<String>,
    turn_id: Option<String>,
    include_logs: bool,
) -> FeedbackUploadParams {
    let extra_log_files = if include_logs {
        rollout_path.map(|rollout_path| vec![rollout_path])
    } else {
        None
    };
    let tags = turn_id.map(|turn_id| BTreeMap::from([(String::from("turn_id"), turn_id)]));
    FeedbackUploadParams {
        classification: crate::bottom_pane::feedback_classification(category).to_string(),
        reason,
        thread_id: origin_thread_id.map(|thread_id| thread_id.to_string()),
        include_logs,
        extra_log_files,
        tags,
    }
}

pub(super) async fn fetch_feedback_upload(
    request_handle: AppServerRequestHandle,
    params: FeedbackUploadParams,
) -> Result<FeedbackUploadResponse> {
    let request_id = RequestId::String(format!("feedback-upload-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::FeedbackUpload { request_id, params })
        .await
        .wrap_err("feedback/upload failed in TUI")
}

/// Convert flat `McpServerStatus` responses into the per-server maps used by the
/// in-process MCP subsystem (tools keyed as `mcp__{server}__{tool}`, plus
/// per-server resource/template/auth maps). Test-only because the TUI
/// renders directly from `McpServerStatus` rather than these maps.
#[cfg(test)]
pub(super) type McpInventoryMaps = (
    HashMap<String, codex_protocol::mcp::Tool>,
    HashMap<String, Vec<codex_protocol::mcp::Resource>>,
    HashMap<String, Vec<codex_protocol::mcp::ResourceTemplate>>,
    HashMap<String, McpAuthStatus>,
);

#[cfg(test)]
pub(super) fn mcp_inventory_maps_from_statuses(statuses: Vec<McpServerStatus>) -> McpInventoryMaps {
    let mut tools = HashMap::new();
    let mut resources = HashMap::new();
    let mut resource_templates = HashMap::new();
    let mut auth_statuses = HashMap::new();

    for status in statuses {
        let server_name = status.name;
        auth_statuses.insert(
            server_name.clone(),
            match status.auth_status {
                codex_app_server_protocol::McpAuthStatus::Unsupported => McpAuthStatus::Unsupported,
                codex_app_server_protocol::McpAuthStatus::NotLoggedIn => McpAuthStatus::NotLoggedIn,
                codex_app_server_protocol::McpAuthStatus::BearerToken => McpAuthStatus::BearerToken,
                codex_app_server_protocol::McpAuthStatus::OAuth => McpAuthStatus::OAuth,
            },
        );
        resources.insert(server_name.clone(), status.resources);
        resource_templates.insert(server_name.clone(), status.resource_templates);
        for (tool_name, tool) in status.tools {
            tools.insert(format!("mcp__{server_name}__{tool_name}"), tool);
        }
    }

    (tools, resources, resource_templates, auth_statuses)
}
