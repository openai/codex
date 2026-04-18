use super::*;

impl CodexMessageProcessor {
    pub(super) async fn plugin_list(
        &self,
        request_id: ConnectionRequestId,
        params: PluginListParams,
    ) {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginListParams { cwds } = params;
        let roots = cwds.unwrap_or_default();
        plugins_manager.maybe_start_non_curated_plugin_cache_refresh(&roots);

        let config = match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => config,
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
                return;
            }
        };
        let auth = self.auth_manager.auth().await;

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
            Ok(Err(err)) => {
                self.send_marketplace_error(request_id, err, "list marketplace plugins")
                    .await;
                return;
            }
            Err(err) => {
                self.send_internal_error(
                    request_id,
                    format!("failed to list marketplace plugins: {err}"),
                )
                .await;
                return;
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
                Ok(remote_marketplaces) => data.extend(
                    remote_marketplaces
                        .into_iter()
                        .map(remote_marketplace_to_info),
                ),
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

        self.outgoing
            .send_response(
                request_id,
                PluginListResponse {
                    marketplaces: data,
                    marketplace_load_errors,
                    featured_plugin_ids,
                },
            )
            .await;
    }

    pub(super) async fn plugin_read(
        &self,
        request_id: ConnectionRequestId,
        params: PluginReadParams,
    ) {
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
                self.outgoing
                    .send_error(
                        request_id,
                        JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: "plugin/read requires exactly one of marketplacePath or remoteMarketplaceName".to_string(),
                            data: None,
                        },
                    )
                    .await;
                return;
            }
        };
        let config_cwd = read_source.as_ref().ok().and_then(|marketplace_path| {
            marketplace_path.as_path().parent().map(Path::to_path_buf)
        });

        let config = match self.load_latest_config(config_cwd).await {
            Ok(config) => config,
            Err(err) => {
                self.outgoing.send_error(request_id, err).await;
                return;
            }
        };

        let plugin = match read_source {
            Ok(marketplace_path) => {
                let request = PluginReadRequest {
                    plugin_name,
                    marketplace_path,
                };
                let outcome = match plugins_manager
                    .read_plugin_for_config(&config, &request)
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(err) => {
                        self.send_marketplace_error(request_id, err, "read plugin details")
                            .await;
                        return;
                    }
                };
                let app_summaries =
                    plugin_app_helpers::load_plugin_app_summaries(&config, &outcome.plugin.apps)
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
                if !config.features.enabled(Feature::RemotePlugin) {
                    self.outgoing
                        .send_error(
                            request_id,
                            JSONRPCErrorError {
                                code: INVALID_REQUEST_ERROR_CODE,
                                message: format!(
                                    "remote plugin read is not enabled for marketplace {remote_marketplace_name}"
                                ),
                                data: None,
                            },
                        )
                        .await;
                    return;
                }
                let auth = self.auth_manager.auth().await;
                let remote_plugin_service_config = RemotePluginServiceConfig {
                    chatgpt_base_url: config.chatgpt_base_url.clone(),
                };
                let remote_plugin_id = format!("{plugin_name}@{remote_marketplace_name}");
                let remote_detail = match codex_core_plugins::remote::fetch_remote_plugin_detail(
                    &remote_plugin_service_config,
                    auth.as_ref(),
                    &remote_marketplace_name,
                    &remote_plugin_id,
                )
                .await
                {
                    Ok(remote_detail) => remote_detail,
                    Err(err) => {
                        self.outgoing
                            .send_error(
                                request_id,
                                remote_plugin_catalog_error_to_jsonrpc(
                                    err,
                                    "read remote plugin details",
                                ),
                            )
                            .await;
                        return;
                    }
                };
                let plugin_apps = remote_detail
                    .app_ids
                    .iter()
                    .cloned()
                    .map(codex_core::plugins::AppConnectorId)
                    .collect::<Vec<_>>();
                let app_summaries =
                    plugin_app_helpers::load_plugin_app_summaries(&config, &plugin_apps).await;
                remote_plugin_detail_to_info(remote_detail, app_summaries)
            }
        };

        self.outgoing
            .send_response(request_id, PluginReadResponse { plugin })
            .await;
    }

    pub(super) async fn plugin_install(
        &self,
        request_id: ConnectionRequestId,
        params: PluginInstallParams,
    ) {
        let PluginInstallParams {
            marketplace_path,
            remote_marketplace_name,
            plugin_name,
        } = params;
        let marketplace_path = match (marketplace_path, remote_marketplace_name) {
            (Some(marketplace_path), None) => marketplace_path,
            (None, Some(remote_marketplace_name)) => {
                self.outgoing
                    .send_error(
                        request_id,
                        JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: format!(
                                "remote plugin install is not supported yet for marketplace {remote_marketplace_name}"
                            ),
                            data: None,
                        },
                    )
                    .await;
                return;
            }
            (Some(_), Some(_)) | (None, None) => {
                self.outgoing
                    .send_error(
                        request_id,
                        JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: "plugin/install requires exactly one of marketplacePath or remoteMarketplaceName".to_string(),
                            data: None,
                        },
                    )
                    .await;
                return;
            }
        };
        let config_cwd = marketplace_path.as_path().parent().map(Path::to_path_buf);

        let plugins_manager = self.thread_manager.plugins_manager();
        let request = PluginInstallRequest {
            plugin_name,
            marketplace_path,
        };

        let install_result = plugins_manager.install_plugin(request).await;

        match install_result {
            Ok(result) => {
                let config = match self.load_latest_config(config_cwd).await {
                    Ok(config) => config,
                    Err(err) => {
                        warn!(
                            "failed to reload config after plugin install, using current config: {err:?}"
                        );
                        self.config.as_ref().clone()
                    }
                };

                self.clear_plugin_related_caches();

                let plugin_mcp_servers =
                    load_plugin_mcp_servers(result.installed_path.as_path()).await;

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
                let apps_needing_auth = if plugin_apps.is_empty()
                    || !config.features.apps_enabled_for_auth(
                        auth.as_ref().is_some_and(CodexAuth::is_chatgpt_auth),
                    ) {
                    Vec::new()
                } else {
                    let (all_connectors_result, accessible_connectors_result) = tokio::join!(
                        connectors::list_all_connectors_with_options(&config, /*force_refetch*/ true),
                        connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status(
                            &config, /*force_refetch*/ true
                        ),
                    );

                    let all_connectors = match all_connectors_result {
                        Ok(connectors) => connectors,
                        Err(err) => {
                            warn!(
                                plugin = result.plugin_id.as_key(),
                                "failed to load app metadata after plugin install: {err:#}"
                            );
                            connectors::list_cached_all_connectors(&config)
                                .await
                                .unwrap_or_default()
                        }
                    };
                    let all_connectors =
                        connectors::connectors_for_plugin_apps(all_connectors, &plugin_apps);
                    let (accessible_connectors, codex_apps_ready) =
                        match accessible_connectors_result {
                            Ok(status) => (status.connectors, status.codex_apps_ready),
                            Err(err) => {
                                warn!(
                                    plugin = result.plugin_id.as_key(),
                                    "failed to load accessible apps after plugin install: {err:#}"
                                );
                                (
                                    connectors::list_cached_accessible_connectors_from_mcp_tools(
                                        &config,
                                    )
                                    .await
                                    .unwrap_or_default(),
                                    false,
                                )
                            }
                        };
                    if !codex_apps_ready {
                        warn!(
                            plugin = result.plugin_id.as_key(),
                            "codex_apps MCP not ready after plugin install; skipping appsNeedingAuth check"
                        );
                    }

                    plugin_app_helpers::plugin_apps_needing_auth(
                        &all_connectors,
                        &accessible_connectors,
                        &plugin_apps,
                        codex_apps_ready,
                    )
                };

                self.outgoing
                    .send_response(
                        request_id,
                        PluginInstallResponse {
                            auth_policy: result.auth_policy.into(),
                            apps_needing_auth,
                        },
                    )
                    .await;
            }
            Err(err) => {
                if err.is_invalid_request() {
                    self.send_invalid_request_error(request_id, err.to_string())
                        .await;
                    return;
                }

                match err {
                    CorePluginInstallError::Marketplace(err) => {
                        self.send_marketplace_error(request_id, err, "install plugin")
                            .await;
                    }
                    CorePluginInstallError::Config(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to persist installed plugin config: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Remote(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to enable remote plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Join(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to install plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginInstallError::Store(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to install plugin: {err}"),
                        )
                        .await;
                    }
                }
            }
        }
    }

    pub(super) async fn plugin_uninstall(
        &self,
        request_id: ConnectionRequestId,
        params: PluginUninstallParams,
    ) {
        let PluginUninstallParams { plugin_id } = params;
        let plugins_manager = self.thread_manager.plugins_manager();

        let uninstall_result = plugins_manager.uninstall_plugin(plugin_id).await;

        match uninstall_result {
            Ok(()) => {
                self.clear_plugin_related_caches();
                self.outgoing
                    .send_response(request_id, PluginUninstallResponse {})
                    .await;
            }
            Err(err) => {
                if err.is_invalid_request() {
                    self.send_invalid_request_error(request_id, err.to_string())
                        .await;
                    return;
                }

                match err {
                    CorePluginUninstallError::Config(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to clear plugin config: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Remote(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall remote plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Join(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::Store(err) => {
                        self.send_internal_error(
                            request_id,
                            format!("failed to uninstall plugin: {err}"),
                        )
                        .await;
                    }
                    CorePluginUninstallError::InvalidPluginId(_) => {
                        unreachable!("invalid plugin ids are handled above");
                    }
                }
            }
        }
    }
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
        RemotePluginCatalogError::AuthToken(_)
        | RemotePluginCatalogError::Request { .. }
        | RemotePluginCatalogError::UnexpectedStatus { .. }
        | RemotePluginCatalogError::Decode { .. } => JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            message: format!("{context}: {err}"),
            data: None,
        },
    }
}
