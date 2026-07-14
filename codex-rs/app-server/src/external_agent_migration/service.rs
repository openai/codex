mod source;
mod source_cla;
mod utils;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::PluginInstallSource;
use codex_config::types::PluginConfig;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginInstallError;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::marketplace::MarketplaceError;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_add::is_local_marketplace_source;
use codex_external_agent_migration::count_missing_subagents;
use codex_external_agent_migration::missing_subagent_names;
use codex_external_agent_migration::sessions::ExternalAgentSessionMigration;
use codex_protocol::protocol::Product;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use self::source::ExternalAgentSource;
use self::source::InstructionSourceGroup;
use self::source::MarketplaceImportSource;
use self::source::PluginDetectionContext;
use self::source::SourceFeature;
#[cfg(test)]
use self::source_cla::CONFIG_DIR as EXTERNAL_AGENT_DIR;
#[cfg(test)]
use self::source_cla::CONFIG_MD as EXTERNAL_AGENT_CONFIG_MD;
#[cfg(test)]
use self::source_cla::KNOWN_MARKETPLACES_PATH as EXTERNAL_AGENT_KNOWN_MARKETPLACES_PATH;
#[cfg(test)]
use self::source_cla::OFFICIAL_MARKETPLACE_NAME as EXTERNAL_OFFICIAL_MARKETPLACE_NAME;
use self::utils::copy_dir_recursive;
use self::utils::display_source_paths;
use self::utils::rewrite_external_agent_terms;

const EXTERNAL_AGENT_CONFIG_DETECT_METRIC: &str = "codex.external_agent_config.detect";
const EXTERNAL_AGENT_CONFIG_IMPORT_METRIC: &str = "codex.external_agent_config.import";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigDetectOptions {
    pub include_home: bool,
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExternalAgentConfigMigrationItemType {
    Config,
    Skills,
    AgentsMd,
    Plugins,
    McpServerConfig,
    Subagents,
    Hooks,
    Commands,
    Sessions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PluginsMigration {
    pub marketplace_name: String,
    pub plugin_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedMigration {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MigrationDetails {
    pub plugins: Vec<PluginsMigration>,
    pub skills: Vec<NamedMigration>,
    pub sessions: Vec<ExternalAgentSessionMigration>,
    pub mcp_servers: Vec<NamedMigration>,
    pub hooks: Vec<NamedMigration>,
    pub subagents: Vec<NamedMigration>,
    pub commands: Vec<NamedMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingPluginImport {
    pub cwd: Option<PathBuf>,
    pub description: String,
    pub details: MigrationDetails,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PluginImportOutcome {
    pub succeeded_marketplaces: Vec<String>,
    pub succeeded_plugin_ids: Vec<String>,
    pub failed_marketplaces: Vec<String>,
    pub failed_plugin_ids: Vec<String>,
    pub raw_errors: Vec<ExternalAgentConfigImportRawError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigImportOutcome {
    pub pending_plugin_imports: Vec<PendingPluginImport>,
    pub item_results: Vec<ExternalAgentConfigImportItemResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigImportItemResult {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
    pub success_count: u32,
    pub error_count: u32,
    pub successes: Vec<ExternalAgentConfigImportSuccess>,
    pub raw_errors: Vec<ExternalAgentConfigImportRawError>,
}

impl ExternalAgentConfigImportItemResult {
    pub(crate) fn new(
        item_type: ExternalAgentConfigMigrationItemType,
        description: String,
        cwd: Option<PathBuf>,
    ) -> Self {
        Self {
            item_type,
            description,
            cwd,
            success_count: 0,
            error_count: 0,
            successes: Vec::new(),
            raw_errors: Vec::new(),
        }
    }

    pub(crate) fn record_error(&mut self, raw_error: ExternalAgentConfigImportRawError) {
        self.error_count = self.error_count.saturating_add(1);
        self.raw_errors.push(raw_error);
    }

    pub(crate) fn record_success(&mut self, source: Option<String>, target: Option<String>) {
        self.success_count = self.success_count.saturating_add(1);
        self.successes.push(ExternalAgentConfigImportSuccess {
            item_type: self.item_type,
            cwd: self.cwd.clone(),
            source,
            target,
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigImportSuccess {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigImportRawError {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub error_type: Option<String>,
    pub sub_error_type: Option<String>,
    pub failure_stage: String,
    pub message: String,
    pub cwd: Option<PathBuf>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
    pub details: Option<MigrationDetails>,
}

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigService {
    codex_home: PathBuf,
    external_agent_home: PathBuf,
    analytics_events_client: Option<AnalyticsEventsClient>,
    source: ExternalAgentSource,
}

impl ExternalAgentConfigService {
    pub(crate) fn new(codex_home: PathBuf, analytics_events_client: AnalyticsEventsClient) -> Self {
        let source = ExternalAgentSource::default();
        let external_agent_home = default_external_agent_home(source);
        Self {
            codex_home,
            external_agent_home,
            analytics_events_client: Some(analytics_events_client),
            source,
        }
    }

    #[cfg(test)]
    fn new_for_test(codex_home: PathBuf, external_agent_home: PathBuf) -> Self {
        Self {
            codex_home,
            external_agent_home,
            analytics_events_client: None,
            source: ExternalAgentSource::default(),
        }
    }

    pub(crate) async fn detect(
        &self,
        params: ExternalAgentConfigDetectOptions,
    ) -> io::Result<Vec<ExternalAgentConfigMigrationItem>> {
        let mut items = Vec::new();
        if params.include_home {
            self.detect_migrations(/*repo_root*/ None, &mut items)
                .await?;
        }

        for cwd in params.cwds.as_deref().unwrap_or(&[]) {
            let Some(repo_root) = find_repo_root(Some(cwd))? else {
                continue;
            };
            self.detect_migrations(Some(&repo_root), &mut items).await?;
        }

        Ok(items)
    }

    pub(crate) fn external_agent_session_source_path(
        &self,
        path: &Path,
    ) -> io::Result<Option<PathBuf>> {
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            return Ok(None);
        }
        let path = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let projects_root = match fs::canonicalize(self.external_agent_home.join("projects")) {
            Ok(projects_root) => projects_root,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(path.starts_with(projects_root).then_some(path))
    }

    pub(crate) async fn import(
        &self,
        migration_items: Vec<ExternalAgentConfigMigrationItem>,
    ) -> ExternalAgentConfigImportOutcome {
        let mut outcome = ExternalAgentConfigImportOutcome::default();
        for migration_item in migration_items {
            let item_type = migration_item.item_type;
            let description = migration_item.description.clone();
            let cwd_for_log = migration_item.cwd.clone();
            let mut item_result = ExternalAgentConfigImportItemResult::new(
                item_type,
                description.clone(),
                cwd_for_log.clone(),
            );
            let import_result = match migration_item.item_type {
                ExternalAgentConfigMigrationItemType::Config => (|| {
                    if let Some((source, target)) =
                        self.import_config(migration_item.cwd.as_deref())?
                    {
                        item_result.record_success(Some(source), Some(target));
                    }
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Config,
                        /*skills_count*/ None,
                    );
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Skills => (|| {
                    let imported_skills = self.import_skills(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Skills,
                        Some(imported_skills.len()),
                    );
                    for skill_name in imported_skills {
                        item_result.record_success(Some(skill_name.clone()), Some(skill_name));
                    }
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::AgentsMd => (|| {
                    if let Some((source, target)) =
                        self.import_agents_md(migration_item.cwd.as_deref())?
                    {
                        item_result.record_success(Some(source), Some(target));
                    }
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::AgentsMd,
                        /*skills_count*/ None,
                    );
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Plugins
                    if self.source.supports(SourceFeature::Plugins) =>
                {
                    async {
                        let cwd = migration_item.cwd;
                        let details = match migration_item.details {
                            Some(details) => details,
                            None => {
                                let err = invalid_data_error(
                                    "plugins migration item is missing details".to_string(),
                                );
                                record_import_error(
                                    &mut item_result,
                                    "plugin_import",
                                    err.to_string(),
                                    /*source*/ None,
                                );
                                return Err(err);
                            }
                        };
                        let (local_details, remote_details) = match self
                            .partition_plugin_migration_details(cwd.as_deref(), details)
                        {
                            Ok(details) => details,
                            Err(err) => {
                                record_import_error(
                                    &mut item_result,
                                    "plugin_import",
                                    err.to_string(),
                                    /*source*/ None,
                                );
                                return Err(err);
                            }
                        };

                        if let Some(local_details) = local_details {
                            let plugin_outcome = match self
                                .import_plugins(cwd.as_deref(), Some(local_details))
                                .await
                            {
                                Ok(plugin_outcome) => plugin_outcome,
                                Err(err) => {
                                    record_import_error(
                                        &mut item_result,
                                        "plugin_import",
                                        err.to_string(),
                                        /*source*/ None,
                                    );
                                    return Err(err);
                                }
                            };
                            for plugin_id in plugin_outcome.succeeded_plugin_ids {
                                item_result
                                    .record_success(Some(plugin_id.clone()), Some(plugin_id));
                            }
                            for raw_error in plugin_outcome.raw_errors {
                                item_result.record_error(raw_error);
                            }
                        }
                        if let Some(remote_details) = remote_details {
                            outcome.pending_plugin_imports.push(PendingPluginImport {
                                cwd,
                                description: description.clone(),
                                details: remote_details,
                            });
                        }
                        emit_migration_metric(
                            EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                            ExternalAgentConfigMigrationItemType::Plugins,
                            /*skills_count*/ None,
                        );
                        Ok(())
                    }
                    .await
                }
                ExternalAgentConfigMigrationItemType::Plugins => Ok(()),
                ExternalAgentConfigMigrationItemType::McpServerConfig => (|| {
                    let migrated_server_names =
                        self.import_mcp_server_config(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::McpServerConfig,
                        /*skills_count*/ None,
                    );
                    for server_name in migrated_server_names {
                        item_result.record_success(Some(server_name.clone()), Some(server_name));
                    }
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Subagents => (|| {
                    let imported_subagents =
                        self.import_subagents(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Subagents,
                        Some(imported_subagents.len()),
                    );
                    for subagent_name in imported_subagents {
                        item_result
                            .record_success(Some(subagent_name.clone()), Some(subagent_name));
                    }
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Hooks => (|| {
                    let migrated_hook_names = self.import_hooks(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Hooks,
                        /*skills_count*/ None,
                    );
                    for hook_name in migrated_hook_names {
                        item_result.record_success(Some(hook_name.clone()), Some(hook_name));
                    }
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Commands => (|| {
                    let imported_commands = self.import_commands(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Commands,
                        Some(imported_commands.len()),
                    );
                    for command_name in imported_commands {
                        item_result.record_success(Some(command_name.clone()), Some(command_name));
                    }
                    Ok(())
                })(),
                ExternalAgentConfigMigrationItemType::Sessions => Ok(()),
            };
            if let Err(err) = import_result
                && item_type != ExternalAgentConfigMigrationItemType::Plugins
            {
                let message = err.to_string();
                let error_type = if message.contains("invalid existing config.toml") {
                    "invalid_existing_config"
                } else {
                    "external_agent_config_import_error"
                };
                item_result.record_error(ExternalAgentConfigImportRawError {
                    item_type,
                    error_type: Some(error_type.to_string()),
                    sub_error_type: None,
                    failure_stage: "import_request_failed".to_string(),
                    message,
                    cwd: item_result.cwd.clone(),
                    source: None,
                });
            }
            outcome.item_results.push(item_result);
        }

        outcome
    }

    async fn detect_migrations(
        &self,
        repo_root: Option<&Path>,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
        let cwd = repo_root.map(Path::to_path_buf);
        let source_settings = self.source_settings(repo_root);
        let settings = if self.source.supports(SourceFeature::Config) {
            self.effective_source_settings(repo_root)?
        } else {
            None
        };
        let target_config = repo_root.map_or_else(
            || self.codex_home.join("config.toml"),
            |repo_root| repo_root.join(".codex").join("config.toml"),
        );
        if let Some(settings) = settings.as_ref() {
            let migrated = build_config_from_external(settings, self.source)?;
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

        let mcp_source_path = self.source.mcp_source_path(self.source_root(repo_root));
        let migrated_mcp = self.build_mcp_config(repo_root, settings.clone())?;
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

        let source_external_agent_dir = self.source_config_dir(repo_root);
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

        if self.source.supports(SourceFeature::Plugins)
            && self.source.can_detect_plugins(settings.as_ref())
        {
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
                    if let Some(detected) = self.source.detect_plugins(PluginDetectionContext {
                        external_agent_home: self.external_agent_home.as_path(),
                        source_settings: source_settings.as_path(),
                        source_root,
                        settings: settings.as_ref(),
                        configured_plugin_ids: &configured_plugin_ids,
                        configured_marketplace_plugins: &configured_marketplace_plugins,
                    })? {
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

        if repo_root.is_none() && self.source.supports(SourceFeature::Sessions) {
            let sessions = self
                .source
                .detect_recent_sessions(&self.external_agent_home, &self.codex_home)?;
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

    fn home_target_skills_dir(&self) -> PathBuf {
        self.codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"))
    }

    fn source_config_dir(&self, repo_root: Option<&Path>) -> PathBuf {
        repo_root.map_or_else(
            || self.external_agent_home.clone(),
            |repo_root| repo_root.join(self.source.config_dir()),
        )
    }

    fn source_settings(&self, repo_root: Option<&Path>) -> PathBuf {
        self.source_config_dir(repo_root)
            .join(self.source.settings_file_name(repo_root.is_some()))
    }

    fn effective_source_settings(&self, repo_root: Option<&Path>) -> io::Result<Option<JsonValue>> {
        let source_settings = self.source_settings(repo_root);
        self.source.effective_settings(&source_settings)
    }

    fn build_mcp_config(
        &self,
        repo_root: Option<&Path>,
        settings: Option<JsonValue>,
    ) -> io::Result<TomlValue> {
        let settings = if self.source.supports(SourceFeature::Config) {
            self.mcp_settings(repo_root, settings)?
        } else {
            None
        };
        self.source.build_mcp_config(
            self.source_root(repo_root).as_path(),
            self.external_agent_home.as_path(),
            settings.as_ref(),
        )
    }

    fn repo_agents_md_source_groups(
        &self,
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        self.source.repo_instruction_source_groups(repo_root)
    }

    fn home_agents_md_sources(&self) -> io::Result<Vec<PathBuf>> {
        self.source
            .home_instruction_sources(self.external_agent_home.as_path())
    }

    fn mcp_settings(
        &self,
        repo_root: Option<&Path>,
        source_settings: Option<JsonValue>,
    ) -> io::Result<Option<JsonValue>> {
        if repo_root.is_some() && source_settings.is_none() {
            let home_settings = self.source_settings(/*repo_root*/ None);
            match self.effective_source_settings(/*repo_root*/ None) {
                Ok(settings) => Ok(settings),
                Err(err) => {
                    tracing::warn!(
                        path = %home_settings.display(),
                        error = %err,
                        "ignoring invalid external agent home settings during repo MCP migration"
                    );
                    Ok(None)
                }
            }
        } else {
            Ok(source_settings)
        }
    }

    fn source_root(&self, repo_root: Option<&Path>) -> PathBuf {
        repo_root.map_or_else(
            || {
                self.external_agent_home
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            },
            Path::to_path_buf,
        )
    }

    fn marketplace_import_sources(
        &self,
        cwd: Option<&Path>,
    ) -> io::Result<BTreeMap<String, MarketplaceImportSource>> {
        let source_root = cwd.unwrap_or(self.external_agent_home.as_path());
        let source_settings = self.source_settings(cwd);
        self.source.marketplace_import_sources(
            self.external_agent_home.as_path(),
            source_root,
            &source_settings,
        )
    }

    fn partition_plugin_migration_details(
        &self,
        cwd: Option<&Path>,
        details: MigrationDetails,
    ) -> io::Result<(Option<MigrationDetails>, Option<MigrationDetails>)> {
        let import_sources = self.marketplace_import_sources(cwd)?;

        let mut local_plugins = Vec::new();
        let mut remote_plugins = Vec::new();
        for plugin_group in details.plugins {
            let is_local = import_sources
                .get(&plugin_group.marketplace_name)
                .and_then(|import_source| {
                    is_local_marketplace_source(
                        &import_source.source,
                        import_source.ref_name.clone(),
                    )
                    .ok()
                })
                .unwrap_or(false);

            if is_local {
                local_plugins.push(plugin_group);
            } else {
                remote_plugins.push(plugin_group);
            }
        }

        let local_details = (!local_plugins.is_empty()).then_some(MigrationDetails {
            plugins: local_plugins,
            ..Default::default()
        });
        let remote_details = (!remote_plugins.is_empty()).then_some(MigrationDetails {
            plugins: remote_plugins,
            ..Default::default()
        });

        Ok((local_details, remote_details))
    }

    pub(crate) async fn import_plugins(
        &self,
        cwd: Option<&Path>,
        details: Option<MigrationDetails>,
    ) -> io::Result<PluginImportOutcome> {
        let Some(MigrationDetails { plugins, .. }) = details else {
            return Err(invalid_data_error(
                "plugins migration item is missing details".to_string(),
            ));
        };
        let config = ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .fallback_cwd(Some(
                cwd.map(Path::to_path_buf)
                    .unwrap_or_else(|| self.codex_home.clone()),
            ))
            .build()
            .await
            .map_err(|err| io::Error::other(format!("failed to load config: {err}")))?;
        let requirements = config.config_layer_stack.requirements().clone();
        let mut outcome = PluginImportOutcome::default();
        let plugins_manager = PluginsManager::new(self.codex_home.clone())
            .with_plugin_install_source(PluginInstallSource::ExternalAgentMigration);
        if let Some(analytics_events_client) = self.analytics_events_client.clone() {
            plugins_manager.set_analytics_events_client(analytics_events_client);
        }
        let import_sources = self.marketplace_import_sources(cwd)?;
        for plugin_group in plugins {
            let marketplace_name = plugin_group.marketplace_name.clone();
            let plugin_names = plugin_group.plugin_names;
            let plugin_ids = plugin_names
                .iter()
                .map(|plugin_name| format!("{plugin_name}@{marketplace_name}"))
                .collect::<Vec<_>>();
            let import_source = import_sources.get(&marketplace_name).cloned();
            let Some(import_source) = import_source else {
                let message = format!(
                    "external agent plugin marketplace source was not found: {marketplace_name}"
                );
                record_plugin_import_errors(
                    &mut outcome,
                    cwd,
                    &plugin_ids,
                    "plugin_import",
                    message,
                );
                outcome.failed_marketplaces.push(marketplace_name);
                outcome.failed_plugin_ids.extend(plugin_ids);
                continue;
            };
            let request = MarketplaceAddRequest {
                source: import_source.source,
                ref_name: import_source.ref_name,
                sparse_paths: Vec::new(),
            };
            let add_marketplace_outcome =
                add_marketplace(self.codex_home.clone(), requirements.clone(), request).await;
            let marketplace_path = match add_marketplace_outcome {
                Ok(add_marketplace_outcome) => {
                    let Some(marketplace_path) = find_marketplace_manifest_path(
                        add_marketplace_outcome.installed_root.as_path(),
                    ) else {
                        let message = format!(
                            "plugin marketplace manifest was not found after install: {marketplace_name}"
                        );
                        record_plugin_import_errors(
                            &mut outcome,
                            cwd,
                            &plugin_ids,
                            "plugin_import",
                            message,
                        );
                        outcome.failed_marketplaces.push(marketplace_name);
                        outcome.failed_plugin_ids.extend(plugin_ids);
                        continue;
                    };
                    outcome
                        .succeeded_marketplaces
                        .push(marketplace_name.clone());
                    marketplace_path
                }
                Err(err) => {
                    record_plugin_import_errors(
                        &mut outcome,
                        cwd,
                        &plugin_ids,
                        "plugin_import",
                        err.to_string(),
                    );
                    outcome.failed_marketplaces.push(marketplace_name);
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                }
            };
            let install_config = match ConfigBuilder::default()
                .codex_home(self.codex_home.clone())
                .fallback_cwd(Some(
                    cwd.map(Path::to_path_buf)
                        .unwrap_or_else(|| self.codex_home.clone()),
                ))
                .build()
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    record_plugin_import_errors(
                        &mut outcome,
                        cwd,
                        &plugin_ids,
                        "plugin_import",
                        format!("failed to reload config after adding marketplace: {err}"),
                    );
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                }
            };
            for plugin_name in plugin_names {
                match plugins_manager
                    .install_plugin(
                        &install_config.config_layer_stack,
                        PluginInstallRequest {
                            plugin_name: plugin_name.clone(),
                            marketplace_path: marketplace_path.clone(),
                        },
                    )
                    .await
                {
                    Ok(_) => outcome
                        .succeeded_plugin_ids
                        .push(format!("{plugin_name}@{marketplace_name}")),
                    Err(err) => {
                        let plugin_id = format!("{plugin_name}@{marketplace_name}");
                        outcome.failed_plugin_ids.push(plugin_id.clone());
                        let sub_error_type = err.sub_error_type();
                        let mut raw_error = plugin_import_raw_error(
                            cwd,
                            "plugin_import",
                            err.to_string(),
                            Some(plugin_id),
                        );
                        raw_error.sub_error_type = sub_error_type;
                        if matches!(
                            err,
                            PluginInstallError::Marketplace(
                                MarketplaceError::PluginNotFound { .. }
                            )
                        ) {
                            raw_error.error_type = Some("plugin_not_found".to_string());
                        }
                        outcome.raw_errors.push(raw_error);
                    }
                }
            }
        }

        Ok(outcome)
    }

    fn import_config(&self, cwd: Option<&Path>) -> io::Result<Option<(String, String)>> {
        if !self.source.supports(SourceFeature::Config) {
            return Ok(None);
        }
        let repo_root = find_repo_root(cwd)?;
        let (source_settings, target_config) = if let Some(repo_root) = repo_root.as_ref() {
            (
                self.source_settings(Some(repo_root)),
                repo_root.join(".codex").join("config.toml"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(None);
        } else {
            (
                self.source_settings(/*repo_root*/ None),
                self.codex_home.join("config.toml"),
            )
        };
        let Some(settings) = self.effective_source_settings(repo_root.as_deref())? else {
            return Ok(None);
        };
        let migrated = build_config_from_external(&settings, self.source)?;
        if is_empty_toml_table(&migrated) {
            return Ok(None);
        }

        let Some(target_parent) = target_config.parent() else {
            return Err(invalid_data_error("config target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;
        if !target_config.exists() {
            write_toml_file(&target_config, &migrated)?;
            return Ok(Some((
                source_settings.display().to_string(),
                target_config.display().to_string(),
            )));
        }

        let existing_raw = fs::read_to_string(&target_config)?;
        let mut existing = if existing_raw.trim().is_empty() {
            TomlValue::Table(Default::default())
        } else {
            toml::from_str::<TomlValue>(&existing_raw)
                .map_err(|err| invalid_data_error(format!("invalid existing config.toml: {err}")))?
        };

        let changed = merge_missing_toml_values(&mut existing, &migrated)?;
        if !changed {
            return Ok(None);
        }

        write_toml_file(&target_config, &existing)?;
        Ok(Some((
            source_settings.display().to_string(),
            target_config.display().to_string(),
        )))
    }

    fn import_mcp_server_config(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let repo_root = find_repo_root(cwd)?;
        let target_config = if let Some(repo_root) = repo_root.as_ref() {
            repo_root.join(".codex").join("config.toml")
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(Vec::new());
        } else {
            self.codex_home.join("config.toml")
        };
        let settings = if self.source.supports(SourceFeature::Config) {
            self.effective_source_settings(repo_root.as_deref())?
        } else {
            None
        };
        let migrated = self.build_mcp_config(repo_root.as_deref(), settings)?;
        if is_empty_toml_table(&migrated) {
            return Ok(Vec::new());
        }

        let Some(target_parent) = target_config.parent() else {
            return Err(invalid_data_error("config target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;
        if !target_config.exists() {
            let migrated_server_names = migrated_mcp_server_names(&migrated);
            write_toml_file(&target_config, &migrated)?;
            return Ok(migrated_server_names);
        }

        let existing_raw = fs::read_to_string(&target_config)?;
        let mut existing = if existing_raw.trim().is_empty() {
            TomlValue::Table(Default::default())
        } else {
            toml::from_str::<TomlValue>(&existing_raw)
                .map_err(|err| invalid_data_error(format!("invalid existing config.toml: {err}")))?
        };
        let merged_server_names = merge_missing_mcp_servers(&mut existing, &migrated)?;
        if !merged_server_names.is_empty() {
            write_toml_file(&target_config, &existing)?;
        }
        Ok(merged_server_names)
    }

    fn import_subagents(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let (source_agents, target_agents) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(self.source.config_dir()).join("agents"),
                repo_root.join(".codex").join("agents"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(Vec::new());
        } else {
            (
                self.external_agent_home.join("agents"),
                self.codex_home.join("agents"),
            )
        };

        self.source.import_subagents(&source_agents, &target_agents)
    }

    fn import_hooks(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let (source_external_agent_dir, target_hooks) =
            if let Some(repo_root) = find_repo_root(cwd)? {
                (
                    self.source_config_dir(Some(repo_root.as_path())),
                    repo_root.join(".codex").join("hooks.json"),
                )
            } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
                return Ok(Vec::new());
            } else {
                (
                    self.external_agent_home.clone(),
                    self.codex_home.join("hooks.json"),
                )
            };

        let hook_names = self
            .source
            .hook_event_names(&source_external_agent_dir, &target_hooks)?;
        if self
            .source
            .import_hooks(&source_external_agent_dir, &target_hooks)?
        {
            Ok(hook_names)
        } else {
            Ok(Vec::new())
        }
    }

    fn import_commands(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let (source_commands, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(self.source.config_dir()).join("commands"),
                repo_root.join(".agents").join("skills"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(Vec::new());
        } else {
            (
                self.external_agent_home.join("commands"),
                self.home_target_skills_dir(),
            )
        };

        self.source
            .import_commands(&source_commands, &target_skills)
    }

    fn import_skills(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let (source_skills, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(self.source.config_dir()).join("skills"),
                repo_root.join(".agents").join("skills"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(Vec::new());
        } else {
            (
                self.external_agent_home.join("skills"),
                self.home_target_skills_dir(),
            )
        };
        if !source_skills.is_dir() {
            return Ok(Vec::new());
        }

        fs::create_dir_all(&target_skills)?;
        let mut copied_names = Vec::new();

        for entry in fs::read_dir(&source_skills)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }

            let target = target_skills.join(entry.file_name());
            if target.exists() {
                continue;
            }

            copy_dir_recursive(&entry.path(), &target, self.source.rewrite_profile())?;
            copied_names.push(entry.file_name().to_string_lossy().to_string());
        }

        Ok(copied_names)
    }

    fn import_agents_md(&self, cwd: Option<&Path>) -> io::Result<Option<(String, String)>> {
        let (source_agents_md, target_agents_md) =
            if let Some(requested_scope) = cwd.filter(|cwd| !cwd.as_os_str().is_empty()) {
                let Some(repo_root) = find_repo_root(Some(requested_scope))? else {
                    return Ok(None);
                };
                let Some(group) = self
                    .repo_agents_md_source_groups(&repo_root)?
                    .into_iter()
                    .find(|group| group.scope == repo_root)
                else {
                    return Ok(None);
                };
                let target_agents_md = group.scope.join("AGENTS.md");
                (group.sources, target_agents_md)
            } else {
                let source_agents_md = self.home_agents_md_sources()?;
                if source_agents_md.is_empty() {
                    return Ok(None);
                }
                (source_agents_md, self.codex_home.join("AGENTS.md"))
            };
        if !is_missing_or_empty_text_file(&target_agents_md)? {
            return Ok(None);
        }

        let Some(target_parent) = target_agents_md.parent() else {
            return Err(invalid_data_error("AGENTS.md target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;

        let source_contents = source_agents_md
            .iter()
            .map(|source| {
                self.source.read_instruction_source(source).map(|contents| {
                    rewrite_external_agent_terms(&contents, self.source.rewrite_profile())
                })
            })
            .collect::<io::Result<Vec<_>>>()?
            .join("\n\n");
        fs::write(&target_agents_md, source_contents)?;
        Ok(Some((
            display_source_paths(&source_agents_md),
            target_agents_md.display().to_string(),
        )))
    }
}

fn default_external_agent_home(source: ExternalAgentSource) -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(source.config_dir());
    }

    PathBuf::from(source.config_dir())
}

fn read_external_settings(path: &Path) -> io::Result<Option<JsonValue>> {
    if !path.is_file() {
        return Ok(None);
    }

    let raw_settings = fs::read_to_string(path)?;
    let settings =
        serde_json::from_str(&raw_settings).map_err(|err| invalid_data_error(err.to_string()))?;
    Ok(Some(settings))
}

fn configured_marketplace_plugins(
    config: &Config,
    plugins_manager: &PluginsManager,
) -> io::Result<BTreeMap<String, HashSet<String>>> {
    let plugins_input = config.plugins_config_input();
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(&plugins_input, &[], /*include_openai_curated*/ true)
        .map_err(|err| {
            invalid_data_error(format!("failed to list configured marketplaces: {err}"))
        })?;
    let mut marketplace_plugins = BTreeMap::new();
    for marketplace in marketplaces.marketplaces {
        let plugins = marketplace
            .plugins
            .into_iter()
            .filter(|plugin| {
                plugin.policy.installation != MarketplacePluginInstallPolicy::NotAvailable
            })
            .filter(|plugin| {
                plugin
                    .policy
                    .products
                    .as_deref()
                    .is_none_or(|products| Product::Codex.matches_product_restriction(products))
            })
            .map(|plugin| plugin.name)
            .collect::<HashSet<_>>();
        marketplace_plugins.insert(marketplace.name, plugins);
    }
    Ok(marketplace_plugins)
}

fn find_repo_root(cwd: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let Some(cwd) = cwd.filter(|cwd| !cwd.as_os_str().is_empty()) else {
        return Ok(None);
    };

    let mut current = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        std::env::current_dir()?.join(cwd)
    };

    if !current.exists() {
        return Ok(None);
    }

    if current.is_file() {
        let Some(parent) = current.parent() else {
            return Ok(None);
        };
        current = parent.to_path_buf();
    }

    let fallback = current.clone();
    loop {
        let git_path = current.join(".git");
        if git_path.is_dir() || git_path.is_file() {
            return Ok(Some(current));
        }
        if !current.pop() {
            break;
        }
    }

    Ok(Some(fallback))
}

fn collect_subdirectory_names(path: &Path) -> io::Result<HashSet<OsString>> {
    let mut names = HashSet::new();
    if !path.is_dir() {
        return Ok(names);
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.insert(entry.file_name());
        }
    }

    Ok(names)
}

fn missing_subdirectory_names(source: &Path, target: &Path) -> io::Result<Vec<String>> {
    let source_names = collect_subdirectory_names(source)?;
    let target_names = collect_subdirectory_names(target)?;
    let mut missing_names = source_names
        .into_iter()
        .filter(|name| !target_names.contains(name))
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    missing_names.sort();
    Ok(missing_names)
}

fn is_missing_or_empty_text_file(path: &Path) -> io::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    if !path.is_file() {
        return Ok(false);
    }

    Ok(fs::read_to_string(path)?.trim().is_empty())
}

fn is_non_empty_text_file(path: &Path) -> io::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }

    Ok(!fs::read_to_string(path)?.trim().is_empty())
}

fn build_config_from_external(
    settings: &JsonValue,
    source: ExternalAgentSource,
) -> io::Result<TomlValue> {
    let Some(settings_obj) = settings.as_object() else {
        return Err(invalid_data_error(
            "external agent settings root must be an object",
        ));
    };

    let mut root = toml::map::Map::new();

    if let Some(env) = settings_obj.get("env").and_then(JsonValue::as_object)
        && !env.is_empty()
    {
        let mut shell_policy = toml::map::Map::new();
        shell_policy.insert("inherit".to_string(), TomlValue::String("core".to_string()));
        shell_policy.insert(
            "set".to_string(),
            TomlValue::Table(json_object_to_env_toml_table(env)),
        );
        root.insert(
            "shell_environment_policy".to_string(),
            TomlValue::Table(shell_policy),
        );
    }

    source.append_config(&mut root, settings_obj);

    Ok(TomlValue::Table(root))
}

fn json_object_to_env_toml_table(
    object: &serde_json::Map<String, JsonValue>,
) -> toml::map::Map<String, TomlValue> {
    let mut table = toml::map::Map::new();
    for (key, value) in object {
        if let Some(value) = json_env_value_to_string(value) {
            table.insert(key.clone(), TomlValue::String(value));
        }
    }
    table
}

fn json_env_value_to_string(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Null => None,
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

fn merge_missing_toml_values(existing: &mut TomlValue, incoming: &TomlValue) -> io::Result<bool> {
    match (existing, incoming) {
        (TomlValue::Table(existing_table), TomlValue::Table(incoming_table)) => {
            let mut changed = false;
            for (key, incoming_value) in incoming_table {
                match existing_table.get_mut(key) {
                    Some(existing_value) => {
                        if matches!(
                            (&*existing_value, incoming_value),
                            (TomlValue::Table(_), TomlValue::Table(_))
                        ) && merge_missing_toml_values(existing_value, incoming_value)?
                        {
                            changed = true;
                        }
                    }
                    None => {
                        existing_table.insert(key.clone(), incoming_value.clone());
                        changed = true;
                    }
                }
            }
            Ok(changed)
        }
        _ => Err(invalid_data_error(
            "expected TOML table while merging migrated config values",
        )),
    }
}

fn merge_missing_mcp_servers(
    existing: &mut TomlValue,
    incoming: &TomlValue,
) -> io::Result<Vec<String>> {
    let existing_root = existing
        .as_table_mut()
        .ok_or_else(|| invalid_data_error("expected existing config to be a TOML table"))?;
    let incoming_root = incoming
        .as_table()
        .ok_or_else(|| invalid_data_error("expected migrated MCP config to be a TOML table"))?;
    let Some(incoming_servers) = incoming_root.get("mcp_servers") else {
        return Ok(Vec::new());
    };
    let incoming_servers = incoming_servers
        .as_table()
        .ok_or_else(|| invalid_data_error("expected migrated MCP servers to be a TOML table"))?;
    let Some(existing_servers) = existing_root.get_mut("mcp_servers") else {
        existing_root.insert(
            "mcp_servers".to_string(),
            TomlValue::Table(incoming_servers.clone()),
        );
        return Ok(incoming_servers.keys().cloned().collect());
    };
    let Some(existing_servers) = existing_servers.as_table_mut() else {
        return Ok(Vec::new());
    };

    let mut merged_server_names = Vec::new();
    for (server_name, incoming_server) in incoming_servers {
        if !existing_servers.contains_key(server_name) {
            existing_servers.insert(server_name.clone(), incoming_server.clone());
            merged_server_names.push(server_name.clone());
        }
    }
    Ok(merged_server_names)
}

fn write_toml_file(path: &Path, value: &TomlValue) -> io::Result<()> {
    let serialized = toml::to_string_pretty(value)
        .map_err(|err| invalid_data_error(format!("failed to serialize config.toml: {err}")))?;
    fs::write(path, format!("{}\n", serialized.trim_end()))
}

fn migrated_mcp_server_names(value: &TomlValue) -> Vec<String> {
    value
        .get("mcp_servers")
        .and_then(TomlValue::as_table)
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default()
}

fn named_migrations(names: Vec<String>) -> Vec<NamedMigration> {
    names
        .into_iter()
        .map(|name| NamedMigration { name })
        .collect()
}

fn is_empty_toml_table(value: &TomlValue) -> bool {
    match value {
        TomlValue::Table(table) => table.is_empty(),
        TomlValue::String(_)
        | TomlValue::Integer(_)
        | TomlValue::Float(_)
        | TomlValue::Boolean(_)
        | TomlValue::Datetime(_)
        | TomlValue::Array(_) => false,
    }
}

fn invalid_data_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn migration_item_type_label(item_type: ExternalAgentConfigMigrationItemType) -> &'static str {
    match item_type {
        ExternalAgentConfigMigrationItemType::Config => "config",
        ExternalAgentConfigMigrationItemType::Skills => "skills",
        ExternalAgentConfigMigrationItemType::AgentsMd => "agents_md",
        ExternalAgentConfigMigrationItemType::Plugins => "plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "mcp_server_config",
        ExternalAgentConfigMigrationItemType::Subagents => "subagents",
        ExternalAgentConfigMigrationItemType::Hooks => "hooks",
        ExternalAgentConfigMigrationItemType::Commands => "commands",
        ExternalAgentConfigMigrationItemType::Sessions => "sessions",
    }
}

pub(crate) fn record_import_error(
    result: &mut ExternalAgentConfigImportItemResult,
    failure_stage: &'static str,
    message: impl Into<String>,
    source: Option<String>,
) {
    result.record_error(ExternalAgentConfigImportRawError {
        item_type: result.item_type,
        error_type: None,
        sub_error_type: None,
        failure_stage: failure_stage.to_string(),
        message: message.into(),
        cwd: result.cwd.clone(),
        source,
    });
}

fn record_plugin_import_errors(
    outcome: &mut PluginImportOutcome,
    cwd: Option<&Path>,
    plugin_ids: &[String],
    failure_stage: &'static str,
    message: impl Into<String>,
) {
    let message = message.into();
    outcome
        .raw_errors
        .extend(plugin_ids.iter().map(|plugin_id| {
            plugin_import_raw_error(cwd, failure_stage, message.clone(), Some(plugin_id.clone()))
        }));
}

fn plugin_import_raw_error(
    cwd: Option<&Path>,
    failure_stage: &'static str,
    message: String,
    source: Option<String>,
) -> ExternalAgentConfigImportRawError {
    ExternalAgentConfigImportRawError {
        item_type: ExternalAgentConfigMigrationItemType::Plugins,
        error_type: None,
        sub_error_type: None,
        failure_stage: failure_stage.to_string(),
        message,
        cwd: cwd.map(Path::to_path_buf),
        source,
    }
}

fn migration_metric_tags(
    item_type: ExternalAgentConfigMigrationItemType,
    skills_count: Option<usize>,
) -> Vec<(&'static str, String)> {
    let mut tags = vec![(
        "migration_type",
        migration_item_type_label(item_type).to_string(),
    )];
    if matches!(
        item_type,
        ExternalAgentConfigMigrationItemType::Skills
            | ExternalAgentConfigMigrationItemType::Subagents
            | ExternalAgentConfigMigrationItemType::Commands
    ) {
        tags.push(("skills_count", skills_count.unwrap_or(0).to_string()));
    }
    tags
}

fn emit_migration_metric(
    metric_name: &str,
    item_type: ExternalAgentConfigMigrationItemType,
    skills_count: Option<usize>,
) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let tags = migration_metric_tags(item_type, skills_count);
    let tag_refs = tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    let _ = metrics.counter(metric_name, /*inc*/ 1, &tag_refs);
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
