use super::*;

const MCP_TOOL_THREAD_ID_META_KEY: &str = "threadId";

#[derive(Clone)]
pub(crate) struct McpRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    config_manager: ConfigManager,
}

impl McpRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        config_manager: ConfigManager,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            config_manager,
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

        let auth = self
            .auth_manager
            .auth()
            .await
            .map_err(|err| internal_error(format!("failed to resolve auth: {err}")))?;
        let effective_servers = self
            .thread_manager
            .mcp_manager()
            .effective_servers(&config, auth.as_ref())
            .await;
        let Some(server) = effective_servers
            .get(&name)
            .and_then(codex_mcp::EffectiveMcpServer::configured_config)
        else {
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
            config.auth_keyring_backend_kind(),
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
        let (config, thread) = match params.thread_id.as_deref() {
            Some(thread_id) => {
                let (_, thread) = self.load_thread(thread_id).await?;
                let thread_config = thread.config().await;
                let config = self
                    .config_manager
                    .load_latest_config_for_thread(thread_config.as_ref())
                    .await
                    .map_err(|err| internal_error(format!("failed to reload config: {err}")))?;
                (config, Some(thread))
            }
            None => (self.load_latest_config(/*fallback_cwd*/ None).await?, None),
        };
        let mcp_config = match thread {
            Some(thread) => thread.runtime_mcp_config(&config).await,
            None => {
                self.thread_manager
                    .mcp_manager()
                    .runtime_config(&config)
                    .await
            }
        };
        let auth = self
            .auth_manager
            .auth()
            .await
            .map_err(|err| internal_error(format!("failed to resolve auth: {err}")))?;
        let environment_manager = self.thread_manager.environment_manager();
        // This status path has no turn-selected environment. Use config cwd
        // as the local stdio fallback; named environment stdio MCPs must
        // declare their own absolute cwd.
        let runtime_context =
            McpRuntimeContext::new(Arc::clone(&environment_manager), config.cwd.to_path_buf());

        tokio::spawn(async move {
            Self::list_mcp_server_status_task(
                outgoing,
                request,
                params,
                mcp_config,
                auth,
                runtime_context,
            )
            .await;
        });
        Ok(())
    }

    async fn list_mcp_server_status_task(
        outgoing: Arc<OutgoingMessageSender>,
        request_id: ConnectionRequestId,
        params: ListMcpServerStatusParams,
        mcp_config: codex_mcp::McpConfig,
        auth: Option<CodexAuth>,
        runtime_context: McpRuntimeContext,
    ) {
        let result = Self::list_mcp_server_status_response(
            request_id.request_id.to_string(),
            params,
            mcp_config,
            auth,
            runtime_context,
        )
        .await;
        outgoing.send_result(request_id, result).await;
    }

    async fn list_mcp_server_status_response(
        request_id: String,
        params: ListMcpServerStatusParams,
        mcp_config: codex_mcp::McpConfig,
        auth: Option<CodexAuth>,
        runtime_context: McpRuntimeContext,
    ) -> Result<ListMcpServerStatusResponse, JSONRPCErrorError> {
        let detail = match params.detail.unwrap_or(McpServerStatusDetail::Full) {
            McpServerStatusDetail::Full => McpSnapshotDetail::Full,
            McpServerStatusDetail::ToolsAndAuthOnly => McpSnapshotDetail::ToolsAndAuthOnly,
        };

        let snapshot = collect_mcp_server_status_snapshot_with_detail(
            &mcp_config,
            auth.as_ref(),
            request_id,
            runtime_context,
            detail,
        )
        .await;

        let McpServerStatusSnapshot {
            server_infos,
            tools_by_server,
            resources,
            resource_templates,
            auth_statuses,
            mut server_names,
        } = snapshot;
        server_names.extend(
            auth_statuses
                .keys()
                .cloned()
                .chain(resources.keys().cloned())
                .chain(resource_templates.keys().cloned()),
        );
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
                server_info: server_infos.get(name).cloned(),
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
        let mcp_config = self
            .thread_manager
            .mcp_manager()
            .runtime_config(&config)
            .await;
        let auth = self
            .auth_manager
            .auth()
            .await
            .map_err(|err| internal_error(format!("failed to resolve auth: {err}")))?;
        let environment_manager = self.thread_manager.environment_manager();
        // This threadless resource-read path has no turn cwd or turn-selected
        // environment. Use config cwd only as the local stdio fallback; named
        // environment stdio MCPs must declare their own absolute cwd.
        let runtime_context =
            McpRuntimeContext::new(Arc::clone(&environment_manager), config.cwd.to_path_buf());
        let request_id = request_id.clone();

        tokio::spawn(async move {
            let result = read_mcp_resource_without_thread(
                &mcp_config,
                auth.as_ref(),
                runtime_context,
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
