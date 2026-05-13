use super::*;
use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginSharePrincipalRole;
use codex_app_server_protocol::PluginShareTargetRole;
use codex_config::types::McpServerConfig;
use codex_core_plugins::remote::is_valid_remote_plugin_id;
use codex_core_plugins::remote::validate_remote_plugin_id;
use codex_mcp::McpOAuthLoginSupport;
use codex_mcp::oauth_login_support;
use codex_mcp::should_retry_without_scopes;
use codex_rmcp_client::perform_oauth_login_silent;

#[derive(Clone)]
pub(crate) struct PluginRequestProcessor {
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    analytics_events_client: AnalyticsEventsClient,
    config_manager: ConfigManager,
    workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
}

fn plugin_skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
    disabled_skill_paths: &HashSet<AbsolutePathBuf>,
) -> Vec<SkillSummary> {
    skills
        .iter()
        .map(|skill| SkillSummary {
            name: skill.name.clone(),
            description: skill.description.clone(),
            short_description: skill.short_description.clone(),
            interface: skill.interface.clone().map(|interface| {
                codex_app_server_protocol::SkillInterface {
                    display_name: interface.display_name,
                    short_description: interface.short_description,
                    icon_small: interface.icon_small,
                    icon_large: interface.icon_large,
                    brand_color: interface.brand_color,
                    default_prompt: interface.default_prompt,
                }
            }),
            path: Some(skill.path_to_skills_md.clone()),
            enabled: !disabled_skill_paths.contains(&skill.path_to_skills_md),
        })
        .collect()
}

fn local_plugin_interface_to_info(interface: PluginManifestInterface) -> PluginInterface {
    PluginInterface {
        display_name: interface.display_name,
        short_description: interface.short_description,
        long_description: interface.long_description,
        developer_name: interface.developer_name,
        category: interface.category,
        capabilities: interface.capabilities,
        website_url: interface.website_url,
        privacy_policy_url: interface.privacy_policy_url,
        terms_of_service_url: interface.terms_of_service_url,
        default_prompt: interface.default_prompt,
        brand_color: interface.brand_color,
        composer_icon: interface.composer_icon,
        composer_icon_url: None,
        logo: interface.logo,
        logo_url: None,
        screenshots: interface.screenshots,
        screenshot_urls: Vec::new(),
    }
}

fn marketplace_plugin_source_to_info(source: MarketplacePluginSource) -> PluginSource {
    match source {
        MarketplacePluginSource::Local { path } => PluginSource::Local { path },
        MarketplacePluginSource::Git {
            url,
            path,
            ref_name,
            sha,
        } => PluginSource::Git {
            url,
            path,
            ref_name,
            sha,
        },
    }
}

fn load_shared_plugin_ids_by_local_path(
    config: &Config,
) -> Result<std::collections::BTreeMap<AbsolutePathBuf, String>, JSONRPCErrorError> {
    codex_core_plugins::remote::load_plugin_share_remote_ids_by_local_path(
        config.codex_home.as_path(),
    )
    .map_err(|err| {
        internal_error(format!(
            "failed to load plugin share local path mapping: {err}"
        ))
    })
}

fn share_context_for_source(
    source: &MarketplacePluginSource,
    shared_plugin_ids_by_local_path: &std::collections::BTreeMap<AbsolutePathBuf, String>,
) -> Option<PluginShareContext> {
    match source {
        MarketplacePluginSource::Local { path } => shared_plugin_ids_by_local_path
            .get(path)
            .cloned()
            .map(|remote_plugin_id| PluginShareContext {
                remote_plugin_id,
                remote_version: None,
                discoverability: None,
                share_url: None,
                creator_account_user_id: None,
                creator_name: None,
                share_principals: None,
            }),
        MarketplacePluginSource::Git { .. } => None,
    }
}

fn remote_plugin_share_discoverability(
    discoverability: PluginShareDiscoverability,
) -> codex_core_plugins::remote::RemotePluginShareDiscoverability {
    match discoverability {
        PluginShareDiscoverability::Listed => {
            codex_core_plugins::remote::RemotePluginShareDiscoverability::Listed
        }
        PluginShareDiscoverability::Unlisted => {
            codex_core_plugins::remote::RemotePluginShareDiscoverability::Unlisted
        }
        PluginShareDiscoverability::Private => {
            codex_core_plugins::remote::RemotePluginShareDiscoverability::Private
        }
    }
}

fn remote_plugin_share_update_discoverability(
    discoverability: PluginShareUpdateDiscoverability,
) -> codex_core_plugins::remote::RemotePluginShareUpdateDiscoverability {
    match discoverability {
        PluginShareUpdateDiscoverability::Unlisted => {
            codex_core_plugins::remote::RemotePluginShareUpdateDiscoverability::Unlisted
        }
        PluginShareUpdateDiscoverability::Private => {
            codex_core_plugins::remote::RemotePluginShareUpdateDiscoverability::Private
        }
    }
}

fn validate_client_plugin_share_targets(
    targets: &[PluginShareTarget],
) -> Result<(), JSONRPCErrorError> {
    if targets
        .iter()
        .any(|target| target.principal_type == PluginSharePrincipalType::Workspace)
    {
        return Err(invalid_request(
            "shareTargets cannot include workspace principals; use discoverability UNLISTED for workspace link access",
        ));
    }
    Ok(())
}

fn remote_plugin_share_target_role(
    role: PluginShareTargetRole,
) -> codex_core_plugins::remote::RemotePluginShareTargetRole {
    match role {
        PluginShareTargetRole::Reader => {
            codex_core_plugins::remote::RemotePluginShareTargetRole::Reader
        }
        PluginShareTargetRole::Editor => {
            codex_core_plugins::remote::RemotePluginShareTargetRole::Editor
        }
    }
}

fn plugin_share_principal_role_from_remote(
    role: codex_core_plugins::remote::RemotePluginSharePrincipalRole,
) -> PluginSharePrincipalRole {
    match role {
        codex_core_plugins::remote::RemotePluginSharePrincipalRole::Reader => {
            PluginSharePrincipalRole::Reader
        }
        codex_core_plugins::remote::RemotePluginSharePrincipalRole::Editor => {
            PluginSharePrincipalRole::Editor
        }
        codex_core_plugins::remote::RemotePluginSharePrincipalRole::Owner => {
            PluginSharePrincipalRole::Owner
        }
    }
}

fn remote_plugin_share_targets(
    targets: Vec<PluginShareTarget>,
) -> Vec<codex_core_plugins::remote::RemotePluginShareTarget> {
    targets
        .into_iter()
        .map(
            |target| codex_core_plugins::remote::RemotePluginShareTarget {
                principal_type: match target.principal_type {
                    PluginSharePrincipalType::User => {
                        codex_core_plugins::remote::RemotePluginSharePrincipalType::User
                    }
                    PluginSharePrincipalType::Group => {
                        codex_core_plugins::remote::RemotePluginSharePrincipalType::Group
                    }
                    PluginSharePrincipalType::Workspace => {
                        codex_core_plugins::remote::RemotePluginSharePrincipalType::Workspace
                    }
                },
                principal_id: target.principal_id,
                role: remote_plugin_share_target_role(target.role),
            },
        )
        .collect()
}

fn plugin_share_principal_from_remote(
    principal: codex_core_plugins::remote::RemotePluginSharePrincipal,
) -> PluginSharePrincipal {
    PluginSharePrincipal {
        principal_type: match principal.principal_type {
            codex_core_plugins::remote::RemotePluginSharePrincipalType::User => {
                PluginSharePrincipalType::User
            }
            codex_core_plugins::remote::RemotePluginSharePrincipalType::Group => {
                PluginSharePrincipalType::Group
            }
            codex_core_plugins::remote::RemotePluginSharePrincipalType::Workspace => {
                PluginSharePrincipalType::Workspace
            }
        },
        principal_id: principal.principal_id,
        role: plugin_share_principal_role_from_remote(principal.role),
        name: principal.name,
    }
}

impl PluginRequestProcessor {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        outgoing: Arc<OutgoingMessageSender>,
        analytics_events_client: AnalyticsEventsClient,
        config_manager: ConfigManager,
        workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            outgoing,
            analytics_events_client,
            config_manager,
            workspace_settings_cache,
        }
    }

    pub(crate) async fn plugin_list(
        &self,
        params: PluginListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_read(
        &self,
        params: PluginReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_read_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_skill_read(
        &self,
        params: PluginSkillReadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_skill_read_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_share_save(
        &self,
        params: PluginShareSaveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_share_save_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_share_update_targets(
        &self,
        params: PluginShareUpdateTargetsParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_share_update_targets_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_share_list(
        &self,
        params: PluginShareListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_share_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_share_checkout(
        &self,
        params: PluginShareCheckoutParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_share_checkout_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_share_delete(
        &self,
        params: PluginShareDeleteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_share_delete_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_install(
        &self,
        params: PluginInstallParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_install_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn plugin_uninstall(
        &self,
        params: PluginUninstallParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_uninstall_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) fn effective_plugins_changed_callback(&self) -> Arc<dyn Fn() + Send + Sync> {
        let thread_manager = Arc::clone(&self.thread_manager);
        let config_manager = self.config_manager.clone();
        Arc::new(move || {
            Self::spawn_effective_plugins_changed_task(
                Arc::clone(&thread_manager),
                config_manager.clone(),
            );
        })
    }

    fn on_effective_plugins_changed(&self) {
        Self::spawn_effective_plugins_changed_task(
            Arc::clone(&self.thread_manager),
            self.config_manager.clone(),
        );
    }

    fn spawn_effective_plugins_changed_task(
        thread_manager: Arc<ThreadManager>,
        config_manager: ConfigManager,
    ) {
        tokio::spawn(async move {
            thread_manager.plugins_manager().clear_cache();
            thread_manager.skills_manager().clear_cache();
            if thread_manager.list_thread_ids().await.is_empty() {
                return;
            }
            crate::mcp_refresh::queue_best_effort_refresh(&thread_manager, &config_manager).await;
        });
    }

    fn clear_plugin_related_caches(&self) {
        self.thread_manager.plugins_manager().clear_cache();
        self.thread_manager.skills_manager().clear_cache();
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

    async fn plugin_list_response(
        &self,
        params: PluginListParams,
    ) -> Result<PluginListResponse, JSONRPCErrorError> {
        let plugins_manager = self.thread_manager.plugins_manager();
        let PluginListParams {
            cwds,
            marketplace_kinds,
        } = params;
        let roots = cwds.unwrap_or_default();
        let explicit_marketplace_kinds = marketplace_kinds.is_some();
        let marketplace_kinds =
            marketplace_kinds.unwrap_or_else(|| vec![PluginListMarketplaceKind::Local]);
        let include_local = marketplace_kinds.contains(&PluginListMarketplaceKind::Local);

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
        let plugins_input = config.plugins_config_input();
        let (mut data, marketplace_load_errors) = if include_local {
            plugins_manager.maybe_start_plugin_list_background_tasks_for_config(
                &plugins_input,
                auth.clone(),
                &roots,
                Some(self.effective_plugins_changed_callback()),
            );

            let config_for_marketplace_listing = plugins_input.clone();
            let plugins_manager_for_marketplace_listing = plugins_manager.clone();
            let shared_plugin_ids_by_local_path = load_shared_plugin_ids_by_local_path(&config)?;
            match tokio::task::spawn_blocking(move || {
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
                            interface: marketplace.interface.map(|interface| {
                                MarketplaceInterface {
                                    display_name: interface.display_name,
                                }
                            }),
                            plugins: marketplace
                                .plugins
                                .into_iter()
                                .map(|plugin| {
                                    let share_context = share_context_for_source(
                                        &plugin.source,
                                        &shared_plugin_ids_by_local_path,
                                    );
                                    PluginSummary {
                                        id: plugin.id,
                                        remote_plugin_id: None,
                                        local_version: plugin.local_version,
                                        installed: plugin.installed,
                                        enabled: plugin.enabled,
                                        name: plugin.name,
                                        share_context,
                                        source: marketplace_plugin_source_to_info(plugin.source),
                                        install_policy: plugin.policy.installation.into(),
                                        auth_policy: plugin.policy.authentication.into(),
                                        availability: PluginAvailability::Available,
                                        interface: plugin
                                            .interface
                                            .map(local_plugin_interface_to_info),
                                        keywords: plugin.keywords,
                                    }
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
                    return Err(Self::marketplace_error(err, "list marketplace plugins"));
                }
                Err(err) => {
                    return Err(internal_error(format!(
                        "failed to list marketplace plugins: {err}"
                    )));
                }
            }
        } else {
            (Vec::new(), Vec::new())
        };

        let mut remote_sources = Vec::new();
        if !explicit_marketplace_kinds && config.features.enabled(Feature::RemotePlugin) {
            remote_sources.push(RemoteMarketplaceSource::Global);
        }
        if marketplace_kinds.contains(&PluginListMarketplaceKind::WorkspaceDirectory) {
            remote_sources.push(RemoteMarketplaceSource::WorkspaceDirectory);
        }
        if marketplace_kinds.contains(&PluginListMarketplaceKind::SharedWithMe)
            && config.features.enabled(Feature::PluginSharing)
        {
            remote_sources.push(RemoteMarketplaceSource::SharedWithMe);
        }
        if !remote_sources.is_empty() {
            let remote_plugin_service_config = RemotePluginServiceConfig {
                chatgpt_base_url: config.chatgpt_base_url.clone(),
            };
            match codex_core_plugins::remote::fetch_remote_marketplaces(
                &remote_plugin_service_config,
                auth.as_ref(),
                &remote_sources,
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
                    err @ (RemotePluginCatalogError::AuthRequired
                    | RemotePluginCatalogError::UnsupportedAuthMode),
                ) if explicit_marketplace_kinds => {
                    return Err(remote_plugin_catalog_error_to_jsonrpc(
                        err,
                        "list remote plugin catalog",
                    ));
                }
                Err(
                    RemotePluginCatalogError::AuthRequired
                    | RemotePluginCatalogError::UnsupportedAuthMode,
                ) => {}
                Err(err) if explicit_marketplace_kinds => {
                    return Err(remote_plugin_catalog_error_to_jsonrpc(
                        err,
                        "list remote plugin catalog",
                    ));
                }
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
                .featured_plugin_ids_for_config(&plugins_input, auth.as_ref())
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
        let plugins_input = config.plugins_config_input();

        let plugin = match read_source {
            Ok(marketplace_path) => {
                let request = PluginReadRequest {
                    plugin_name,
                    marketplace_path,
                };
                let outcome = plugins_manager
                    .read_plugin_for_config(&plugins_input, &request)
                    .await
                    .map_err(|err| Self::marketplace_error(err, "read plugin details"))?;
                let shared_plugin_ids_by_local_path =
                    load_shared_plugin_ids_by_local_path(&config)?;
                let share_context = share_context_for_source(
                    &outcome.plugin.source,
                    &shared_plugin_ids_by_local_path,
                );
                let share_context = match share_context {
                    Some(context) => {
                        let auth = self.auth_manager.auth().await;
                        let remote_plugin_service_config = RemotePluginServiceConfig {
                            chatgpt_base_url: config.chatgpt_base_url.clone(),
                        };
                        match codex_core_plugins::remote::fetch_remote_plugin_share_context(
                            &remote_plugin_service_config,
                            auth.as_ref(),
                            &context.remote_plugin_id,
                        )
                        .await
                        {
                            Ok(Some(remote_share_context)) => {
                                if remote_share_context.share_principals.is_some() {
                                    Some(remote_plugin_share_context_to_info(remote_share_context))
                                } else {
                                    let remote_version = remote_share_context.remote_version;
                                    let remote_plugin_id = context.remote_plugin_id.clone();
                                    warn!(
                                        remote_plugin_id = %remote_plugin_id,
                                        "remote shared plugin detail did not include share principals; returning local share mapping context with remote version"
                                    );
                                    Some(PluginShareContext {
                                        remote_version,
                                        ..context
                                    })
                                }
                            }
                            Ok(None) => {
                                warn!(
                                    remote_plugin_id = %context.remote_plugin_id,
                                    "remote shared plugin detail did not include share context; returning local share mapping context"
                                );
                                Some(context)
                            }
                            Err(err) => {
                                warn!(
                                    remote_plugin_id = %context.remote_plugin_id,
                                    error = %err,
                                    "failed to hydrate local plugin share context; returning local share mapping context"
                                );
                                Some(context)
                            }
                        }
                    }
                    None => None,
                };
                let environment_manager = self.thread_manager.environment_manager();
                let app_summaries =
                    load_plugin_app_summaries(&config, &outcome.plugin.apps, &environment_manager)
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
                        remote_plugin_id: None,
                        local_version: outcome.plugin.local_version,
                        name: outcome.plugin.name,
                        share_context,
                        source: marketplace_plugin_source_to_info(outcome.plugin.source),
                        installed: outcome.plugin.installed,
                        enabled: outcome.plugin.enabled,
                        install_policy: outcome.plugin.policy.installation.into(),
                        auth_policy: outcome.plugin.policy.authentication.into(),
                        availability: PluginAvailability::Available,
                        interface: outcome.plugin.interface.map(local_plugin_interface_to_info),
                        keywords: outcome.plugin.keywords,
                    },
                    description: outcome.plugin.description,
                    skills: plugin_skills_to_info(
                        &visible_skills,
                        &outcome.plugin.disabled_skill_paths,
                    ),
                    hooks: outcome
                        .plugin
                        .hooks
                        .into_iter()
                        .map(|hook| codex_app_server_protocol::PluginHookSummary {
                            key: hook.key,
                            event_name: hook.event_name.into(),
                        })
                        .collect(),
                    apps: app_summaries,
                    mcp_servers: outcome.plugin.mcp_server_names,
                }
            }
            Err(remote_marketplace_name) => {
                if !config.features.enabled(Feature::Plugins) {
                    return Err(invalid_request(format!(
                        "remote plugin read is not enabled for marketplace {remote_marketplace_name}"
                    )));
                }
                let auth = self.auth_manager.auth().await;
                let remote_plugin_service_config = RemotePluginServiceConfig {
                    chatgpt_base_url: config.chatgpt_base_url.clone(),
                };
                validate_remote_plugin_id(&plugin_name)?;
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
                    .map(codex_plugin::AppConnectorId)
                    .collect::<Vec<_>>();
                let environment_manager = self.thread_manager.environment_manager();
                let app_summaries =
                    load_plugin_app_summaries(&config, &plugin_apps, &environment_manager).await;
                remote_plugin_detail_to_info(remote_detail, app_summaries)
            }
        };

        Ok(PluginReadResponse { plugin })
    }

    async fn plugin_skill_read_response(
        &self,
        params: PluginSkillReadParams,
    ) -> Result<PluginSkillReadResponse, JSONRPCErrorError> {
        let PluginSkillReadParams {
            remote_marketplace_name,
            remote_plugin_id,
            skill_name,
        } = params;

        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins) {
            return Err(invalid_request(format!(
                "remote plugin skill read is not enabled for marketplace {remote_marketplace_name}"
            )));
        }
        validate_remote_plugin_id(&remote_plugin_id)?;
        if skill_name.is_empty() {
            return Err(invalid_request(
                "invalid remote plugin skill name: cannot be empty",
            ));
        }

        let auth = self.auth_manager.auth().await;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let remote_skill_detail = codex_core_plugins::remote::fetch_remote_plugin_skill_detail(
            &remote_plugin_service_config,
            auth.as_ref(),
            &remote_marketplace_name,
            &remote_plugin_id,
            &skill_name,
        )
        .await
        .map_err(|err| {
            remote_plugin_catalog_error_to_jsonrpc(err, "read remote plugin skill details")
        })?;

        Ok(PluginSkillReadResponse {
            contents: remote_skill_detail.contents,
        })
    }

    async fn plugin_share_save_response(
        &self,
        params: PluginShareSaveParams,
    ) -> Result<PluginShareSaveResponse, JSONRPCErrorError> {
        let (config, auth) = self.load_plugin_share_config_and_auth().await?;
        if !config.features.enabled(Feature::PluginSharing) {
            return Err(invalid_request("plugin sharing is disabled"));
        }
        let PluginShareSaveParams {
            plugin_path,
            remote_plugin_id,
            discoverability,
            share_targets,
        } = params;
        if let Some(remote_plugin_id) = remote_plugin_id.as_ref()
            && (remote_plugin_id.is_empty() || !is_valid_remote_plugin_id(remote_plugin_id))
        {
            return Err(invalid_request("invalid remote plugin id"));
        }
        if remote_plugin_id.is_some() && (discoverability.is_some() || share_targets.is_some()) {
            return Err(invalid_request(
                "discoverability and shareTargets are only supported when creating a plugin share; use plugin/share/updateTargets to update share settings",
            ));
        }
        if discoverability == Some(PluginShareDiscoverability::Listed) {
            return Err(invalid_request(
                "discoverability LISTED is not supported for plugin/share/save; use UNLISTED or PRIVATE",
            ));
        }
        if let Some(share_targets) = share_targets.as_ref() {
            validate_client_plugin_share_targets(share_targets)?;
        }

        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let access_policy = codex_core_plugins::remote::RemotePluginShareAccessPolicy {
            discoverability: discoverability.map(remote_plugin_share_discoverability),
            share_targets: share_targets.map(remote_plugin_share_targets),
        };
        let result = codex_core_plugins::remote::save_remote_plugin_share(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.as_path(),
            &plugin_path,
            remote_plugin_id.as_deref(),
            access_policy,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "save remote plugin share"))?;
        let remote_plugin_id = result.remote_plugin_id;
        self.clear_plugin_related_caches();
        Ok(PluginShareSaveResponse {
            remote_plugin_id,
            share_url: result.share_url.unwrap_or_default(),
        })
    }

    async fn plugin_share_update_targets_response(
        &self,
        params: PluginShareUpdateTargetsParams,
    ) -> Result<PluginShareUpdateTargetsResponse, JSONRPCErrorError> {
        let (config, auth) = self.load_plugin_share_config_and_auth().await?;
        let PluginShareUpdateTargetsParams {
            remote_plugin_id,
            discoverability,
            share_targets,
        } = params;
        if remote_plugin_id.is_empty() || !is_valid_remote_plugin_id(&remote_plugin_id) {
            return Err(invalid_request("invalid remote plugin id"));
        }
        validate_client_plugin_share_targets(&share_targets)?;

        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let result = codex_core_plugins::remote::update_remote_plugin_share_targets(
            &remote_plugin_service_config,
            auth.as_ref(),
            &remote_plugin_id,
            remote_plugin_share_targets(share_targets),
            remote_plugin_share_update_discoverability(discoverability),
        )
        .await
        .map_err(|err| {
            remote_plugin_catalog_error_to_jsonrpc(err, "update remote plugin share targets")
        })?;
        self.clear_plugin_related_caches();
        Ok(PluginShareUpdateTargetsResponse {
            principals: result
                .principals
                .into_iter()
                .map(plugin_share_principal_from_remote)
                .collect(),
            discoverability: remote_plugin_share_discoverability_to_info(result.discoverability),
        })
    }

    async fn plugin_share_list_response(
        &self,
        _params: PluginShareListParams,
    ) -> Result<PluginShareListResponse, JSONRPCErrorError> {
        let (config, auth) = self.load_plugin_share_config_and_auth().await?;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let data = codex_core_plugins::remote::list_remote_plugin_shares(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.as_path(),
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "list remote plugin shares"))?
        .into_iter()
        .map(|summary| {
            let RemoteCatalogPluginShareSummary {
                summary,
                local_plugin_path,
            } = summary;
            let plugin = remote_plugin_summary_to_info(summary);
            PluginShareListItem {
                plugin,
                local_plugin_path,
            }
        })
        .collect();
        Ok(PluginShareListResponse { data })
    }

    async fn plugin_share_checkout_response(
        &self,
        params: PluginShareCheckoutParams,
    ) -> Result<PluginShareCheckoutResponse, JSONRPCErrorError> {
        let (config, auth) = self.load_plugin_share_config_and_auth().await?;
        if !config.features.enabled(Feature::PluginSharing) {
            return Err(invalid_request("plugin sharing is disabled"));
        }
        let PluginShareCheckoutParams { remote_plugin_id } = params;
        if remote_plugin_id.is_empty() || !is_valid_remote_plugin_id(&remote_plugin_id) {
            return Err(invalid_request("invalid remote plugin id"));
        }

        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let result = codex_core_plugins::remote::checkout_remote_plugin_share(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.as_path(),
            &remote_plugin_id,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "checkout plugin share"))?;
        self.clear_plugin_related_caches();
        Ok(PluginShareCheckoutResponse {
            remote_plugin_id: result.remote_plugin_id,
            plugin_id: result.plugin_id,
            plugin_name: result.plugin_name,
            plugin_path: result.plugin_path,
            marketplace_name: result.marketplace_name,
            marketplace_path: result.marketplace_path,
            remote_version: result.remote_version,
        })
    }

    async fn plugin_share_delete_response(
        &self,
        params: PluginShareDeleteParams,
    ) -> Result<PluginShareDeleteResponse, JSONRPCErrorError> {
        let (config, auth) = self.load_plugin_share_config_and_auth().await?;
        let PluginShareDeleteParams { remote_plugin_id } = params;
        if remote_plugin_id.is_empty() || !is_valid_remote_plugin_id(&remote_plugin_id) {
            return Err(invalid_request("invalid remote plugin id"));
        }

        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        codex_core_plugins::remote::delete_remote_plugin_share(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.as_path(),
            &remote_plugin_id,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "delete remote plugin share"))?;
        self.clear_plugin_related_caches();
        Ok(PluginShareDeleteResponse {})
    }

    async fn load_plugin_share_config_and_auth(
        &self,
    ) -> Result<(Config, Option<CodexAuth>), JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins) {
            return Err(invalid_request("plugin sharing is not enabled"));
        }
        let auth = self.auth_manager.auth().await;
        Ok((config, auth))
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

        self.on_effective_plugins_changed();

        let plugin_mcp_servers = load_plugin_mcp_servers(result.installed_path.as_path()).await;
        if !plugin_mcp_servers.is_empty() {
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
        remote_plugin_id: String,
    ) -> Result<PluginInstallResponse, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins) {
            return Err(invalid_request(format!(
                "remote plugin install is not enabled for marketplace {remote_marketplace_name}"
            )));
        }
        validate_remote_plugin_id(&remote_plugin_id)?;

        let auth = self.auth_manager.auth().await;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let remote_detail =
            codex_core_plugins::remote::fetch_remote_plugin_detail_with_download_urls(
                &remote_plugin_service_config,
                auth.as_ref(),
                &remote_marketplace_name,
                &remote_plugin_id,
            )
            .await
            .map_err(|err| {
                remote_plugin_catalog_error_to_jsonrpc(
                    err,
                    "read remote plugin details before install",
                )
            })?;
        if remote_detail.summary.availability == PluginAvailability::DisabledByAdmin {
            return Err(invalid_request(format!(
                "remote plugin {remote_plugin_id} is disabled by admin"
            )));
        }
        if remote_detail.summary.install_policy == PluginInstallPolicy::NotAvailable {
            return Err(invalid_request(format!(
                "remote plugin {remote_plugin_id} is not available for install"
            )));
        }
        let actual_remote_marketplace_name = remote_detail.marketplace_name.clone();
        // Direct install writes the same cache tree that installed-plugin sync
        // prunes before the backend installed snapshot can include this plugin.
        let _remote_plugin_cache_mutation =
            codex_core_plugins::remote::mark_remote_plugin_cache_mutation_in_flight(
                config.codex_home.as_path(),
                &actual_remote_marketplace_name,
                &remote_detail.summary.name,
            );
        let validated_bundle = codex_core_plugins::remote_bundle::validate_remote_plugin_bundle(
            &remote_plugin_id,
            &actual_remote_marketplace_name,
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
            &actual_remote_marketplace_name,
            &remote_plugin_id,
        )
        .await
        .map_err(|err| remote_plugin_catalog_error_to_jsonrpc(err, "install remote plugin"))?;

        self.thread_manager
            .plugins_manager()
            .maybe_start_remote_installed_plugins_cache_refresh_after_mutation(
                &config.plugins_config_input(),
                auth.clone(),
                Some(self.effective_plugins_changed_callback()),
            );

        let mut plugin_metadata =
            plugin_telemetry_metadata_from_root(&result.plugin_id, &result.installed_path).await;
        plugin_metadata.remote_plugin_id = Some(remote_plugin_id);
        self.analytics_events_client
            .track_plugin_installed(plugin_metadata);

        let plugin_mcp_servers = load_plugin_mcp_servers(result.installed_path.as_path()).await;
        if !plugin_mcp_servers.is_empty() {
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
        plugin_apps: &[codex_plugin::AppConnectorId],
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

        plugin_apps_needing_auth(
            &all_connectors,
            &accessible_connectors,
            plugin_apps,
            codex_apps_ready,
        )
    }

    async fn start_plugin_mcp_oauth_logins(
        &self,
        config: &Config,
        plugin_mcp_servers: HashMap<String, McpServerConfig>,
    ) {
        for (name, server) in plugin_mcp_servers {
            let oauth_config = match oauth_login_support(&server.transport).await {
                McpOAuthLoginSupport::Supported(config) => config,
                McpOAuthLoginSupport::Unsupported => continue,
                McpOAuthLoginSupport::Unknown(err) => {
                    warn!(
                        "MCP server may or may not require login for plugin install {name}: {err}"
                    );
                    continue;
                }
            };

            let resolved_scopes = resolve_oauth_scopes(
                /*explicit_scopes*/ None,
                server.scopes.clone(),
                oauth_config.discovered_scopes.clone(),
            );

            let store_mode = config.mcp_oauth_credentials_store_mode;
            let callback_port = config.mcp_oauth_callback_port;
            let callback_url = config.mcp_oauth_callback_url.clone();
            let outgoing = Arc::clone(&self.outgoing);
            let notification_name = name.clone();

            tokio::spawn(async move {
                let first_attempt = perform_oauth_login_silent(
                    &name,
                    &oauth_config.url,
                    store_mode,
                    oauth_config.http_headers.clone(),
                    oauth_config.env_http_headers.clone(),
                    &resolved_scopes.scopes,
                    server.oauth_resource.as_deref(),
                    callback_port,
                    callback_url.as_deref(),
                )
                .await;

                let final_result = match first_attempt {
                    Err(err) if should_retry_without_scopes(&resolved_scopes, &err) => {
                        perform_oauth_login_silent(
                            &name,
                            &oauth_config.url,
                            store_mode,
                            oauth_config.http_headers,
                            oauth_config.env_http_headers,
                            &[],
                            server.oauth_resource.as_deref(),
                            callback_port,
                            callback_url.as_deref(),
                        )
                        .await
                    }
                    result => result,
                };

                let (success, error) = match final_result {
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
        }
    }

    async fn plugin_uninstall_response(
        &self,
        params: PluginUninstallParams,
    ) -> Result<PluginUninstallResponse, JSONRPCErrorError> {
        let PluginUninstallParams { plugin_id } = params;
        if codex_plugin::PluginId::parse(&plugin_id).is_err()
            && !is_valid_remote_plugin_id(&plugin_id)
        {
            return Err(invalid_request("invalid remote plugin id"));
        }
        if is_valid_remote_plugin_id(&plugin_id) {
            return self.remote_plugin_uninstall_response(plugin_id).await;
        }
        let plugins_manager = self.thread_manager.plugins_manager();

        plugins_manager
            .uninstall_plugin(plugin_id)
            .await
            .map_err(Self::plugin_uninstall_error)?;
        match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(_) => self.on_effective_plugins_changed(),
            Err(err) => {
                warn!(
                    "failed to reload config after plugin uninstall, clearing plugin-related caches only: {err:?}"
                );
                self.clear_plugin_related_caches();
            }
        }
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
        if !config.features.enabled(Feature::Plugins) {
            return Err(invalid_request("remote plugin uninstall is not enabled"));
        }
        validate_remote_plugin_id(&plugin_id)?;

        let auth = self.auth_manager.auth().await;
        let remote_plugin_service_config = RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        };
        let uninstall_result = codex_core_plugins::remote::uninstall_remote_plugin(
            &remote_plugin_service_config,
            auth.as_ref(),
            config.codex_home.to_path_buf(),
            &plugin_id,
        )
        .await;

        if matches!(
            &uninstall_result,
            Ok(()) | Err(RemotePluginCatalogError::CacheRemove(_))
        ) {
            let plugins_manager = self.thread_manager.plugins_manager();
            if plugins_manager.clear_remote_installed_plugins_cache() {
                self.on_effective_plugins_changed();
            }
            plugins_manager.maybe_start_remote_installed_plugins_cache_refresh_after_mutation(
                &config.plugins_config_input(),
                auth.clone(),
                Some(self.effective_plugins_changed_callback()),
            );
        }

        uninstall_result.map_err(|err| {
            remote_plugin_catalog_error_to_jsonrpc(err, "uninstall remote plugin")
        })?;
        Ok(PluginUninstallResponse {})
    }
}

async fn load_plugin_app_summaries(
    config: &Config,
    plugin_apps: &[codex_plugin::AppConnectorId],
    environment_manager: &EnvironmentManager,
) -> Vec<AppSummary> {
    if plugin_apps.is_empty() {
        return Vec::new();
    }

    let connectors =
        match connectors::list_all_connectors_with_options(config, /*force_refetch*/ false).await {
            Ok(connectors) => connectors,
            Err(err) => {
                warn!("failed to load app metadata for plugin/read: {err:#}");
                connectors::list_cached_all_connectors(config)
                    .await
                    .unwrap_or_default()
            }
        };

    let plugin_connectors = connectors::connectors_for_plugin_apps(connectors, plugin_apps);

    let accessible_connectors =
        match connectors::list_accessible_connectors_from_mcp_tools_with_environment_manager(
            config,
            /*force_refetch*/ false,
            environment_manager,
        )
        .await
        {
            Ok(status) if status.codex_apps_ready => status.connectors,
            Ok(_) => {
                return plugin_connectors
                    .into_iter()
                    .map(AppSummary::from)
                    .collect();
            }
            Err(err) => {
                warn!("failed to load app auth state for plugin/read: {err:#}");
                return plugin_connectors
                    .into_iter()
                    .map(AppSummary::from)
                    .collect();
            }
        };

    let accessible_ids = accessible_connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect::<HashSet<_>>();

    plugin_connectors
        .into_iter()
        .map(|connector| {
            let needs_auth = !accessible_ids.contains(connector.id.as_str());
            AppSummary {
                id: connector.id,
                name: connector.name,
                description: connector.description,
                install_url: connector.install_url,
                needs_auth,
            }
        })
        .collect()
}

fn plugin_apps_needing_auth(
    all_connectors: &[AppInfo],
    accessible_connectors: &[AppInfo],
    plugin_apps: &[codex_plugin::AppConnectorId],
    codex_apps_ready: bool,
) -> Vec<AppSummary> {
    if !codex_apps_ready {
        return Vec::new();
    }

    let accessible_ids = accessible_connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect::<HashSet<_>>();
    let plugin_app_ids = plugin_apps
        .iter()
        .map(|connector_id| connector_id.0.as_str())
        .collect::<HashSet<_>>();

    all_connectors
        .iter()
        .filter(|connector| {
            plugin_app_ids.contains(connector.id.as_str())
                && !accessible_ids.contains(connector.id.as_str())
        })
        .cloned()
        .map(|connector| AppSummary {
            id: connector.id,
            name: connector.name,
            description: connector.description,
            install_url: connector.install_url,
            needs_auth: true,
        })
        .collect()
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
        remote_plugin_id: Some(summary.remote_plugin_id),
        local_version: None,
        name: summary.name,
        share_context: summary
            .share_context
            .map(remote_plugin_share_context_to_info),
        source: PluginSource::Remote,
        installed: summary.installed,
        enabled: summary.enabled,
        install_policy: summary.install_policy,
        auth_policy: summary.auth_policy,
        availability: summary.availability,
        interface: summary.interface,
        keywords: summary.keywords,
    }
}

fn remote_plugin_share_context_to_info(
    context: RemoteCatalogPluginShareContext,
) -> PluginShareContext {
    PluginShareContext {
        remote_plugin_id: context.remote_plugin_id,
        remote_version: context.remote_version,
        discoverability: Some(remote_plugin_share_discoverability_to_info(
            context.discoverability,
        )),
        share_url: context.share_url,
        creator_account_user_id: context.creator_account_user_id,
        creator_name: context.creator_name,
        share_principals: context.share_principals.map(|principals| {
            principals
                .into_iter()
                .map(plugin_share_principal_from_remote)
                .collect()
        }),
    }
}

fn remote_plugin_share_discoverability_to_info(
    discoverability: codex_core_plugins::remote::RemotePluginShareDiscoverability,
) -> PluginShareDiscoverability {
    match discoverability {
        codex_core_plugins::remote::RemotePluginShareDiscoverability::Listed => {
            PluginShareDiscoverability::Listed
        }
        codex_core_plugins::remote::RemotePluginShareDiscoverability::Unlisted => {
            PluginShareDiscoverability::Unlisted
        }
        codex_core_plugins::remote::RemotePluginShareDiscoverability::Private => {
            PluginShareDiscoverability::Private
        }
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
        hooks: Vec::new(),
        apps,
        mcp_servers: Vec::new(),
    }
}

fn remote_plugin_catalog_error_to_jsonrpc(
    err: RemotePluginCatalogError,
    context: &str,
) -> JSONRPCErrorError {
    let message = format!("{context}: {err}");
    match &err {
        RemotePluginCatalogError::AuthRequired | RemotePluginCatalogError::UnsupportedAuthMode => {
            invalid_request(message)
        }
        RemotePluginCatalogError::UnexpectedStatus { status, .. } if status.as_u16() == 404 => {
            invalid_request(message)
        }
        RemotePluginCatalogError::InvalidPluginPath { .. }
        | RemotePluginCatalogError::PluginShareCheckoutNotAvailable { .. }
        | RemotePluginCatalogError::ArchiveTooLarge { .. }
        | RemotePluginCatalogError::UnknownMarketplace { .. } => invalid_request(message),
        RemotePluginCatalogError::AuthToken(_)
        | RemotePluginCatalogError::Request { .. }
        | RemotePluginCatalogError::UnexpectedStatus { .. }
        | RemotePluginCatalogError::Decode { .. }
        | RemotePluginCatalogError::InvalidBaseUrl(_)
        | RemotePluginCatalogError::InvalidBaseUrlPath
        | RemotePluginCatalogError::UnexpectedPluginId { .. }
        | RemotePluginCatalogError::UnexpectedSkillName { .. }
        | RemotePluginCatalogError::UnexpectedEnabledState { .. }
        | RemotePluginCatalogError::Archive { .. }
        | RemotePluginCatalogError::ArchiveJoin(_)
        | RemotePluginCatalogError::MissingUploadEtag
        | RemotePluginCatalogError::UnexpectedResponse(_)
        | RemotePluginCatalogError::CacheRemove(_) => internal_error(message),
    }
}

fn remote_plugin_bundle_install_error_to_jsonrpc(
    err: codex_core_plugins::remote_bundle::RemotePluginBundleInstallError,
) -> JSONRPCErrorError {
    internal_error(format!("install remote plugin bundle: {err}"))
}
