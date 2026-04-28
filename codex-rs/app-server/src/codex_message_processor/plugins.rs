use super::*;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::PluginInstallPolicy;

impl CodexMessageProcessor {
    pub(super) async fn plugin_list(
        &self,
        request_id: ConnectionRequestId,
        params: PluginListParams,
    ) {
        let result = self.plugin_list_response(params).await;
        self.outgoing.send_result(request_id, result).await;
    }

    async fn plugin_list_response(
        &self,
        params: PluginListParams,
    ) -> Result<PluginListResponse, JSONRPCErrorError> {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginListParams { cwds } = params;
        let roots = cwds.unwrap_or_default();

        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let empty_response = || PluginListResponse {
            marketplaces: Vec::new(),
            marketplace_load_errors: Vec::new(),
            featured_plugin_ids: Vec::new(),
        };
        if !config.features.enabled(Feature::Plugins) {
            return Ok(empty_response());
        }
        let auth = self.auth_manager.auth().await;
        if !self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await
        {
            return Ok(empty_response());
        }
        plugins_manager.maybe_start_non_curated_plugin_cache_refresh(&roots);

        let config_for_marketplace_listing = config.clone();
        let plugins_manager_for_marketplace_listing = plugins_manager.clone();
        let (mut data, marketplace_load_errors) = match tokio::task::spawn_blocking(move || {
            let outcome = plugins_manager_for_marketplace_listing
                .list_marketplaces_for_config(&config_for_marketplace_listing, &roots)?;
            Ok::<
                (
                    Vec<PluginMarketplaceEntry>,
                    Vec<codex_app_server_protocol::MarketplaceLoadErrorInfo>,
                ),
                MarketplaceError,
            >((
                outcome
                    .marketplaces
                    .into_iter()
                    .map(|marketplace| PluginMarketplaceEntry {
                        name: marketplace.name,
                        path: Some(marketplace.path),
                        interface: marketplace.interface.map(|interface| MarketplaceInterface {
                            display_name: interface.display_name,
                        }),
                        plugins: marketplace
                            .plugins
                            .into_iter()
                            .map(|plugin| PluginSummary {
                                id: plugin.id,
                                installed: plugin.installed,
                                enabled: plugin.enabled,
                                name: plugin.name,
                                source: marketplace_plugin_source_to_info(plugin.source),
                                install_policy: plugin.policy.installation.into(),
                                auth_policy: plugin.policy.authentication.into(),
                                interface: plugin.interface.map(local_plugin_interface_to_info),
                            })
                            .collect(),
                    })
                    .collect(),
                outcome
                    .errors
                    .into_iter()
                    .map(|err| codex_app_server_protocol::MarketplaceLoadErrorInfo {
                        marketplace_path: err.path,
                        message: err.message,
                    })
                    .collect(),
            ))
        })
        .await
        {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(err)) => return Err(Self::marketplace_error(err, "list marketplace plugins")),
            Err(err) => {
                return Err(internal_error(format!(
                    "failed to list marketplace plugins: {err}"
                )));
            }
        };

        if config.features.enabled(Feature::RemotePlugin) {
            let remote_plugin_service_config = RemotePluginServiceConfig {
                chatgpt_base_url: config.chatgpt_base_url.clone(),
            };
            match codex_core_plugins::remote::fetch_remote_marketplaces(
                &remote_plugin_service_config,
                auth.as_ref(),
            )
            .await
            {
                Ok(remote_marketplaces) => {
                    for remote_marketplace in remote_marketplaces
                        .into_iter()
                        .map(remote_marketplace_to_info)
                    {
                        if let Some(existing) = data
                            .iter_mut()
                            .find(|marketplace| marketplace.name == remote_marketplace.name)
                        {
                            *existing = remote_marketplace;
                        } else {
                            data.push(remote_marketplace);
                        }
                    }
                }
                Err(
                    RemotePluginCatalogError::AuthRequired
                    | RemotePluginCatalogError::UnsupportedAuthMode,
                ) => {}
                Err(err) => {
                    warn!(
                        error = %err,
                        "plugin/list remote plugin catalog fetch failed; returning local marketplaces only"
                    );
                }
            }
        }

        let featured_plugin_ids = if data
            .iter()
            .any(|marketplace| marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME)
        {
            match plugins_manager
                .featured_plugin_ids_for_config(&config, auth.as_ref())
                .await
            {
                Ok(featured_plugin_ids) => featured_plugin_ids,
                Err(err) => {
                    warn!(
                        error = %err,
                        "plugin/list featured plugin fetch failed; returning empty featured ids"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Ok(PluginListResponse {
            marketplaces: data,
            marketplace_load_errors,
            featured_plugin_ids,
        })
    }

    pub(super) async fn plugin_read(
        &self,
        request_id: ConnectionRequestId,
        params: PluginReadParams,
    ) {
        let result = self.plugin_read_response(params).await;
        self.outgoing.send_result(request_id, result).await;
    }

    async fn plugin_read_response(
        &self,
        params: PluginReadParams,
    ) -> Result<PluginReadResponse, JSONRPCErrorError> {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginReadParams {
            marketplace_path,
            remote_marketplace_name,
            plugin_name,
        } = params;
        let read_source = match (marketplace_path, remote_marketplace_name) {
            (Some(marketplace_path), None) => Ok(marketplace_path),
            (None, Some(remote_marketplace_name)) => Err(remote_marketplace_name),
            (Some(_), Some(_)) | (None, None) => {
                return Err(invalid_request(
                    "plugin/read requires exactly one of marketplacePath or remoteMarketplaceName",
                ));
            }
        };
        let config_cwd = read_source.as_ref().ok().and_then(|marketplace_path| {
            marketplace_path.as_path().parent().map(Path::to_path_buf)
        });

        let config = self.load_latest_config(config_cwd).await?;

        let plugin = match read_source {
            Ok(marketplace_path) => {
                let request = PluginReadRequest {
                    plugin_name,
                    marketplace_path,
                };
                let outcome = plugins_manager
                    .read_plugin_for_config(&config, &request)
                    .await
                    .map_err(|err| Self::marketplace_error(err, "read plugin details"))?;
                let environment_manager = self.thread_manager.environment_manager();
                let app_summaries = plugin_app_helpers::load_plugin_app_summaries(
                    &config,
                    &outcome.plugin.apps,
                    &environment_manager,
                )
                .await;
                let visible_skills = outcome
                    .plugin
                    .skills
                    .iter()
                    .filter(|skill| {
                        skill.matches_product_restriction_for_product(
                            self.thread_manager.session_source().restriction_product(),
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                PluginDetail {
                    marketplace_name: outcome.marketplace_name,
                    marketplace_path: outcome.marketplace_path,
                    summary: PluginSummary {
                        id: outcome.plugin.id,
                        name: outcome.plugin.name,
                        source: marketplace_plugin_source_to_info(outcome.plugin.source),
                        installed: outcome.plugin.installed,
                        enabled: outcome.plugin.enabled,
                        install_policy: outcome.plugin.policy.installation.into(),
                        auth_policy: outcome.plugin.policy.authentication.into(),
                        interface: outcome.plugin.interface.map(local_plugin_interface_to_info),
                    },
                    description: outcome.plugin.description,
                    skills: plugin_skills_to_info(
                        &visible_skills,
                        &outcome.plugin.disabled_skill_paths,
                    ),
                    apps: app_summaries,
                    mcp_servers: outcome.plugin.mcp_server_names,
                }
            }
            Err(remote_marketplace_name) => {
                if !config.features.enabled(Feature::Plugins)
                    || !config.features.enabled(Feature::RemotePlugin)
                {
                    return Err(invalid_request(format!(
                        "remote plugin read is not enabled for marketplace {remote_marketplace_name}"
                    )));
                }
                let auth = self.auth_manager.auth().await;
                let remote_plugin_service_config = RemotePluginServiceConfig {
                    chatgpt_base_url: config.chatgpt_base_url.clone(),
                };
                if plugin_name.is_empty()
                    || !plugin_name
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '~')
                {
                    return Err(invalid_request(
                        "invalid remote plugin id: only ASCII letters, digits, `_`, `-`, and `~` are allowed",
                    ));
                }
                let remote_detail = codex_core_plugins::remote::fetch_remote_plugin_detail(
                    &remote_plugin_service_config,
                    auth.as_ref(),
                    &remote_marketplace_name,
                    &plugin_name,
                )
                .await
                .map_err(|err| {
                    remote_plugin_catalog_error_to_jsonrpc(err, "read remote plugin details")
                })?;
                let plugin_apps = remote_detail
                    .app_ids
                    .iter()
                    .cloned()
                    .map(codex_core::plugins::AppConnectorId)
                    .collect::<Vec<_>>();
                let environment_manager = self.thread_manager.environment_manager();
                let app_summaries = plugin_app_helpers::load_plugin_app_summaries(
                    &config,
                    &plugin_apps,
                    &environment_manager,
                )
                .await;
                remote_plugin_detail_to_info(remote_detail, app_summaries)
            }
        };

        Ok(PluginReadResponse { plugin })
    }

    pub(super) async fn plugin_install(
        &self,
        request_id: ConnectionRequestId,
        params: PluginInstallParams,
    ) {
        let result = self.plugin_install_response(params).await;
        self.outgoing.send_result(request_id, result).await;
    }

    async fn plugin_install_response(
        &self,
        params: PluginInstallParams,
    ) -> Result<PluginInstallResponse, JSONRPCErrorError> {
        let PluginInstallParams {
            marketplace_path,
            remote_marketplace_name,
            plugin_name,
        } = params;
        let marketplace_path = match (marketplace_path, remote_marketplace_name) {
            (Some(marketplace_path), None) => marketplace_path,
            (None, Some(remote_marketplace_name)) => {
                return self
                    .remote_plugin_install_response(remote_marketplace_name, plugin_name)
                    .await;
            }
            (Some(_), Some(_)) | (None, None) => {
                return Err(invalid_request(
                    "plugin/install requires exactly one of marketplacePath or remoteMarketplaceName",
                ));
            }
        };
        let config_cwd = marketplace_path.as_path().parent().map(Path::to_path_buf);
        let config = self.load_latest_config(config_cwd.clone()).await?;
        let auth = self.auth_manager.auth().await;

        if !self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await
        {
            return Err(invalid_request(
                "Codex plugins are disabled for this workspace",
            ));
        }

        let plugins_manager = self.thread_manager.plugins_manager();
        let request = PluginInstallRequest {
            plugin_name,
            marketplace_path,
        };

        let result = plugins_manager
            .install_plugin(request)
            .await
            .map_err(Self::plugin_install_error)?;
        let config = match self.load_latest_config(config_cwd).await {
            Ok(config) => config,
            Err(err) => {
                warn!(
                    "failed to reload config after plugin install, using current config: {err:?}"
                );
                config
            }
        };

        self.clear_plugin_related_caches();

        let plugin_mcp_servers = load_plugin_mcp_servers(result.installed_path.as_path()).await;

        if !plugin_mcp_servers.is_empty() {
            if let Err(err) = self.queue_mcp_server_refresh_for_config(&config).await {
                warn!(
                    plugin = result.plugin_id.as_key(),
                    "failed to queue MCP refresh after plugin install: {err:?}"
                );
            }
            self.start_plugin_mcp_oauth_logins(&config, plugin_mcp_servers)
                .await;
        }

        let plugin_apps = load_plugin_apps(result.installed_path.as_path()).await;
        let auth = self.auth_manager.auth().await;
        let apps_needing_auth = self
            .plugin_apps_needing_auth_for_install(
                &config,
                auth.as_ref().is_some_and(CodexAuth::is_chatgpt_auth),
                &result.plugin_id.as_key(),
                &plugin_apps,
            )
            .await;

        Ok(PluginInstallResponse {
            auth_policy: result.auth_policy.into(),
            apps_needing_auth,
        })
    }

    async fn remote_plugin_install_response(
        &self,
        remote_marketplace_name: String,
        plugin_name: String,
    ) -> Result<PluginInstallResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins)
            || !config.features.enabled(Feature::RemotePlugin)
        {
            return Err(invalid_request(format!(
                "remote plugin install is not enabled for marketplace {remote_marketplace_name}"
            )));
        }
        if plugin_name.is_empty() || !is_valid_remote_plugin_id(&plugin_name) {
            return Err(invalid_request(
                "invalid remote plugin id: only ASCII letters, digits, `_`, `-`, and `~` are allowed",
            ));
        }

        let auth = self.auth_manager.auth().await;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let remote_detail =
            codex_core_plugins::remote::fetch_remote_plugin_detail_with_download_urls(
                &remote_plugin_service_config,
                auth.as_ref(),
                &remote_marketplace_name,
                &plugin_name,
            )
            .await
            .map_err(|err| {
                remote_plugin_catalog_error_to_jsonrpc(
                    err,
                    "read remote plugin details before install",
                )
            })?;
        if remote_detail.summary.install_policy == PluginInstallPolicy::NotAvailable {
            return Err(invalid_request(format!(
                "remote plugin {plugin_name} is not available for install"
            )));
        }
        let validated_bundle = codex_core_plugins::remote_bundle::validate_remote_plugin_bundle(
            &plugin_name,
            &remote_marketplace_name,
            &remote_detail.summary.name,
            remote_detail.release_version.as_deref(),
            remote_detail.bundle_download_url.as_deref(),
        )
        .map_err(remote_plugin_bundle_install_error_to_jsonrpc)?;

        let result = codex_core_plugins::remote_bundle::download_and_install_remote_plugin_bundle(
            config.codex_home.to_path_buf(),
            validated_bundle,
        )
        .await
        .map_err(remote_plugin_bundle_install_error_to_jsonrpc)?;

        // Cache first so a backend install cannot succeed when local materialization fails.
        // If this backend call fails, the cache entry is harmless because remote installed state
        // is still backend-gated.
        codex_core_plugins::remote::install_remote_plugin(
            &remote_plugin_service_config,
            auth.as_ref(),
            &remote_marketplace_name,
            &plugin_name,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "install remote plugin"))?;

        // TODO(remote plugins): remote marketplaces do not yet have a local
        // marketplace/read-path sync, so this install path reads MCP/apps directly
        // from the just-cached bundle.
        self.clear_plugin_related_caches();

        let plugin_mcp_servers = load_plugin_mcp_servers(result.installed_path.as_path()).await;
        if !plugin_mcp_servers.is_empty() {
            if let Err(err) = self.queue_mcp_server_refresh_for_config(&config).await {
                warn!(
                    plugin = result.plugin_id.as_key(),
                    "failed to queue MCP refresh after remote plugin install: {err:?}"
                );
            }
            self.start_plugin_mcp_oauth_logins(&config, plugin_mcp_servers)
                .await;
        }

        let plugin_apps = load_plugin_apps(result.installed_path.as_path()).await;
        let apps_needing_auth = self
            .plugin_apps_needing_auth_for_install(
                &config,
                auth.as_ref().is_some_and(CodexAuth::is_chatgpt_auth),
                &result.plugin_id.as_key(),
                &plugin_apps,
            )
            .await;

        Ok(PluginInstallResponse {
            auth_policy: remote_detail.summary.auth_policy,
            apps_needing_auth,
        })
    }

    async fn plugin_apps_needing_auth_for_install(
        &self,
        config: &Config,
        is_chatgpt_auth: bool,
        plugin_id: &str,
        plugin_apps: &[codex_core::plugins::AppConnectorId],
    ) -> Vec<AppSummary> {
        if plugin_apps.is_empty() || !config.features.apps_enabled_for_auth(is_chatgpt_auth) {
            return Vec::new();
        }

        let environment_manager = self.thread_manager.environment_manager();
        let (all_connectors_result, accessible_connectors_result) = tokio::join!(
            connectors::list_all_connectors_with_options(config, /*force_refetch*/ true),
            connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager(
                config,
                /*force_refetch*/ true,
                &environment_manager
            ),
        );

        let all_connectors = match all_connectors_result {
            Ok(connectors) => connectors,
            Err(err) => {
                warn!(
                    plugin = plugin_id,
                    "failed to load app metadata after plugin install: {err:#}"
                );
                connectors::list_cached_all_connectors(config)
                    .await
                    .unwrap_or_default()
            }
        };
        let all_connectors = connectors::connectors_for_plugin_apps(all_connectors, plugin_apps);
        let (accessible_connectors, codex_apps_ready) = match accessible_connectors_result {
            Ok(status) => (status.connectors, status.codex_apps_ready),
            Err(err) => {
                warn!(
                    plugin = plugin_id,
                    "failed to load accessible apps after plugin install: {err:#}"
                );
                (
                    connectors::list_cached_accessible_connectors_from_mcp_tools(config)
                        .await
                        .unwrap_or_default(),
                    false,
                )
            }
        };
        if !codex_apps_ready {
            warn!(
                plugin = plugin_id,
                "codex_apps MCP not ready after plugin install; skipping appsNeedingAuth check"
            );
        }

        plugin_app_helpers::plugin_apps_needing_auth(
            &all_connectors,
            &accessible_connectors,
            plugin_apps,
            codex_apps_ready,
        )
    }

    pub(super) async fn plugin_uninstall(
        &self,
        request_id: ConnectionRequestId,
        params: PluginUninstallParams,
    ) {
        let result = self.plugin_uninstall_response(params).await;
        self.outgoing.send_result(request_id, result).await;
    }

    async fn plugin_uninstall_response(
        &self,
        params: PluginUninstallParams,
    ) -> Result<PluginUninstallResponse, JSONRPCErrorError> {
        let PluginUninstallParams { plugin_id } = params;
        if codex_core::plugins::PluginId::parse(&plugin_id).is_err()
            && !is_valid_remote_uninstall_plugin_id(&plugin_id)
        {
            return Err(invalid_request(
                "invalid plugin id: expected a local plugin id in the form `plugin@marketplace` or a remote plugin id starting with `plugins~`, `app_`, `asdk_app_`, or `connector_`",
            ));
        }
        if is_valid_remote_uninstall_plugin_id(&plugin_id) {
            return self.remote_plugin_uninstall_response(plugin_id).await;
        }
        let plugins_manager = self.thread_manager.plugins_manager();

        plugins_manager
            .uninstall_plugin(plugin_id)
            .await
            .map_err(Self::plugin_uninstall_error)?;
        self.clear_plugin_related_caches();
        Ok(PluginUninstallResponse {})
    }

    fn plugin_install_error(err: CorePluginInstallError) -> JSONRPCErrorError {
        if err.is_invalid_request() {
            return invalid_request(err.to_string());
        }

        match err {
            CorePluginInstallError::Marketplace(err) => {
                Self::marketplace_error(err, "install plugin")
            }
            CorePluginInstallError::Config(err) => {
                internal_error(format!("failed to persist installed plugin config: {err}"))
            }
            CorePluginInstallError::Remote(err) => {
                internal_error(format!("failed to enable remote plugin: {err}"))
            }
            CorePluginInstallError::Join(err) => {
                internal_error(format!("failed to install plugin: {err}"))
            }
            CorePluginInstallError::Store(err) => {
                internal_error(format!("failed to install plugin: {err}"))
            }
        }
    }

    fn plugin_uninstall_error(err: CorePluginUninstallError) -> JSONRPCErrorError {
        if err.is_invalid_request() {
            return invalid_request(err.to_string());
        }

        match err {
            CorePluginUninstallError::Config(err) => {
                internal_error(format!("failed to clear plugin config: {err}"))
            }
            CorePluginUninstallError::Remote(err) => {
                internal_error(format!("failed to uninstall remote plugin: {err}"))
            }
            CorePluginUninstallError::Join(err) => {
                internal_error(format!("failed to uninstall plugin: {err}"))
            }
            CorePluginUninstallError::Store(err) => {
                internal_error(format!("failed to uninstall plugin: {err}"))
            }
            CorePluginUninstallError::InvalidPluginId(_) => {
                unreachable!("invalid plugin ids are handled above");
            }
        }
    }

    fn marketplace_error(err: MarketplaceError, action: &str) -> JSONRPCErrorError {
        match err {
            MarketplaceError::MarketplaceNotFound { .. }
            | MarketplaceError::InvalidMarketplaceFile { .. }
            | MarketplaceError::PluginNotFound { .. }
            | MarketplaceError::PluginNotAvailable { .. }
            | MarketplaceError::PluginsDisabled
            | MarketplaceError::InvalidPlugin(_) => invalid_request(err.to_string()),
            MarketplaceError::Io { .. } => internal_error(format!("failed to {action}: {err}")),
        }
    }

    async fn remote_plugin_uninstall_response(
        &self,
        plugin_id: String,
    ) -> Result<PluginUninstallResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins)
            || !config.features.enabled(Feature::RemotePlugin)
        {
            return Err(invalid_request("remote plugin uninstall is not enabled"));
        }
        if plugin_id.is_empty() || !is_valid_remote_plugin_id(&plugin_id) {
            return Err(invalid_request(
                "invalid remote plugin id: only ASCII letters, digits, `_`, `-`, and `~` are allowed",
            ));
        }

        let auth = self.auth_manager.auth().await;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        codex_core_plugins::remote::uninstall_remote_plugin(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.to_path_buf(),
            &plugin_id,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "uninstall remote plugin"))?;

        self.clear_plugin_related_caches();
        Ok(PluginUninstallResponse {})
    }
}

fn is_valid_remote_plugin_id(plugin_name: &str) -> bool {
    plugin_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '~')
}

fn is_valid_remote_uninstall_plugin_id(plugin_name: &str) -> bool {
    !plugin_name.is_empty()
        && is_valid_remote_plugin_id(plugin_name)
        && (plugin_name.starts_with("plugins~")
            || plugin_name.starts_with("app_")
            || plugin_name.starts_with("asdk_app_")
            || plugin_name.starts_with("connector_"))
}

fn remote_marketplace_to_info(marketplace: RemoteMarketplace) -> PluginMarketplaceEntry {
    PluginMarketplaceEntry {
        name: marketplace.name,
        path: None,
        interface: Some(MarketplaceInterface {
            display_name: Some(marketplace.display_name),
        }),
        plugins: marketplace
            .plugins
            .into_iter()
            .map(remote_plugin_summary_to_info)
            .collect(),
    }
}

fn remote_plugin_summary_to_info(summary: RemoteCatalogPluginSummary) -> PluginSummary {
    PluginSummary {
        id: summary.id,
        name: summary.name,
        source: PluginSource::Remote,
        installed: summary.installed,
        enabled: summary.enabled,
        install_policy: summary.install_policy,
        auth_policy: summary.auth_policy,
        interface: summary.interface,
    }
}

fn remote_plugin_detail_to_info(
    detail: RemoteCatalogPluginDetail,
    apps: Vec<AppSummary>,
) -> PluginDetail {
    PluginDetail {
        marketplace_name: detail.marketplace_name,
        marketplace_path: None,
        summary: remote_plugin_summary_to_info(detail.summary),
        description: detail.description,
        skills: detail
            .skills
            .into_iter()
            .map(|skill| SkillSummary {
                name: skill.name,
                description: skill.description,
                short_description: skill.short_description,
                interface: skill.interface,
                path: None,
                enabled: skill.enabled,
            })
            .collect(),
        apps,
        mcp_servers: Vec::new(),
    }
}

fn remote_plugin_catalog_error_to_jsonrpc(
    err: RemotePluginCatalogError,
    context: &str,
) -> JSONRPCErrorError {
    match err {
        RemotePluginCatalogError::AuthRequired | RemotePluginCatalogError::UnsupportedAuthMode => {
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("{context}: {err}"),
                data: None,
            }
        }
        RemotePluginCatalogError::UnknownMarketplace { .. }
        | RemotePluginCatalogError::MarketplaceMismatch { .. } => JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("{context}: {err}"),
            data: None,
        },
        RemotePluginCatalogError::UnexpectedStatus { status, .. } if status.as_u16() == 404 => {
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: format!("{context}: {err}"),
                data: None,
            }
        }
        RemotePluginCatalogError::AuthToken(_)
        | RemotePluginCatalogError::Request { .. }
        | RemotePluginCatalogError::UnexpectedStatus { .. }
        | RemotePluginCatalogError::Decode { .. }
        | RemotePluginCatalogError::UnexpectedPluginId { .. }
        | RemotePluginCatalogError::UnexpectedEnabledState { .. }
        | RemotePluginCatalogError::CacheRemove(_) => JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("{context}: {err}"),
            data: None,
        },
    }
}

fn remote_plugin_bundle_install_error_to_jsonrpc(
    err: codex_core_plugins::remote_bundle::RemotePluginBundleInstallError,
) -> JSONRPCErrorError {
    internal_error(format!("install remote plugin bundle: {err}"))
}
