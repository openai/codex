use super::*;
use crate::model_catalog::ModelCatalog;
use codex_config::ConfigPathContext;
use codex_core::config::permission_profile_catalog;
use codex_hooks::HookListEntryHandler;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

#[derive(Clone)]
pub(crate) struct CatalogRequestProcessor {
    pub(super) outgoing: Arc<OutgoingMessageSender>,
    pub(super) skills_watcher: Arc<SkillsWatcher>,
    pub(super) thread_manager: Arc<ThreadManager>,
    pub(super) config: Arc<Config>,
    pub(super) config_manager: ConfigManager,
    model_catalog: Arc<ModelCatalog>,
}

const SKILLS_LIST_CWD_CONCURRENCY: usize = 5;

fn skills_to_info(
    skills: &[codex_skills::SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
) -> Vec<codex_app_server_protocol::SkillMetadata> {
    skills
        .iter()
        .map(|skill| {
            let enabled = !disabled_paths.contains(&skill.path_to_skills_md);
            codex_app_server_protocol::SkillMetadata {
                name: skill.name.clone(),
                description: skill.description.clone(),
                short_description: skill.short_description.clone(),
                interface: skill.interface.clone().map(|interface| {
                    codex_app_server_protocol::SkillInterface {
                        display_name: interface.display_name,
                        short_description: interface.short_description,
                        icon_small: interface.icon_small,
                        icon_large: interface.icon_large,
                        icon_small_url: None,
                        icon_large_url: None,
                        brand_color: interface.brand_color,
                        default_prompt: interface.default_prompt,
                    }
                }),
                dependencies: skill.dependencies.clone().map(|dependencies| {
                    codex_app_server_protocol::SkillDependencies {
                        tools: dependencies
                            .tools
                            .into_iter()
                            .map(|tool| codex_app_server_protocol::SkillToolDependency {
                                r#type: tool.r#type,
                                value: tool.value,
                                description: tool.description,
                                transport: tool.transport,
                                command: tool.command,
                                url: tool.url,
                            })
                            .collect(),
                    }
                }),
                path: skill.path_to_skills_md.clone(),
                scope: skill.scope.into(),
                enabled,
                plugin_id: skill.plugin_id.clone(),
            }
        })
        .collect()
}

fn hooks_to_info(hooks: &[codex_hooks::HookListEntry]) -> Vec<HookMetadata> {
    hooks
        .iter()
        .filter(|hook| !hook.builtin)
        .map(|hook| {
            let handler = match &hook.handler {
                HookListEntryHandler::Command { command, r#async } => {
                    HookHandlerMetadata::Command {
                        command: command.clone(),
                        r#async: *r#async,
                    }
                }
                HookListEntryHandler::McpTool { server, tool } => HookHandlerMetadata::McpTool {
                    server: server.clone(),
                    tool: tool.clone(),
                },
            };
            HookMetadata {
                key: hook.key.clone(),
                event_name: hook.event_name.into(),
                handler,
                matcher: hook.matcher.clone(),
                timeout_sec: hook.timeout_sec,
                status_message: hook.status_message.clone(),
                additional_context_limit: hook.additional_context_limit,
                source_path: hook.source_path.clone(),
                source: hook.source.into(),
                plugin_id: hook.plugin_id.clone(),
                display_order: hook.display_order,
                enabled: hook.enabled,
                is_managed: hook.is_managed,
                current_hash: hook.current_hash.clone(),
                trust_status: hook.trust_status.into(),
            }
        })
        .collect()
}

fn errors_to_info(
    errors: &[codex_skills::SkillError],
) -> Vec<codex_app_server_protocol::SkillErrorInfo> {
    errors
        .iter()
        .map(|err| codex_app_server_protocol::SkillErrorInfo {
            path: err.path.to_path_buf(),
            message: err.message.clone(),
        })
        .collect()
}

impl CatalogRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        skills_watcher: Arc<SkillsWatcher>,
        thread_manager: Arc<ThreadManager>,
        config: Arc<Config>,
        config_manager: ConfigManager,
        model_catalog: Arc<ModelCatalog>,
    ) -> Self {
        Self {
            outgoing,
            skills_watcher,
            thread_manager,
            config,
            config_manager,
            model_catalog,
        }
    }

    pub(crate) async fn skills_list(
        &self,
        params: SkillsListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.skills_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn hooks_list(
        &self,
        params: HooksListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.hooks_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn skills_config_write(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.skills_config_write_response_inner(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn skills_extra_roots_set(
        &self,
        params: SkillsExtraRootsSetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.skills_extra_roots_set_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn model_list(
        &self,
        params: ModelListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        // Gate the same provider used by the catalog, including when its model cache is warm.
        // Resolving credentials may refresh them, but explicit host policy prevents browser login.
        if let Some(gateway) = codex_model_provider::create_model_provider(
            self.config.model_provider.clone(),
            Some(self.thread_manager.auth_manager()),
        )
        .gateway_auth_manager()
        .map_err(|error| internal_error(error.to_string()))?
        {
            // Refreshing credentials can contact the gateway before the catalog's own check.
            self.config_manager
                .check_thread_model_provider(&self.config)
                .await
                .map_err(|err| config_load_error(&err))?;
            if gateway.resolve_access_token().await.is_err() {
                let current = self
                    .config_manager
                    .load_latest_config(/*fallback_cwd*/ None)
                    .await
                    .map_err(|err| config_load_error(&err))?;
                // Login RPCs use current config, while the catalog retains its startup provider.
                if current.model_provider_id != self.config.model_provider_id
                    || current.model_provider != self.config.model_provider
                {
                    return Err(invalid_request(
                        "Model provider settings changed. Restart Codex to apply them, then retry fetching the model list",
                    ));
                }
                return Err(invalid_request(
                    "Gateway sign-in required or unavailable. Complete gateway sign-in and retry fetching the model list",
                ));
            }
        }
        self.list_models(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn experimental_feature_list(
        &self,
        params: ExperimentalFeatureListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.experimental_feature_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn permission_profile_list(
        &self,
        params: PermissionProfileListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.permission_profile_list_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn collaboration_mode_list(
        &self,
        params: CollaborationModeListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        Self::list_collaboration_modes(self.thread_manager.clone(), params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn mock_experimental_method(
        &self,
        params: MockExperimentalMethodParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.mock_experimental_method_inner(params)
            .await
            .map(|response| Some(response.into()))
    }

    async fn resolve_cwd_config(
        &self,
        cwd: &Path,
    ) -> Result<(AbsolutePathBuf, ConfigLayerStack), String> {
        let cwd_abs =
            AbsolutePathBuf::relative_to_current_dir(cwd).map_err(|err| err.to_string())?;
        let config_layer_stack = self
            .config_manager
            .load_config_layers_for_cwd(cwd_abs.clone())
            .await
            .map_err(|err| err.to_string())?;

        Ok((cwd_abs, config_layer_stack))
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

    async fn list_models(
        &self,
        params: ModelListParams,
    ) -> Result<ModelListResponse, JSONRPCErrorError> {
        let ModelListParams {
            limit,
            cursor,
            include_hidden,
        } = params;
        let presets = self
            .model_catalog
            .list_models(codex_models_manager::manager::RefreshStrategy::OnlineIfUncached)
            .await
            .map_err(|err| config_load_error(&err))?;
        let models = supported_models(presets, include_hidden.unwrap_or(false));
        let total = models.len();

        if total == 0 {
            return Ok(ModelListResponse {
                data: Vec::new(),
                next_cursor: None,
            });
        }

        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = effective_limit.min(total);
        let start = match cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?,
            None => 0,
        };

        if start > total {
            return Err(invalid_request(format!(
                "cursor {start} exceeds total models {total}"
            )));
        }

        let end = start.saturating_add(effective_limit).min(total);
        let items = models[start..end].to_vec();
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };
        Ok(ModelListResponse {
            data: items,
            next_cursor,
        })
    }

    async fn list_collaboration_modes(
        thread_manager: Arc<ThreadManager>,
        params: CollaborationModeListParams,
    ) -> Result<CollaborationModeListResponse, JSONRPCErrorError> {
        let CollaborationModeListParams {} = params;
        let items = thread_manager
            .list_collaboration_modes()
            .into_iter()
            .map(Into::into)
            .collect();
        let response = CollaborationModeListResponse { data: items };
        Ok(response)
    }

    async fn experimental_feature_list_response(
        &self,
        params: ExperimentalFeatureListParams,
    ) -> Result<ExperimentalFeatureListResponse, JSONRPCErrorError> {
        let ExperimentalFeatureListParams {
            cursor,
            limit,
            thread_id,
        } = params;
        let config = match thread_id.as_deref() {
            Some(thread_id) => {
                let thread_id = ThreadId::from_string(thread_id)
                    .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;
                let thread = self
                    .thread_manager
                    .get_thread(thread_id)
                    .await
                    .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;
                let thread_config = thread.config().await;
                self.config_manager
                    .load_latest_config_with_session_layers(
                        &thread_config.config_layer_stack,
                        &thread_config.cwd,
                    )
                    .await
                    .map_err(|err| internal_error(format!("failed to reload config: {err}")))?
            }
            None => self.load_latest_config(/*fallback_cwd*/ None).await?,
        };
        let data = FEATURES
            .iter()
            .map(|spec| {
                let (stage, display_name, description, announcement) = match spec.stage {
                    Stage::Experimental {
                        name,
                        menu_description,
                        announcement,
                    } => (
                        ApiExperimentalFeatureStage::Beta,
                        Some(name.to_string()),
                        Some(menu_description.to_string()),
                        Some(announcement.to_string()),
                    ),
                    Stage::UnderDevelopment => (
                        ApiExperimentalFeatureStage::UnderDevelopment,
                        None,
                        None,
                        None,
                    ),
                    Stage::Stable => (ApiExperimentalFeatureStage::Stable, None, None, None),
                    Stage::Deprecated => {
                        (ApiExperimentalFeatureStage::Deprecated, None, None, None)
                    }
                    Stage::Removed => (ApiExperimentalFeatureStage::Removed, None, None, None),
                };

                ApiExperimentalFeature {
                    name: spec.key.to_string(),
                    stage,
                    display_name,
                    description,
                    announcement,
                    enabled: config.features.enabled(spec.id),
                    default_enabled: spec.default_enabled,
                }
            })
            .collect::<Vec<_>>();

        let total = data.len();
        if total == 0 {
            return Ok(ExperimentalFeatureListResponse {
                data: Vec::new(),
                next_cursor: None,
            });
        }

        // Clamp to 1 so limit=0 cannot return a non-advancing page.
        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = effective_limit.min(total);
        let start = match cursor {
            Some(cursor) => match cursor.parse::<usize>() {
                Ok(idx) => idx,
                Err(_) => return Err(invalid_request(format!("invalid cursor: {cursor}"))),
            },
            None => 0,
        };

        if start > total {
            return Err(invalid_request(format!(
                "cursor {start} exceeds total feature flags {total}"
            )));
        }

        let end = start.saturating_add(effective_limit).min(total);
        let data = data[start..end].to_vec();
        let next_cursor = if end < total {
            Some(end.to_string())
        } else {
            None
        };

        Ok(ExperimentalFeatureListResponse { data, next_cursor })
    }

    async fn permission_profile_list_response(
        &self,
        params: PermissionProfileListParams,
    ) -> Result<PermissionProfileListResponse, JSONRPCErrorError> {
        let PermissionProfileListParams { cursor, limit, cwd } = params;
        let (cwd, config_layer_stack) = match cwd {
            Some(cwd) => self
                .resolve_cwd_config(&PathBuf::from(cwd))
                .await
                .map_err(|err| internal_error(format!("failed to reload config: {err}")))?,
            None => (
                self.config.cwd.clone(),
                self.config_manager
                    .load_config_layers(/*cwd*/ None)
                    .await
                    .map_err(|err| internal_error(format!("failed to reload config: {err}")))?,
            ),
        };
        let context = ConfigPathContext::new(
            PathConvention::native(),
            Some(PathUri::from_abs_path(&cwd)),
            AbsolutePathBufGuard::home_directory()
                .and_then(|home| PathUri::from_host_native_path(home).ok()),
        );
        let profiles = permission_profile_catalog(&config_layer_stack, &context)
            .map_err(|err| internal_error(format!("failed to resolve permission profiles: {err}")))?
            .into_iter()
            .map(|profile| PermissionProfileSummary {
                id: profile.id,
                description: profile.description,
                allowed: profile.allowed,
            })
            .collect::<Vec<_>>();
        let total = profiles.len();
        let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
        let effective_limit = effective_limit.min(total);
        let start = match cursor {
            Some(cursor) => cursor
                .parse::<usize>()
                .map_err(|_| invalid_request(format!("invalid cursor: {cursor}")))?,
            None => 0,
        };

        if start > total {
            return Err(invalid_request(format!(
                "cursor {start} exceeds total permission profiles {total}"
            )));
        }

        let end = start.saturating_add(effective_limit).min(total);
        let data = profiles[start..end].to_vec();
        let next_cursor = (end < total).then_some(end.to_string());

        Ok(PermissionProfileListResponse { data, next_cursor })
    }

    async fn mock_experimental_method_inner(
        &self,
        params: MockExperimentalMethodParams,
    ) -> Result<MockExperimentalMethodResponse, JSONRPCErrorError> {
        let MockExperimentalMethodParams { value } = params;
        let response = MockExperimentalMethodResponse { echoed: value };
        Ok(response)
    }

    async fn skills_list_response(
        &self,
        params: SkillsListParams,
    ) -> Result<SkillsListResponse, JSONRPCErrorError> {
        let SkillsListParams { cwds, force_reload } = params;
        let cwds = if cwds.is_empty() {
            vec![self.config.cwd.to_path_buf()]
        } else {
            cwds
        };

        let skills_service = self.thread_manager.skills_service();
        let plugins_manager = self.thread_manager.plugins_manager();
        if force_reload {
            plugins_manager.clear_cache();
            skills_service.clear_cache();
        }
        let fs = self
            .thread_manager
            .environment_manager()
            .default_environment()
            .map(|environment| environment.get_filesystem());
        let skills_request = skills_service.for_request();
        let mut data = futures::stream::iter(cwds.into_iter().enumerate())
            .map(|(index, cwd)| {
                let fs = fs.clone();
                let skills_request = &skills_request;
                let plugins_manager = Arc::clone(&plugins_manager);
                async move {
                    let config = match self.load_latest_config(Some(cwd.clone())).await {
                        Ok(resolved) => resolved,
                        Err(error) => {
                            let error_path = cwd.clone();
                            return (
                                index,
                                codex_app_server_protocol::SkillsListEntry {
                                    cwd,
                                    skills: Vec::new(),
                                    errors: vec![codex_app_server_protocol::SkillErrorInfo {
                                        path: error_path,
                                        message: error.message,
                                    }],
                                },
                            );
                        }
                    };
                    let plugins_input = config.plugins_config_input();
                    let plugins = plugins_manager.plugins_for_config(&plugins_input).await;
                    let plugin_skill_snapshots =
                        plugins_manager.plugin_skill_snapshots_for_config(&plugins_input);
                    let skills_input = codex_skills_extension::HostSkillsLoadInput::new(
                        config.cwd.clone(),
                        plugins.effective_plugin_skill_roots(),
                        config.config_layer_stack,
                    )
                    .with_plugin_skill_snapshots(plugin_skill_snapshots);
                    let snapshot = skills_request
                        .snapshot_for_cwd(&skills_input, force_reload, fs)
                        .await;
                    let outcome = snapshot.outcome();
                    let errors = errors_to_info(&outcome.errors);
                    let skills = skills_to_info(&outcome.skills, &outcome.disabled_paths);
                    (
                        index,
                        codex_app_server_protocol::SkillsListEntry {
                            cwd,
                            skills,
                            errors,
                        },
                    )
                }
            })
            .buffer_unordered(SKILLS_LIST_CWD_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        data.sort_unstable_by_key(|(index, _)| *index);
        let data = data.into_iter().map(|(_, entry)| entry).collect();
        Ok(SkillsListResponse { data })
    }

    async fn skills_extra_roots_set_response(
        &self,
        params: SkillsExtraRootsSetParams,
    ) -> Result<SkillsExtraRootsSetResponse, JSONRPCErrorError> {
        let SkillsExtraRootsSetParams { extra_roots } = params;
        self.skills_watcher
            .register_runtime_extra_roots(&extra_roots);
        self.thread_manager
            .skills_service()
            .set_extra_roots(extra_roots);
        self.outgoing
            .send_server_notification(ServerNotification::SkillsChanged(
                codex_app_server_protocol::SkillsChangedNotification {},
            ))
            .await;
        Ok(SkillsExtraRootsSetResponse {})
    }

    /// Handle `hooks/list` by resolving hooks for each requested cwd.
    async fn hooks_list_response(
        &self,
        params: HooksListParams,
    ) -> Result<HooksListResponse, JSONRPCErrorError> {
        let HooksListParams { cwds } = params;
        let cwds = if cwds.is_empty() {
            vec![self.config.cwd.to_path_buf()]
        } else {
            cwds
        };

        let plugins_manager = self.thread_manager.plugins_manager();
        let mut data = Vec::new();
        for cwd in cwds {
            let config = match self
                .config_manager
                .load_for_cwd(
                    /*request_overrides*/ None,
                    ConfigOverrides::default(),
                    Some(cwd.clone()),
                )
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    let error_path = cwd.clone();
                    data.push(codex_app_server_protocol::HooksListEntry {
                        cwd,
                        hooks: Vec::new(),
                        warnings: Vec::new(),
                        errors: vec![codex_app_server_protocol::HookErrorInfo {
                            path: error_path,
                            message: err.to_string(),
                        }],
                    });
                    continue;
                }
            };
            let hooks_enabled = config.features.enabled(Feature::CodexHooks);
            let plugin_hooks = if hooks_enabled && config.features.enabled(Feature::Plugins) {
                let plugins_input = config.plugins_config_input();
                let plugin_outcome = plugins_manager.plugins_for_config(&plugins_input).await;
                codex_core_plugins::PluginHookLoadOutcome {
                    hook_sources: plugin_outcome.effective_plugin_hook_sources(),
                    hook_load_warnings: plugin_outcome.effective_plugin_hook_warnings(),
                }
            } else {
                codex_core_plugins::PluginHookLoadOutcome::default()
            };
            let hooks = codex_hooks::list_hooks(codex_hooks::HooksConfig {
                feature_enabled: hooks_enabled,
                bypass_hook_trust: config.bypass_hook_trust,
                config_layer_stack: Some(config.config_layer_stack),
                plugin_hook_sources: plugin_hooks.hook_sources,
                plugin_hook_load_warnings: plugin_hooks.hook_load_warnings,
                ..Default::default()
            });
            data.push(codex_app_server_protocol::HooksListEntry {
                cwd,
                hooks: hooks_to_info(&hooks.hooks),
                warnings: hooks.warnings,
                errors: Vec::new(),
            });
        }
        Ok(HooksListResponse { data })
    }

    async fn skills_config_write_response_inner(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, JSONRPCErrorError> {
        let SkillsConfigWriteParams {
            path,
            name,
            enabled,
        } = params;
        let edit = match (path, name) {
            (Some(path), None) => ConfigEdit::SetSkillConfig {
                path: path.into_path_buf(),
                enabled,
            },
            (None, Some(name)) if !name.trim().is_empty() => {
                ConfigEdit::SetSkillConfigByName { name, enabled }
            }
            _ => {
                return Err(invalid_params(
                    "skills/config/write requires exactly one of path or name",
                ));
            }
        };
        let edits = vec![edit];
        ConfigEditsBuilder::new(&self.config.codex_home)
            .with_edits(edits)
            .apply()
            .await
            .map(|()| {
                self.thread_manager.plugins_manager().clear_cache();
                self.thread_manager.skills_service().clear_cache();
                SkillsConfigWriteResponse {
                    effective_enabled: enabled,
                }
            })
            .map_err(|err| internal_error(format!("failed to update skill settings: {err}")))
    }
}
