use super::*;
use codex_config::config_toml::ConfigToml;
use futures::StreamExt;

#[derive(Clone)]
pub(crate) struct CatalogRequestProcessor {
    pub(super) auth_manager: Arc<AuthManager>,
    pub(super) thread_manager: Arc<ThreadManager>,
    pub(super) config: Arc<Config>,
    pub(super) config_manager: ConfigManager,
    pub(super) workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
}

const SKILLS_LIST_CWD_CONCURRENCY: usize = 5;

struct SkillsListTarget {
    path_ref: codex_core::skills::EnvironmentPathRef,
}

struct ResolvedSkillsListTarget {
    primary_cwd: codex_core::skills::EnvironmentPathRef,
    config_layer_stack: ConfigLayerStack,
}

fn skills_to_info(
    skills: &[codex_core::skills::SkillMetadata],
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
            }
        })
        .collect()
}

fn hooks_to_info(hooks: &[codex_hooks::HookListEntry]) -> Vec<HookMetadata> {
    hooks
        .iter()
        .map(|hook| HookMetadata {
            key: hook.key.clone(),
            event_name: hook.event_name.into(),
            handler_type: hook.handler_type.into(),
            matcher: hook.matcher.clone(),
            command: hook.command.clone(),
            timeout_sec: hook.timeout_sec,
            status_message: hook.status_message.clone(),
            source_path: hook.source_path.clone(),
            source: hook.source.into(),
            plugin_id: hook.plugin_id.clone(),
            display_order: hook.display_order,
            enabled: hook.enabled,
            is_managed: hook.is_managed,
            current_hash: hook.current_hash.clone(),
            trust_status: hook.trust_status.into(),
        })
        .collect()
}

fn errors_to_info(
    errors: &[codex_core::skills::SkillError],
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
        auth_manager: Arc<AuthManager>,
        thread_manager: Arc<ThreadManager>,
        config: Arc<Config>,
        config_manager: ConfigManager,
        workspace_settings_cache: Arc<workspace_settings::WorkspaceSettingsCache>,
    ) -> Self {
        Self {
            auth_manager,
            thread_manager,
            config,
            config_manager,
            workspace_settings_cache,
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

    pub(crate) async fn model_list(
        &self,
        params: ModelListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        Self::list_models(self.thread_manager.clone(), params)
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
        let config_layer_stack = self.load_config_layers_for_cwd(cwd_abs.clone()).await?;

        Ok((cwd_abs, config_layer_stack))
    }

    async fn load_config_layers_for_cwd(
        &self,
        cwd: AbsolutePathBuf,
    ) -> Result<ConfigLayerStack, String> {
        let config_layer_stack = self
            .config_manager
            .load_config_layers_for_cwd(cwd)
            .await
            .map_err(|err| err.to_string())?;

        Ok(config_layer_stack)
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

    fn skills_list_targets(
        &self,
        cwds: Vec<PathBuf>,
        environments: Option<Vec<codex_app_server_protocol::SkillsListEnvironment>>,
    ) -> Result<
        Vec<Result<SkillsListTarget, codex_app_server_protocol::SkillsListEntry>>,
        JSONRPCErrorError,
    > {
        if environments.is_some() && !cwds.is_empty() {
            return Err(invalid_request(
                "skills/list cannot set both `cwds` and `environments`",
            ));
        }

        let environment_manager = self.thread_manager.environment_manager();
        match environments {
            Some(environments) => environments
                .into_iter()
                .map(|environment| {
                    let selected_environment = environment_manager
                        .get_environment(&environment.environment_id)
                        .ok_or_else(|| {
                            invalid_request(format!(
                                "unknown skills/list environment id `{}`",
                                environment.environment_id
                            ))
                        })?;
                    Ok(Ok(SkillsListTarget {
                        path_ref: codex_core::skills::EnvironmentPathRef::new(
                            Some(environment.environment_id),
                            selected_environment.get_filesystem(),
                            environment.cwd,
                        ),
                    }))
                })
                .collect(),
            None => {
                let cwds = if cwds.is_empty() {
                    vec![self.config.cwd.to_path_buf()]
                } else {
                    cwds
                };
                let environment = environment_manager
                    .default_environment()
                    .map(|environment| {
                        (
                            environment_manager
                                .default_environment_id()
                                .map(str::to_string),
                            environment,
                        )
                    })
                    .or_else(|| {
                        environment_manager
                            .try_local_environment()
                            .map(|environment| {
                                (
                                    Some(codex_exec_server::LOCAL_ENVIRONMENT_ID.to_string()),
                                    environment,
                                )
                            })
                    });
                Ok(cwds
                    .into_iter()
                    .map(|cwd| {
                        let cwd = match AbsolutePathBuf::relative_to_current_dir(&cwd) {
                            Ok(cwd) => cwd,
                            Err(err) => {
                                return Err(Self::skills_list_error_entry(cwd, err.to_string()));
                            }
                        };
                        let Some((environment_id, environment)) = environment.as_ref() else {
                            return Err(Self::skills_list_empty_entry(cwd.to_path_buf()));
                        };
                        Ok(SkillsListTarget {
                            path_ref: codex_core::skills::EnvironmentPathRef::new(
                                environment_id.clone(),
                                environment.get_filesystem(),
                                cwd,
                            ),
                        })
                    })
                    .collect())
            }
        }
    }

    async fn resolve_skills_list_target(
        &self,
        target: SkillsListTarget,
    ) -> Result<ResolvedSkillsListTarget, codex_app_server_protocol::SkillsListEntry> {
        let SkillsListTarget { path_ref } = target;
        match self
            .load_config_layers_for_cwd(path_ref.path().clone())
            .await
        {
            Ok(config_layer_stack) => Ok(ResolvedSkillsListTarget {
                primary_cwd: path_ref,
                config_layer_stack,
            }),
            Err(message) => Err(Self::skills_list_error_entry(
                path_ref.path().to_path_buf(),
                message,
            )),
        }
    }

    async fn load_skills_list_entry(
        &self,
        target: ResolvedSkillsListTarget,
        config: &Config,
        workspace_codex_plugins_enabled: bool,
        force_reload: bool,
    ) -> codex_app_server_protocol::SkillsListEntry {
        let ResolvedSkillsListTarget {
            primary_cwd,
            config_layer_stack,
        } = target;
        let cwd = primary_cwd.path().to_path_buf();
        let skill_root_path_ref = Some(primary_cwd.clone());
        let effective_skill_roots = if workspace_codex_plugins_enabled {
            let plugins_input = config
                .plugins_config_input()
                .with_skill_path_ref(skill_root_path_ref.clone());
            self.thread_manager
                .plugins_manager()
                .effective_skill_roots_for_layer_stack(&config_layer_stack, &plugins_input)
                .await
        } else {
            Vec::new()
        };
        let skills_input = codex_core::skills::SkillsLoadInput::new(
            Some(primary_cwd),
            skill_root_path_ref,
            effective_skill_roots,
            config_layer_stack,
            config.bundled_skills_enabled(),
        );
        let outcome = self
            .thread_manager
            .skills_manager()
            .skills_for_cwd(&skills_input, force_reload)
            .await;
        codex_app_server_protocol::SkillsListEntry {
            cwd,
            skills: skills_to_info(&outcome.skills, &outcome.disabled_paths),
            errors: errors_to_info(&outcome.errors),
        }
    }

    fn skills_list_empty_entry(cwd: PathBuf) -> codex_app_server_protocol::SkillsListEntry {
        codex_app_server_protocol::SkillsListEntry {
            cwd,
            skills: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn skills_list_error_entry(
        cwd: PathBuf,
        message: String,
    ) -> codex_app_server_protocol::SkillsListEntry {
        codex_app_server_protocol::SkillsListEntry {
            cwd: cwd.clone(),
            skills: Vec::new(),
            errors: vec![codex_app_server_protocol::SkillErrorInfo { path: cwd, message }],
        }
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

    async fn list_models(
        thread_manager: Arc<ThreadManager>,
        params: ModelListParams,
    ) -> Result<ModelListResponse, JSONRPCErrorError> {
        let ModelListParams {
            limit,
            cursor,
            include_hidden,
        } = params;
        let models = supported_models(thread_manager, include_hidden.unwrap_or(false)).await;
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
                    .load_latest_config_for_thread(thread_config.as_ref())
                    .await
                    .map_err(|err| internal_error(format!("failed to reload config: {err}")))?
            }
            None => self.load_latest_config(/*fallback_cwd*/ None).await?,
        };
        let auth = self.auth_manager.auth().await;
        let workspace_codex_plugins_enabled = self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await;

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
                    enabled: config.features.enabled(spec.id)
                        && (workspace_codex_plugins_enabled
                            || !matches!(spec.id, Feature::Apps | Feature::Plugins)),
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
        let config_layer_stack = match cwd {
            Some(cwd) => {
                let cwd = PathBuf::from(cwd);
                let (_, config_layer_stack) = self
                    .resolve_cwd_config(&cwd)
                    .await
                    .map_err(|err| internal_error(format!("failed to reload config: {err}")))?;
                config_layer_stack
            }
            None => self
                .config_manager
                .load_config_layers(/*cwd*/ None)
                .await
                .map_err(|err| internal_error(format!("failed to reload config: {err}")))?,
        };
        let effective_config: ConfigToml = config_layer_stack
            .effective_config()
            .try_into()
            .map_err(|err| internal_error(format!("failed to read effective config: {err}")))?;
        let mut profiles = vec![
            PermissionProfileSummary {
                id: BUILT_IN_PERMISSION_PROFILE_READ_ONLY.to_string(),
                description: None,
            },
            PermissionProfileSummary {
                id: BUILT_IN_PERMISSION_PROFILE_WORKSPACE.to_string(),
                description: None,
            },
            PermissionProfileSummary {
                id: BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS.to_string(),
                description: None,
            },
        ];
        let mut configured_profiles = effective_config
            .permissions
            .into_iter()
            .flat_map(|permissions| permissions.entries)
            .map(|(id, profile)| PermissionProfileSummary {
                id,
                description: profile.description,
            })
            .collect::<Vec<_>>();
        configured_profiles.sort_by(|left, right| left.id.cmp(&right.id));
        profiles.extend(configured_profiles);
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
        let SkillsListParams {
            cwds,
            environments,
            force_reload,
        } = params;
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let targets = self.skills_list_targets(cwds, environments)?;
        let auth = self.auth_manager.auth().await;
        let workspace_codex_plugins_enabled = self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await;
        let mut data = futures::stream::iter(targets.into_iter().enumerate())
            .map(|(index, target)| {
                let config = &config;
                async move {
                    let entry = match target {
                        Ok(target) => match self.resolve_skills_list_target(target).await {
                            Ok(target) => {
                                self.load_skills_list_entry(
                                    target,
                                    config,
                                    workspace_codex_plugins_enabled,
                                    force_reload,
                                )
                                .await
                            }
                            Err(entry) => entry,
                        },
                        Err(entry) => entry,
                    };
                    (index, entry)
                }
            })
            .buffer_unordered(SKILLS_LIST_CWD_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        data.sort_unstable_by_key(|(index, _)| *index);
        let data = data.into_iter().map(|(_, entry)| entry).collect();
        Ok(SkillsListResponse { data })
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

        let auth = self.auth_manager.auth().await;
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
            let workspace_codex_plugins_enabled = self
                .workspace_codex_plugins_enabled(&config, auth.as_ref())
                .await;
            let plugins_enabled =
                config.features.enabled(Feature::Plugins) && workspace_codex_plugins_enabled;
            let plugin_outcome = if plugins_enabled {
                let plugins_input = config.plugins_config_input();
                plugins_manager
                    .plugins_for_layer_stack(&config.config_layer_stack, &plugins_input)
                    .await
            } else {
                PluginLoadOutcome::default()
            };
            let hooks = codex_hooks::list_hooks(codex_hooks::HooksConfig {
                feature_enabled: config.features.enabled(Feature::CodexHooks),
                bypass_hook_trust: config.bypass_hook_trust,
                config_layer_stack: Some(config.config_layer_stack),
                plugin_hook_sources: plugin_outcome.effective_plugin_hook_sources(),
                plugin_hook_load_warnings: plugin_outcome.effective_plugin_hook_warnings(),
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
                self.thread_manager.skills_manager().clear_cache();
                SkillsConfigWriteResponse {
                    effective_enabled: enabled,
                }
            })
            .map_err(|err| internal_error(format!("failed to update skill settings: {err}")))
    }
}
