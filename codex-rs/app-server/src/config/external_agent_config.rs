use codex_config::types::PluginConfig;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_add::is_local_marketplace_source;
use codex_external_agent_migration::build_mcp_config_from_external;
use codex_external_agent_migration::count_missing_commands;
use codex_external_agent_migration::count_missing_subagents;
use codex_external_agent_migration::hook_migration_event_names;
use codex_external_agent_migration::import_commands;
use codex_external_agent_migration::import_hooks;
use codex_external_agent_migration::import_subagents;
use codex_external_agent_migration::missing_command_names;
use codex_external_agent_migration::missing_subagent_names;
use codex_external_agent_sessions::ExternalAgentSessionMigration;
use codex_external_agent_sessions::detect_recent_sessions;
use codex_plugin::PluginId;
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

const EXTERNAL_AGENT_CONFIG_DETECT_METRIC: &str = "codex.external_agent_config.detect";
const EXTERNAL_AGENT_CONFIG_IMPORT_METRIC: &str = "codex.external_agent_config.import";
const EXTERNAL_AGENT_DIR: &str = ".claude";
const EXTERNAL_AGENT_CONFIG_MD: &str = "CLAUDE.md";
const EXTERNAL_OFFICIAL_MARKETPLACE_NAME: &str = "claude-plugins-official";
const EXTERNAL_OFFICIAL_MARKETPLACE_SOURCE: &str = "anthropics/claude-plugins-official";

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
    pub sessions: Vec<ExternalAgentSessionMigration>,
    pub mcp_servers: Vec<NamedMigration>,
    pub hooks: Vec<NamedMigration>,
    pub subagents: Vec<NamedMigration>,
    pub commands: Vec<NamedMigration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingPluginImport {
    pub cwd: Option<PathBuf>,
    pub details: MigrationDetails,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PluginImportOutcome {
    pub succeeded_marketplaces: Vec<String>,
    pub succeeded_plugin_ids: Vec<String>,
    pub failed_marketplaces: Vec<String>,
    pub failed_plugin_ids: Vec<String>,
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
}

impl ExternalAgentConfigService {
    pub(crate) fn new(codex_home: PathBuf) -> Self {
        let external_agent_home = default_external_agent_home();
        Self {
            codex_home,
            external_agent_home,
        }
    }

    #[cfg(test)]
    fn new_for_test(codex_home: PathBuf, external_agent_home: PathBuf) -> Self {
        Self {
            codex_home,
            external_agent_home,
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
    ) -> io::Result<Vec<PendingPluginImport>> {
        let mut pending_plugin_imports = Vec::new();
        for migration_item in migration_items {
            match migration_item.item_type {
                ExternalAgentConfigMigrationItemType::Config => {
                    self.import_config(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Config,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::Skills => {
                    let skills_count = self.import_skills(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Skills,
                        Some(skills_count),
                    );
                }
                ExternalAgentConfigMigrationItemType::AgentsMd => {
                    self.import_agents_md(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::AgentsMd,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::Plugins => {
                    let cwd = migration_item.cwd;
                    let details = migration_item.details.ok_or_else(|| {
                        invalid_data_error("plugins migration item is missing details".to_string())
                    })?;
                    let (local_details, remote_details) =
                        self.partition_plugin_migration_details(cwd.as_deref(), details)?;

                    if let Some(local_details) = local_details {
                        self.import_plugins(cwd.as_deref(), Some(local_details))
                            .await?;
                    }
                    if let Some(remote_details) = remote_details {
                        pending_plugin_imports.push(PendingPluginImport {
                            cwd,
                            details: remote_details,
                        });
                    }
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Plugins,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::McpServerConfig => {
                    self.import_mcp_server_config(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::McpServerConfig,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::Subagents => {
                    let subagents_count = self.import_subagents(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Subagents,
                        Some(subagents_count),
                    );
                }
                ExternalAgentConfigMigrationItemType::Hooks => {
                    self.import_hooks(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Hooks,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::Commands => {
                    let commands_count = self.import_commands(migration_item.cwd.as_deref())?;
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Commands,
                        Some(commands_count),
                    );
                }
                ExternalAgentConfigMigrationItemType::Sessions => {}
            }
        }

        Ok(pending_plugin_imports)
    }

    async fn detect_migrations(
        &self,
        repo_root: Option<&Path>,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
        let cwd = repo_root.map(Path::to_path_buf);
        let source_settings = repo_root.map_or_else(
            || self.external_agent_home.join("settings.json"),
            |repo_root| repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        );
        let settings = effective_external_settings(&source_settings)?;
        let target_config = repo_root.map_or_else(
            || self.codex_home.join("config.toml"),
            |repo_root| repo_root.join(".codex").join("config.toml"),
        );
        if let Some(settings) = settings.as_ref() {
            let migrated = build_config_from_external(settings)?;
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

        let source_root = self.source_root(repo_root);
        let mcp_settings = self.mcp_settings(repo_root, settings.clone())?;
        let migrated_mcp = build_mcp_config_from_external(
            source_root.as_path(),
            Some(self.external_agent_home.as_path()),
            mcp_settings.as_ref(),
        )?;
        let mcp_server_names = migrated_mcp_server_names(&migrated_mcp);
        if !is_empty_toml_table(&migrated_mcp) {
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
                should_include = merge_missing_toml_values(&mut existing, &migrated_mcp)?;
            }

            if should_include {
                items.push(ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
                    description: format!(
                        "Migrate MCP servers from {} into {}",
                        source_root.display(),
                        target_config.display()
                    ),
                    cwd: cwd.clone(),
                    details: Some(MigrationDetails {
                        mcp_servers: named_migrations(mcp_server_names.clone()),
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

        let source_external_agent_dir = repo_root.map_or_else(
            || self.external_agent_home.clone(),
            |repo_root| repo_root.join(EXTERNAL_AGENT_DIR),
        );
        let target_hooks = repo_root.map_or_else(
            || self.codex_home.join("hooks.json"),
            |repo_root| repo_root.join(".codex").join("hooks.json"),
        );
        let hook_event_names =
            hook_migration_event_names(source_external_agent_dir.as_path(), &target_hooks)?;
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
            |repo_root| repo_root.join(EXTERNAL_AGENT_DIR).join("skills"),
        );
        let target_skills = repo_root.map_or_else(
            || self.home_target_skills_dir(),
            |repo_root| repo_root.join(".agents").join("skills"),
        );
        let skills_count = count_missing_subdirectories(&source_skills, &target_skills)?;
        if skills_count > 0 {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Migrate skills from {} to {}",
                    source_skills.display(),
                    target_skills.display()
                ),
                cwd: cwd.clone(),
                details: None,
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
        let commands_count = count_missing_commands(&source_commands, &target_command_skills)?;
        if commands_count > 0 {
            let command_names = missing_command_names(&source_commands, &target_command_skills)?;
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

        let source_agents_md = if let Some(repo_root) = repo_root {
            find_repo_agents_md_source(repo_root)?
        } else {
            let path = self.external_agent_home.join(EXTERNAL_AGENT_CONFIG_MD);
            is_non_empty_text_file(&path)?.then_some(path)
        };
        let target_agents_md = repo_root.map_or_else(
            || self.codex_home.join("AGENTS.md"),
            |repo_root| repo_root.join("AGENTS.md"),
        );
        if let Some(source_agents_md) = source_agents_md
            && is_missing_or_empty_text_file(&target_agents_md)?
        {
            items.push(ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Migrate {} to {}",
                    source_agents_md.display(),
                    target_agents_md.display()
                ),
                cwd: cwd.clone(),
                details: None,
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::AgentsMd,
                /*skills_count*/ None,
            );
        }

        if let Some(settings) = settings.as_ref() {
            match ConfigBuilder::default()
                .codex_home(self.codex_home.clone())
                .fallback_cwd(Some(self.codex_home.clone()))
                .build()
                .await
            {
                Ok(config) => {
                    let configured_plugin_ids = config
                        .config_layer_stack
                        .get_user_layer()
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
                    if let Some(item) = self.detect_plugin_migration(
                        source_settings.as_path(),
                        repo_root.unwrap_or(self.external_agent_home.as_path()),
                        cwd.clone(),
                        settings,
                        &configured_plugin_ids,
                        &configured_marketplace_plugins,
                    ) {
                        items.push(item);
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

        if repo_root.is_none() {
            let sessions = detect_recent_sessions(&self.external_agent_home, &self.codex_home)?;
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

    fn mcp_settings(
        &self,
        repo_root: Option<&Path>,
        source_settings: Option<JsonValue>,
    ) -> io::Result<Option<JsonValue>> {
        if repo_root.is_some() && source_settings.is_none() {
            let home_settings = self.external_agent_home.join("settings.json");
            match effective_external_settings(&home_settings) {
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

    fn detect_plugin_migration(
        &self,
        source_settings: &Path,
        source_root: &Path,
        cwd: Option<PathBuf>,
        settings: &JsonValue,
        configured_plugin_ids: &HashSet<String>,
        configured_marketplace_plugins: &BTreeMap<String, HashSet<String>>,
    ) -> Option<ExternalAgentConfigMigrationItem> {
        let plugin_details = extract_plugin_migration_details(
            settings,
            source_root,
            configured_plugin_ids,
            configured_marketplace_plugins,
        )?;
        emit_migration_metric(
            EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
            ExternalAgentConfigMigrationItemType::Plugins,
            /*skills_count*/ None,
        );

        Some(ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!("Migrate enabled plugins from {}", source_settings.display()),
            cwd,
            details: Some(plugin_details),
        })
    }

    fn partition_plugin_migration_details(
        &self,
        cwd: Option<&Path>,
        details: MigrationDetails,
    ) -> io::Result<(Option<MigrationDetails>, Option<MigrationDetails>)> {
        let source_settings = cwd.map_or_else(
            || self.external_agent_home.join("settings.json"),
            |cwd| cwd.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        );
        let source_root = cwd.unwrap_or(self.external_agent_home.as_path());
        let import_sources = effective_external_settings(&source_settings)?
            .map(|settings| collect_marketplace_import_sources(&settings, source_root))
            .unwrap_or_default();

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
        let mut outcome = PluginImportOutcome::default();
        let plugins_manager = PluginsManager::new(self.codex_home.clone());
        for plugin_group in plugins {
            let marketplace_name = plugin_group.marketplace_name.clone();
            let plugin_names = plugin_group.plugin_names;
            let plugin_ids = plugin_names
                .iter()
                .map(|plugin_name| format!("{plugin_name}@{marketplace_name}"))
                .collect::<Vec<_>>();
            let source_settings = cwd.map_or_else(
                || self.external_agent_home.join("settings.json"),
                |cwd| cwd.join(EXTERNAL_AGENT_DIR).join("settings.json"),
            );
            let source_root = cwd.unwrap_or(self.external_agent_home.as_path());
            let import_source =
                effective_external_settings(&source_settings)?.and_then(|settings| {
                    collect_marketplace_import_sources(&settings, source_root)
                        .remove(&marketplace_name)
                });
            let Some(import_source) = import_source else {
                outcome.failed_marketplaces.push(marketplace_name);
                outcome.failed_plugin_ids.extend(plugin_ids);
                continue;
            };
            let request = MarketplaceAddRequest {
                source: import_source.source,
                ref_name: import_source.ref_name,
                sparse_paths: Vec::new(),
            };
            let add_marketplace_outcome = add_marketplace(self.codex_home.clone(), request).await;
            let marketplace_path = match add_marketplace_outcome {
                Ok(add_marketplace_outcome) => {
                    let Some(marketplace_path) = find_marketplace_manifest_path(
                        add_marketplace_outcome.installed_root.as_path(),
                    ) else {
                        outcome.failed_marketplaces.push(marketplace_name);
                        outcome.failed_plugin_ids.extend(plugin_ids);
                        continue;
                    };
                    outcome
                        .succeeded_marketplaces
                        .push(marketplace_name.clone());
                    marketplace_path
                }
                Err(_) => {
                    outcome.failed_marketplaces.push(marketplace_name);
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                }
            };
            for plugin_name in plugin_names {
                match plugins_manager
                    .install_plugin(PluginInstallRequest {
                        plugin_name: plugin_name.clone(),
                        marketplace_path: marketplace_path.clone(),
                    })
                    .await
                {
                    Ok(_) => outcome
                        .succeeded_plugin_ids
                        .push(format!("{plugin_name}@{marketplace_name}")),
                    Err(_) => outcome
                        .failed_plugin_ids
                        .push(format!("{plugin_name}@{marketplace_name}")),
                }
            }
        }

        Ok(outcome)
    }

    fn import_config(&self, cwd: Option<&Path>) -> io::Result<()> {
        let repo_root = find_repo_root(cwd)?;
        let (source_settings, target_config) = if let Some(repo_root) = repo_root.as_ref() {
            (
                repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
                repo_root.join(".codex").join("config.toml"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.external_agent_home.join("settings.json"),
                self.codex_home.join("config.toml"),
            )
        };
        let Some(settings) = effective_external_settings(&source_settings)? else {
            return Ok(());
        };
        let migrated = build_config_from_external(&settings)?;
        if is_empty_toml_table(&migrated) {
            return Ok(());
        }

        let Some(target_parent) = target_config.parent() else {
            return Err(invalid_data_error("config target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;
        if !target_config.exists() {
            write_toml_file(&target_config, &migrated)?;
            return Ok(());
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
            return Ok(());
        }

        write_toml_file(&target_config, &existing)?;
        Ok(())
    }

    fn import_mcp_server_config(&self, cwd: Option<&Path>) -> io::Result<()> {
        let repo_root = find_repo_root(cwd)?;
        let (source_settings, target_config) = if let Some(repo_root) = repo_root.as_ref() {
            (
                repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
                repo_root.join(".codex").join("config.toml"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.external_agent_home.join("settings.json"),
                self.codex_home.join("config.toml"),
            )
        };
        let settings = self.mcp_settings(
            repo_root.as_deref(),
            effective_external_settings(&source_settings)?,
        )?;
        let migrated = build_mcp_config_from_external(
            self.source_root(repo_root.as_deref()).as_path(),
            Some(self.external_agent_home.as_path()),
            settings.as_ref(),
        )?;
        if is_empty_toml_table(&migrated) {
            return Ok(());
        }

        let Some(target_parent) = target_config.parent() else {
            return Err(invalid_data_error("config target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;
        if !target_config.exists() {
            write_toml_file(&target_config, &migrated)?;
            return Ok(());
        }

        let existing_raw = fs::read_to_string(&target_config)?;
        let mut existing = if existing_raw.trim().is_empty() {
            TomlValue::Table(Default::default())
        } else {
            toml::from_str::<TomlValue>(&existing_raw)
                .map_err(|err| invalid_data_error(format!("invalid existing config.toml: {err}")))?
        };
        if merge_missing_toml_values(&mut existing, &migrated)? {
            write_toml_file(&target_config, &existing)?;
        }
        Ok(())
    }

    fn import_subagents(&self, cwd: Option<&Path>) -> io::Result<usize> {
        let (source_agents, target_agents) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(EXTERNAL_AGENT_DIR).join("agents"),
                repo_root.join(".codex").join("agents"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(0);
        } else {
            (
                self.external_agent_home.join("agents"),
                self.codex_home.join("agents"),
            )
        };

        import_subagents(&source_agents, &target_agents)
    }

    fn import_hooks(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_external_agent_dir, target_hooks) =
            if let Some(repo_root) = find_repo_root(cwd)? {
                (
                    repo_root.join(EXTERNAL_AGENT_DIR),
                    repo_root.join(".codex").join("hooks.json"),
                )
            } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
                return Ok(());
            } else {
                (
                    self.external_agent_home.clone(),
                    self.codex_home.join("hooks.json"),
                )
            };

        import_hooks(&source_external_agent_dir, &target_hooks)?;
        Ok(())
    }

    fn import_commands(&self, cwd: Option<&Path>) -> io::Result<usize> {
        let (source_commands, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(EXTERNAL_AGENT_DIR).join("commands"),
                repo_root.join(".agents").join("skills"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(0);
        } else {
            (
                self.external_agent_home.join("commands"),
                self.home_target_skills_dir(),
            )
        };

        import_commands(&source_commands, &target_skills)
    }

    fn import_skills(&self, cwd: Option<&Path>) -> io::Result<usize> {
        let (source_skills, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(EXTERNAL_AGENT_DIR).join("skills"),
                repo_root.join(".agents").join("skills"),
            )
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(0);
        } else {
            (
                self.external_agent_home.join("skills"),
                self.home_target_skills_dir(),
            )
        };
        if !source_skills.is_dir() {
            return Ok(0);
        }

        fs::create_dir_all(&target_skills)?;
        let mut copied_count = 0usize;

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

            copy_dir_recursive(&entry.path(), &target)?;
            copied_count += 1;
        }

        Ok(copied_count)
    }

    fn import_agents_md(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_agents_md, target_agents_md) = if let Some(repo_root) = find_repo_root(cwd)? {
            let Some(source_agents_md) = find_repo_agents_md_source(&repo_root)? else {
                return Ok(());
            };
            (source_agents_md, repo_root.join("AGENTS.md"))
        } else if cwd.is_some_and(|cwd| !cwd.as_os_str().is_empty()) {
            return Ok(());
        } else {
            (
                self.external_agent_home.join(EXTERNAL_AGENT_CONFIG_MD),
                self.codex_home.join("AGENTS.md"),
            )
        };
        if !is_non_empty_text_file(&source_agents_md)?
            || !is_missing_or_empty_text_file(&target_agents_md)?
        {
            return Ok(());
        }

        let Some(target_parent) = target_agents_md.parent() else {
            return Err(invalid_data_error("AGENTS.md target path has no parent"));
        };
        fs::create_dir_all(target_parent)?;

        rewrite_and_copy_text_file(&source_agents_md, &target_agents_md)
    }
}

fn default_external_agent_home() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(EXTERNAL_AGENT_DIR);
    }

    PathBuf::from(EXTERNAL_AGENT_DIR)
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

fn effective_external_settings(project_settings: &Path) -> io::Result<Option<JsonValue>> {
    let mut effective = read_external_settings(project_settings)?;
    let Some(settings_dir) = project_settings.parent() else {
        return Ok(effective);
    };
    let local_settings = settings_dir.join("settings.local.json");
    let local_settings = match read_external_settings(&local_settings) {
        Ok(Some(local_settings)) => local_settings,
        Ok(None) => return Ok(effective),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => return Ok(effective),
        Err(err) => return Err(err),
    };
    if let Some(effective) = effective.as_mut() {
        merge_json_settings(effective, &local_settings);
    } else {
        effective = Some(local_settings);
    }
    Ok(effective)
}

fn merge_json_settings(existing: &mut JsonValue, incoming: &JsonValue) {
    match (existing, incoming) {
        (JsonValue::Object(existing), JsonValue::Object(incoming)) => {
            for (key, incoming_value) in incoming {
                match existing.get_mut(key) {
                    Some(existing_value) => merge_json_settings(existing_value, incoming_value),
                    None => {
                        existing.insert(key.clone(), incoming_value.clone());
                    }
                }
            }
        }
        (existing, incoming) => {
            *existing = incoming.clone();
        }
    }
}
fn extract_plugin_migration_details(
    settings: &JsonValue,
    source_root: &Path,
    configured_plugin_ids: &HashSet<String>,
    configured_marketplace_plugins: &BTreeMap<String, HashSet<String>>,
) -> Option<MigrationDetails> {
    let loadable_marketplaces = collect_marketplace_import_sources(settings, source_root)
        .into_iter()
        .filter_map(|(marketplace_name, source)| {
            is_local_marketplace_source(&source.source, source.ref_name)
                .ok()
                .map(|_| marketplace_name)
        })
        .collect::<HashSet<_>>();
    let mut plugins = BTreeMap::new();
    for plugin_id in collect_enabled_plugins(settings)
        .into_iter()
        .filter(|plugin_id| !configured_plugin_ids.contains(plugin_id))
    {
        let Ok(plugin_id) = PluginId::parse(&plugin_id) else {
            continue;
        };
        if let Some(installable_plugins) =
            configured_marketplace_plugins.get(&plugin_id.marketplace_name)
        {
            if !installable_plugins.contains(&plugin_id.plugin_name) {
                continue;
            }
        } else if !loadable_marketplaces.contains(&plugin_id.marketplace_name) {
            continue;
        }
        let plugin_group = plugins
            .entry(plugin_id.marketplace_name.clone())
            .or_insert_with(|| PluginsMigration {
                marketplace_name: plugin_id.marketplace_name.clone(),
                plugin_names: Vec::new(),
            });
        plugin_group.plugin_names.push(plugin_id.plugin_name);
    }

    let plugins = plugins
        .into_values()
        .filter_map(|mut plugin_group| {
            if plugin_group.plugin_names.is_empty() {
                return None;
            }
            plugin_group.plugin_names.sort();
            Some(plugin_group)
        })
        .collect::<Vec<_>>();
    if plugins.is_empty() {
        return None;
    }

    Some(MigrationDetails {
        plugins,
        ..Default::default()
    })
}

fn collect_enabled_plugins(settings: &JsonValue) -> Vec<String> {
    let Some(enabled_plugins) = settings
        .as_object()
        .and_then(|settings| settings.get("enabledPlugins"))
        .and_then(JsonValue::as_object)
    else {
        return Vec::new();
    };

    enabled_plugins
        .iter()
        .filter_map(|(plugin_key, enabled)| {
            if !enabled.as_bool().unwrap_or(false) {
                return None;
            }
            PluginId::parse(plugin_key)
                .ok()
                .map(|plugin_id| plugin_id.as_key())
        })
        .collect()
}

fn has_enabled_plugin_for_marketplace(settings: &JsonValue, marketplace_name: &str) -> bool {
    collect_enabled_plugins(settings)
        .into_iter()
        .any(|plugin_id| {
            PluginId::parse(&plugin_id)
                .map(|plugin_id| plugin_id.marketplace_name == marketplace_name)
                .unwrap_or(false)
        })
}

fn configured_marketplace_plugins(
    config: &Config,
    plugins_manager: &PluginsManager,
) -> io::Result<BTreeMap<String, HashSet<String>>> {
    let plugins_input = config.plugins_config_input();
    let marketplaces = plugins_manager
        .list_marketplaces_for_config(&plugins_input, &[])
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

fn collect_marketplace_import_sources(
    settings: &JsonValue,
    source_root: &Path,
) -> BTreeMap<String, MarketplaceImportSource> {
    let mut import_sources: BTreeMap<String, MarketplaceImportSource> = settings
        .as_object()
        .and_then(|settings| settings.get("extraKnownMarketplaces"))
        .and_then(JsonValue::as_object)
        .map(|extra_known_marketplaces| {
            extra_known_marketplaces
                .iter()
                .filter_map(|(name, value)| {
                    let source_fields = if let Some(source) = value.get("source")
                        && source.is_object()
                    {
                        source.as_object()?
                    } else {
                        value.as_object()?
                    };
                    let source = source_fields
                        .get("repo")
                        .or_else(|| source_fields.get("url"))
                        .or_else(|| source_fields.get("path"))
                        .or_else(|| value.get("source"))?
                        .as_str()?
                        .trim()
                        .to_string();
                    if source.is_empty() {
                        return None;
                    }
                    let source = resolve_external_marketplace_source(&source, source_root);

                    let ref_name = source_fields
                        .get("ref")
                        .or_else(|| value.get("ref"))
                        .and_then(JsonValue::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned);

                    Some((name.clone(), MarketplaceImportSource { source, ref_name }))
                })
                .collect()
        })
        .unwrap_or_default();

    if has_enabled_plugin_for_marketplace(settings, EXTERNAL_OFFICIAL_MARKETPLACE_NAME)
        && !import_sources.contains_key(EXTERNAL_OFFICIAL_MARKETPLACE_NAME)
    {
        import_sources.insert(
            EXTERNAL_OFFICIAL_MARKETPLACE_NAME.to_string(),
            MarketplaceImportSource {
                source: EXTERNAL_OFFICIAL_MARKETPLACE_SOURCE.to_string(),
                ref_name: None,
            },
        );
    }

    import_sources
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketplaceImportSource {
    source: String,
    ref_name: Option<String>,
}

fn resolve_external_marketplace_source(source: &str, source_root: &Path) -> String {
    if !looks_like_relative_local_path(source) {
        return source.to_string();
    }

    source_root.join(source).display().to_string()
}

fn looks_like_relative_local_path(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../") || source == "." || source == ".."
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

fn count_missing_subdirectories(source: &Path, target: &Path) -> io::Result<usize> {
    let source_names = collect_subdirectory_names(source)?;
    let target_names = collect_subdirectory_names(target)?;
    Ok(source_names
        .iter()
        .filter(|name| !target_names.contains(*name))
        .count())
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

fn find_repo_agents_md_source(repo_root: &Path) -> io::Result<Option<PathBuf>> {
    for candidate in [
        repo_root.join(EXTERNAL_AGENT_CONFIG_MD),
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join(EXTERNAL_AGENT_CONFIG_MD),
    ] {
        if is_non_empty_text_file(&candidate)? {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
            continue;
        }

        if file_type.is_file() {
            if is_skill_md(&source_path) {
                rewrite_and_copy_text_file(&source_path, &target_path)?;
            } else {
                fs::copy(source_path, target_path)?;
            }
        }
    }

    Ok(())
}

fn is_skill_md(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
}

fn rewrite_and_copy_text_file(source: &Path, target: &Path) -> io::Result<()> {
    let source_contents = fs::read_to_string(source)?;
    let rewritten = rewrite_external_agent_terms(&source_contents);
    fs::write(target, rewritten)
}

fn rewrite_external_agent_terms(content: &str) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(
        content,
        &EXTERNAL_AGENT_CONFIG_MD.to_ascii_lowercase(),
        "AGENTS.md",
    );
    for from in [
        "claude code",
        "claude-code",
        "claude_code",
        "claudecode",
        "claude",
    ] {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
    }
    rewritten
}

fn replace_case_insensitive_with_boundaries(
    input: &str,
    needle: &str,
    replacement: &str,
) -> String {
    let needle_lower = needle.to_ascii_lowercase();
    if needle_lower.is_empty() {
        return input.to_string();
    }

    let haystack_lower = input.to_ascii_lowercase();
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = haystack_lower[search_start..].find(&needle_lower) {
        let start = search_start + relative_pos;
        let end = start + needle_lower.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }

        search_start = start + 1;
    }

    if last_emitted == 0 {
        return input.to_string();
    }

    output.push_str(&input[last_emitted..]);
    output
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn build_config_from_external(settings: &JsonValue) -> io::Result<TomlValue> {
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

    if let Some(sandbox_enabled) = settings_obj
        .get("sandbox")
        .and_then(JsonValue::as_object)
        .and_then(|sandbox| sandbox.get("enabled"))
        .and_then(JsonValue::as_bool)
        && sandbox_enabled
    {
        root.insert(
            "sandbox_mode".to_string(),
            TomlValue::String("workspace-write".to_string()),
        );
    }

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

fn migration_metric_tags(
    item_type: ExternalAgentConfigMigrationItemType,
    skills_count: Option<usize>,
) -> Vec<(&'static str, String)> {
    let migration_type = match item_type {
        ExternalAgentConfigMigrationItemType::Config => "config",
        ExternalAgentConfigMigrationItemType::Skills => "skills",
        ExternalAgentConfigMigrationItemType::AgentsMd => "agents_md",
        ExternalAgentConfigMigrationItemType::Plugins => "plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "mcp_server_config",
        ExternalAgentConfigMigrationItemType::Subagents => "subagents",
        ExternalAgentConfigMigrationItemType::Hooks => "hooks",
        ExternalAgentConfigMigrationItemType::Commands => "commands",
        ExternalAgentConfigMigrationItemType::Sessions => "sessions",
    };
    let mut tags = vec![("migration_type", migration_type.to_string())];
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
#[path = "external_agent_config_tests.rs"]
mod tests;
