use super::*;

const MCP_TOOL_THREAD_ID_META_KEY: &str = "threadId";

#[derive(Clone)]
pub(crate) struct McpRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
    runtime_capabilities: Arc<RuntimeCapabilities>,
}

impl McpRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
        runtime_capabilities: Arc<RuntimeCapabilities>,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            config_manager,
            runtime_capabilities,
        }
    }

    pub(crate) async fn mcp_server_oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.mcp_server_oauth_login_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn mcp_server_refresh(
        &self,
        params: Option<()>,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.mcp_server_refresh_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn mcp_server_status_list(
        &self,
        request_id: &ConnectionRequestId,
        params: ListMcpServerStatusParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.list_mcp_server_status(request_id, params)
            .await
            .map(|()| None)
    }

    pub(crate) async fn mcp_resource_read(
        &self,
        request_id: &ConnectionRequestId,
        params: McpResourceReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.read_mcp_resource(request_id, params)
            .await
            .map(|()| None)
    }

    pub(crate) async fn mcp_server_tool_call(
        &self,
        request_id: &ConnectionRequestId,
        params: McpServerToolCallParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.call_mcp_server_tool(request_id, params)
            .await
            .map(|()| None)
    }

    async fn mcp_server_refresh_response(
        &self,
        _params: Option<()>,
    ) -> Result<McpServerRefreshResponse, JSONRPCErrorError> {
        crate::mcp_refresh::queue_strict_refresh(&self.thread_manager, &self.config_manager)
            .await
            .map_err(|err| internal_error(format!("failed to refresh MCP servers: {err}")))?;
        Ok(McpServerRefreshResponse {})
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

    async fn mcp_server_oauth_login_response(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let McpServerOauthLoginParams {
            name,
            scopes,
            timeout_secs,
        } = params;

        let configured_servers = self
            .thread_manager
            .mcp_manager()
            .configured_servers(&config)
            .await;
        let Some(server) = configured_servers.get(&name) else {
            return Err(invalid_request(format!(
                "No MCP server named '{name}' found."
            )));
        };

        let (url, http_headers, env_http_headers) = match &server.transport {
            McpServerTransportConfig::StreamableHttp {
                url,
                http_headers,
                env_http_headers,
                ..
            } => (url.clone(), http_headers.clone(), env_http_headers.clone()),
            _ => {
                return Err(invalid_request(
                    "OAuth login is only supported for streamable HTTP servers.",
                ));
            }
        };

        let discovered_scopes = if scopes.is_none() && server.scopes.is_none() {
            discover_supported_scopes(&server.transport).await
        } else {
            None
        };
        let resolved_scopes =
            resolve_oauth_scopes(scopes, server.scopes.clone(), discovered_scopes);

        let handle = perform_oauth_login_return_url(
            &name,
            &url,
            config.mcp_oauth_credentials_store_mode,
            http_headers,
            env_http_headers,
            &resolved_scopes.scopes,
            server.oauth_client_id(),
            server.oauth_resource.as_deref(),
            timeout_secs,
            config.mcp_oauth_callback_port,
            config.mcp_oauth_callback_url.as_deref(),
        )
        .await
        .map_err(|err| internal_error(format!("failed to login to MCP server '{name}': {err}")))?;
        let authorization_url = handle.authorization_url().to_string();
        let notification_name = name.clone();
        let outgoing = Arc::clone(&self.outgoing);

        tokio::spawn(async move {
            let (success, error) = match handle.wait().await {
                Ok(()) => (true, None),
                Err(err) => (false, Some(err.to_string())),
            };

            let notification = ServerNotification::McpServerOauthLoginCompleted(
                McpServerOauthLoginCompletedNotification {
                    name: notification_name,
                    success,
                    error,
                },
            );
            outgoing.send_server_notification(notification).await;
        });

        Ok(McpServerOauthLoginResponse { authorization_url })
    }

    async fn list_mcp_server_status(
        &self,
        request_id: &ConnectionRequestId,
        params: ListMcpServerStatusParams,
    ) -> Result<(), JSONRPCErrorError> {
        let request = request_id.clone();

        let outgoing = Arc::clone(&self.outgoing);
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let mcp_config = config
            .to_mcp_config(self.thread_manager.plugins_manager().as_ref())
            .await;
        let auth = self.auth_manager.auth().await;
        let runtime_environment = runtime_environment_without_thread(
            self.thread_manager.environment_manager().as_ref(),
            &self.runtime_capabilities,
            config.cwd.to_path_buf(),
            "list MCP server status without thread",
        )?;

        tokio::spawn(async move {
            Self::list_mcp_server_status_task(
                outgoing,
                request,
                params,
                config,
                mcp_config,
                auth,
                runtime_environment,
            )
            .await;
        });
        Ok(())
    }

    async fn list_mcp_server_status_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: ListMcpServerStatusParams,
        config: Config,
        mcp_config: codex_mcp::McpConfig,
        auth: Option<CodexAuth>,
        runtime_environment: McpRuntimeEnvironment,
    ) {
        let result = Self::list_mcp_server_status_response(
            request_id.request_id.to_string(),
            params,
            config,
            mcp_config,
            auth,
            runtime_environment,
        )
        .await;
        outgoing.send_result(request_id, result).await;
    }

    async fn list_mcp_server_status_response(
        request_id: String,
        params: ListMcpServerStatusParams,
        config: Config,
        mcp_config: codex_mcp::McpConfig,
        auth: Option<CodexAuth>,
        runtime_environment: McpRuntimeEnvironment,
    ) -> Result<ListMcpServerStatusResponse, JSONRPCErrorError> {
        let detail = match params.detail.unwrap_or(McpServerStatusDetail::Full) {
            McpServerStatusDetail::Full => McpSnapshotDetail::Full,
            McpServerStatusDetail::ToolsAndAuthOnly => McpSnapshotDetail::ToolsAndAuthOnly,
        };

        let snapshot = collect_mcp_server_status_snapshot_with_detail(
            &mcp_config,
            auth.as_ref(),
            request_id,
            runtime_environment,
            detail,
        )
        .await;

        let effective_servers = effective_mcp_servers(&mcp_config, auth.as_ref());
        let McpServerStatusSnapshot {
            tools_by_server,
            resources,
            resource_templates,
            auth_statuses,
        } = snapshot;

        let mut server_names: Vec<String> = config
            .mcp_servers
            .keys()
            .cloned()
            // Include runtime-added/plugin MCP servers that are present in the
            // effective runtime config even when they are not user-declared in
            // `config.mcp_servers`.
            .chain(effective_servers.keys().cloned())
            .chain(auth_statuses.keys().cloned())
            .chain(resources.keys().cloned())
            .chain(resource_templates.keys().cloned())
            .collect();
        server_names.sort();
        server_names.dedup();

        let total = server_names.len();
        let limit = params.limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = limit.min(total);
        let start = match params.cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => return Err(invalid_request(format!("invalid cursor: {cursor}"))),
            },
            None => 0,
        };

        if start > total {
            return Err(invalid_request(format!(
                "cursor {start} exceeds total MCP servers {total}"
            )));
        }

        let end = start.saturating_add(effective_limit).min(total);

        let data: Vec<McpServerStatus> = server_names[start..end]
            .iter()
            .map(|name| McpServerStatus {
                name: name.clone(),
                tools: tools_by_server.get(name).cloned().unwrap_or_default(),
                resources: resources.get(name).cloned().unwrap_or_default(),
                resource_templates: resource_templates.get(name).cloned().unwrap_or_default(),
                auth_status: auth_statuses
                    .get(name)
                    .cloned()
                    .unwrap_or(CoreMcpAuthStatus::Unsupported)
                    .into(),
            })
            .collect();

        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        Ok(ListMcpServerStatusResponse { data, next_cursor })
    }

    async fn read_mcp_resource(
        &self,
        request_id: &ConnectionRequestId,
        params: McpResourceReadParams,
    ) -> Result<(), JSONRPCErrorError> {
        let outgoing = Arc::clone(&self.outgoing);
        let McpResourceReadParams {
            thread_id,
            server,
            uri,
        } = params;

        if let Some(thread_id) = thread_id {
            let (_, thread) = self.load_thread(&thread_id).await?;
            let request_id = request_id.clone();

            tokio::spawn(async move {
                let result = thread.read_mcp_resource(&server, &uri).await;
                Self::send_mcp_resource_read_response(outgoing, request_id, result).await;
            });
            return Ok(());
        }

        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let mcp_config = config
            .to_mcp_config(self.thread_manager.plugins_manager().as_ref())
            .await;
        let auth = self.auth_manager.auth().await;
        let runtime_environment = runtime_environment_without_thread(
            self.thread_manager.environment_manager().as_ref(),
            &self.runtime_capabilities,
            config.cwd.to_path_buf(),
            "read MCP resource without thread",
        )?;
        let request_id = request_id.clone();

        tokio::spawn(async move {
            let result = read_mcp_resource_without_thread(
                &mcp_config,
                auth.as_ref(),
                runtime_environment,
                &server,
                &uri,
            )
            .await
            .and_then(|result| serde_json::to_value(result).map_err(anyhow::Error::from));
            Self::send_mcp_resource_read_response(outgoing, request_id, result).await;
        });
        Ok(())
    }

    async fn send_mcp_resource_read_response(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        result: anyhow::Result<serde_json::Value>,
    ) {
        let result = result
            .map_err(|error| internal_error(format!("{error:#}")))
            .and_then(|result| {
                serde_json::from_value::<McpResourceReadResponse>(result).map_err(|error| {
                    internal_error(format!(
                        "failed to deserialize MCP resource read response: {error}"
                    ))
                })
            });
        outgoing.send_result(request_id, result).await;
    }

    async fn call_mcp_server_tool(
        &self,
        request_id: &ConnectionRequestId,
        params: McpServerToolCallParams,
    ) -> Result<(), JSONRPCErrorError> {
        let outgoing = Arc::clone(&self.outgoing);
        let thread_id = params.thread_id.clone();
        let (_, thread) = self.load_thread(&thread_id).await?;
        let meta = with_mcp_tool_call_thread_id_meta(params.meta, &thread_id);
        let request_id = request_id.clone();

        tokio::spawn(async move {
            let result = thread
                .call_mcp_tool(&params.server, &params.tool, params.arguments, meta)
                .await
                .map(McpServerToolCallResponse::from)
                .map_err(|error| internal_error(format!("{error:#}")));
            outgoing.send_result(request_id, result).await;
        });
        Ok(())
    }
}

fn runtime_environment_without_thread(
    environment_manager: &EnvironmentManager,
    runtime_capabilities: &RuntimeCapabilities,
    cwd: PathBuf,
    local_fallback_operation: &str,
) -> Result<McpRuntimeEnvironment, JSONRPCErrorError> {
    let environment = match environment_manager.default_environment() {
        Some(environment) => environment,
        None => runtime_capabilities
            .require_local_environment(local_fallback_operation)
            .map_err(|err| internal_error(err.to_string()))?,
    };
    // Threadless MCP requests have no turn cwd. This fallback is used only
    // by executor-backed stdio MCPs whose config omits `cwd`.
    Ok(McpRuntimeEnvironment::new(environment, cwd))
}

#[cfg(test)]
mod tests {
    use super::runtime_environment_without_thread;
    use codex_core::RuntimeCapabilities;
    use codex_exec_server::EnvironmentManager;
    use codex_exec_server::ExecServerRuntimePaths;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn threadless_mcp_local_fallback_rejects_isolated_runtime() {
        let environment_manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());
        let error = runtime_environment_without_thread(
            &environment_manager,
            &RuntimeCapabilities::isolated(),
            test_cwd(),
            "list MCP server status without thread",
        )
        .expect_err("isolated runtime should reject local MCP fallback");

        assert_eq!(
            error.message,
            "list MCP server status without thread requires ambient worker-local environment"
        );
    }

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    fn test_cwd() -> std::path::PathBuf {
        AbsolutePathBuf::current_dir()
            .expect("current dir")
            .to_path_buf()
    }
}

fn with_mcp_tool_call_thread_id_meta(
    meta: Option<serde_json::Value>,
    thread_id: &str,
) -> Option<serde_json::Value> {
    match meta {
        Some(serde_json::Value::Object(mut map)) => {
            map.insert(
                MCP_TOOL_THREAD_ID_META_KEY.to_string(),
                serde_json::Value::String(thread_id.to_string()),
            );
            Some(serde_json::Value::Object(map))
        }
        None => {
            let mut map = serde_json::Map::new();
            map.insert(
                MCP_TOOL_THREAD_ID_META_KEY.to_string(),
                serde_json::Value::String(thread_id.to_string()),
            );
            Some(serde_json::Value::Object(map))
        }
        other => other,
    }
}
