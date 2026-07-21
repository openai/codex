mod memory;
pub(crate) mod plugins;
pub(crate) mod sessions;

use crate::config_values::is_empty_toml_table;
use crate::config_values::merge_missing_mcp_servers;
use crate::config_values::merge_missing_toml_values;
use crate::config_values::migrated_mcp_server_names;
use crate::count_missing_subagents;
use crate::migration_source::InstructionSourceGroup;
use crate::migration_source::PluginDetectionContext;
use crate::missing_subagent_names;
use crate::model::ExternalAgentConfigDetectOptions;
use crate::model::ExternalAgentConfigMigrationItem;
use crate::model::ExternalAgentConfigMigrationItemType;
use crate::model::MigrationDetails;
use crate::reporting::emit_migration_metric;
use crate::scope::MigrationScope;
use crate::service::ExternalAgentConfigService;
use crate::service::configured_marketplace_plugins;
use crate::service::missing_subdirectory_names;
use crate::service::named_migrations;
use crate::utils::display_source_paths;
use crate::utils::invalid_data_error;
use crate::utils::is_missing_or_empty_text_file;
use codex_config::types::PluginConfig;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginsManager;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use toml::Value as TomlValue;

const EXTERNAL_AGENT_CONFIG_DETECT_METRIC: &str = "codex.external_agent_config.detect";

impl ExternalAgentConfigService {
    pub async fn detect(
        &self,
        params: ExternalAgentConfigDetectOptions,
    ) -> io::Result<Vec<ExternalAgentConfigMigrationItem>> {
        let mut items = Vec::new();
        if params.include_home {
            self.detect_migrations(&MigrationScope::home(), &mut items)
                .await?;
        }

        for cwd in params.cwds.as_deref().unwrap_or(&[]) {
            let Some(scope) = MigrationScope::from_cwd(Some(cwd))? else {
                continue;
            };
            if scope.is_home() {
                continue;
            }
            self.detect_migrations(&scope, &mut items).await?;
        }

        if params.include_home
            && params.include_memory
            && self.source.supports_memory()
            && let Some(item) = memory::detect(&self.codex_home, &self.external_agent_home)?
        {
            items.push(item);
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::Memory,
                /*skills_count*/ None,
            );
        }

        Ok(items)
    }

    async fn detect_migrations(
        &self,
        scope: &MigrationScope,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
        let repo_root = scope.repo_root();
        let cwd = scope.cwd();
        let source_settings = self.source_settings(scope);
        let settings = self.effective_source_settings(scope)?;
        let target_config = repo_root.map_or_else(
            || self.codex_home.join("config.toml"),
            |repo_root| repo_root.join(".codex").join("config.toml"),
        );
        if let Some(settings) = settings.as_ref() {
            let migrated = self.source.build_config(settings)?;
            if !is_empty_toml_table(&migrated) {
                let mut should_include = true;
                if target_config.exists() {
                    let existing_raw = fs::read_to_string(&target_config)?;
                    let mut existing = if existing_raw.trim().is_empty() {
                        TomlValue::Table(Default::default())
                    } else {
                        toml::from_str::<TomlValue>(&existing_raw).map_err(|err| {
                            invalid_data_error(format!("invalid existing config.toml: {err}"))
                        })?
                    };
                    should_include = merge_missing_toml_values(&mut existing, &migrated)?;
                }

                if should_include {
                    items.push(ExternalAgentConfigMigrationItem {
                        item_type: ExternalAgentConfigMigrationItemType::Config,
                        description: format!(
                            "Migrate {} into {}",
                            source_settings.display(),
                            target_config.display()
                        ),
                        cwd: cwd.clone(),
                        details: None,
                    });
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                        ExternalAgentConfigMigrationItemType::Config,
                        /*skills_count*/ None,
                    );
                }
            }
        }

        let mcp_source_path = self
            .source
            .mcp_source_path(self.source_root(scope), self.source_config_dir(scope));
        let migrated_mcp = self.build_mcp_config(scope, settings.clone())?;
        let mut mcp_server_names = migrated_mcp_server_names(&migrated_mcp);
        if !is_empty_toml_table(&migrated_mcp) {
            if target_config.exists() {
                let existing_raw = fs::read_to_string(&target_config)?;
                let mut existing = if existing_raw.trim().is_empty() {
                    TomlValue::Table(Default::default())
                } else {
                    toml::from_str::<TomlValue>(&existing_raw).map_err(|err| {
                        invalid_data_error(format!("invalid existing config.toml: {err}"))
                    })?
                };
                mcp_server_names = merge_missing_mcp_servers(&mut existing, &migrated_mcp)?;
            }

            if !mcp_server_names.is_empty() {
                items.push(ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
                    description: format!(
                        "Migrate MCP servers from {} into {}",
                        mcp_source_path.display(),
                        target_config.display()
                    ),
                    cwd: cwd.clone(),
                    details: Some(MigrationDetails {
                        mcp_servers: named_migrations(mcp_server_names),
                        ..Default::default()
                    }),
                });
                emit_migration_metric(
                    EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                    ExternalAgentConfigMigrationItemType::McpServerConfig,
                    /*skills_count*/ None,
                );
            }
        }

        let source_external_agent_dir = self.source_config_dir(scope);
        let target_hooks = repo_root.map_or_else(
            || self.codex_home.join("hooks.json"),
            |repo_root| repo_root.join(".codex").join("hooks.json"),
        );
        let hook_event_names = self
            .source
            .hook_event_names(source_external_agent_dir.as_path(), &target_hooks)?;
        if !hook_event_names.is_empty() && is_missing_or_empty_text_file(&target_hooks)? {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Hooks,
                description: format!(
                    "Migrate hooks from {} to {}",
                    source_external_agent_dir.display(),
                    target_hooks.display()
                ),
                cwd: cwd.clone(),
                details: Some(MigrationDetails {
                    hooks: named_migrations(hook_event_names),
                    ..Default::default()
                }),
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::Hooks,
                /*skills_count*/ None,
            );
        }

        let source_skills = repo_root.map_or_else(
            || self.external_agent_home.join("skills"),
            |repo_root| repo_root.join(self.source.config_dir()).join("skills"),
        );
        let target_skills = repo_root.map_or_else(
            || self.home_target_skills_dir(),
            |repo_root| repo_root.join(".agents").join("skills"),
        );
        let skill_names = missing_subdirectory_names(&source_skills, &target_skills)?;
        let skills_count = skill_names.len();
        if skills_count > 0 {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Migrate skills from {} to {}",
                    source_skills.display(),
                    target_skills.display()
                ),
                cwd: cwd.clone(),
                details: Some(MigrationDetails {
                    skills: named_migrations(skill_names),
                    ..Default::default()
                }),
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::Skills,
                Some(skills_count),
            );
        }

        let source_commands = source_external_agent_dir.join("commands");
        let target_command_skills = repo_root.map_or_else(
            || self.home_target_skills_dir(),
            |repo_root| repo_root.join(".agents").join("skills"),
        );
        let commands_count = self
            .source
            .count_missing_commands(&source_commands, &target_command_skills)?;
        if commands_count > 0 {
            let command_names = self
                .source
                .missing_command_names(&source_commands, &target_command_skills)?;
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Commands,
                description: format!(
                    "Migrate commands from {} to {}",
                    source_commands.display(),
                    target_command_skills.display()
                ),
                cwd: cwd.clone(),
                details: Some(MigrationDetails {
                    commands: named_migrations(command_names),
                    ..Default::default()
                }),
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::Commands,
                Some(commands_count),
            );
        }

        let source_subagents = source_external_agent_dir.join("agents");
        let target_subagents = repo_root.map_or_else(
            || self.codex_home.join("agents"),
            |repo_root| repo_root.join(".codex").join("agents"),
        );
        let subagents_count = count_missing_subagents(&source_subagents, &target_subagents)?;
        if subagents_count > 0 {
            let subagent_names = missing_subagent_names(&source_subagents, &target_subagents)?;
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Subagents,
                description: format!(
                    "Migrate subagents from {} to {}",
                    source_subagents.display(),
                    target_subagents.display()
                ),
                cwd: cwd.clone(),
                details: Some(MigrationDetails {
                    subagents: named_migrations(subagent_names),
                    ..Default::default()
                }),
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::Subagents,
                Some(subagents_count),
            );
        }

        let instruction_source_groups = if let Some(repo_root) = repo_root {
            self.repo_agents_md_source_groups(repo_root)?
        } else {
            let sources = self.home_agents_md_sources()?;
            (!sources.is_empty())
                .then(|| InstructionSourceGroup {
                    scope: self.codex_home.clone(),
                    sources,
                })
                .into_iter()
                .collect()
        };
        for group in instruction_source_groups {
            let target_agents_md = group.scope.join("AGENTS.md");
            if !is_missing_or_empty_text_file(&target_agents_md)? {
                continue;
            }
            let item_cwd = repo_root.is_some().then(|| group.scope.clone());
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Migrate {} to {}",
                    display_source_paths(&group.sources),
                    target_agents_md.display()
                ),
                cwd: item_cwd,
                details: None,
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::AgentsMd,
                /*skills_count*/ None,
            );
        }

        if self.source.supports_plugin_migration(settings.as_ref()) {
            match ConfigBuilder::default()
                .codex_home(self.codex_home.clone())
                .fallback_cwd(Some(self.codex_home.clone()))
                .build()
                .await
            {
                Ok(config) => {
                    let configured_plugin_ids = config
                        .config_layer_stack
                        .get_active_user_layer()
                        .and_then(|user_layer| user_layer.config.get("plugins"))
                        .and_then(|plugins| {
                            match plugins.clone().try_into::<HashMap<String, PluginConfig>>() {
                                Ok(plugins) => Some(plugins),
                                Err(err) => {
                                    tracing::warn!("invalid plugins config: {err}");
                                    None
                                }
                            }
                        })
                        .map(|plugins| plugins.into_keys().collect::<HashSet<_>>())
                        .unwrap_or_default();
                    let configured_marketplace_plugins = configured_marketplace_plugins(
                        &config,
                        &PluginsManager::new(self.codex_home.clone()),
                    )?;
                    let source_root = repo_root.unwrap_or(self.external_agent_home.as_path());
                    if let Some(detected) =
                        self.source.plugin_migration(PluginDetectionContext {
                            external_agent_home: self.external_agent_home.as_path(),
                            source_settings: source_settings.as_path(),
                            source_root,
                            repo_root,
                            settings: settings.as_ref(),
                            configured_plugin_ids: &configured_plugin_ids,
                            configured_marketplace_plugins: &configured_marketplace_plugins,
                        })?
                    {
                        emit_migration_metric(
                            EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                            ExternalAgentConfigMigrationItemType::Plugins,
                            /*skills_count*/ None,
                        );
                        items.push(ExternalAgentConfigMigrationItem {
                            item_type: ExternalAgentConfigMigrationItemType::Plugins,
                            description: detected.description,
                            cwd: cwd.clone(),
                            details: Some(detected.details),
                        });
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        settings_path = %source_settings.display(),
                        "skipping external agent plugin migration detection because config load failed"
                    );
                }
            }
        }

        if scope.is_home() {
            let sessions = self.source.recent_sessions(
                &self.external_agent_home,
                &self.codex_home,
                self.session_import_limits,
            )?;
            if !sessions.is_empty() {
                items.push(ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Sessions,
                    description: format!(
                        "Migrate recent sessions from {}",
                        self.external_agent_home.join("projects").display()
                    ),
                    cwd: None,
                    details: Some(MigrationDetails {
                        sessions,
                        ..Default::default()
                    }),
                });
                emit_migration_metric(
                    EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                    ExternalAgentConfigMigrationItemType::Sessions,
                    /*skills_count*/ None,
                );
            }
        }

        Ok(())
    }
}
