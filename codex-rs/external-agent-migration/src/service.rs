use crate::config_values::is_empty_toml_table;
use crate::config_values::merge_missing_mcp_servers;
use crate::config_values::merge_missing_toml_values;
use crate::config_values::migrated_mcp_server_names;
use crate::config_values::write_toml_file;
use crate::memory_import;
use crate::migration_source::ExternalAgentSource;
use crate::migration_source::InstructionSourceGroup;
pub use crate::model::ExternalAgentConfigDetectOptions;
pub use crate::model::ExternalAgentConfigImportItemResult;
pub use crate::model::ExternalAgentConfigImportOutcome;
pub use crate::model::ExternalAgentConfigImportRawError;
pub use crate::model::ExternalAgentConfigImportSuccess;
pub use crate::model::ExternalAgentConfigMigrationItem;
pub use crate::model::ExternalAgentConfigMigrationItemType;
pub use crate::model::MigrationDetails;
pub use crate::model::NamedMigration;
pub use crate::model::PendingPluginImport;
pub use crate::model::PluginImportOutcome;
pub use crate::model::PluginsMigration;
use crate::reporting::emit_migration_metric;
#[cfg(test)]
use crate::reporting::migration_metric_tags;
pub use crate::reporting::record_import_error;
use crate::scope::MigrationScope;
use crate::sessions::SessionMetadataMode;
#[cfg(test)]
use crate::source_cla::KNOWN_MARKETPLACES_PATH as EXTERNAL_AGENT_KNOWN_MARKETPLACES_PATH;
#[cfg(test)]
use crate::source_cla::OFFICIAL_MARKETPLACE_NAME as EXTERNAL_OFFICIAL_MARKETPLACE_NAME;
use crate::utils::copy_dir_recursive;
use crate::utils::display_source_paths;
use crate::utils::invalid_data_error;
use crate::utils::is_missing_or_empty_text_file;
pub(super) use crate::utils::read_json_file as read_external_settings;
use crate::utils::rewrite_external_agent_terms;
use codex_analytics::AnalyticsEventsClient;
use codex_core::config::Config;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_protocol::protocol::Product;
use codex_rollout::StateDbHandle;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

#[cfg(test)]
const EXTERNAL_AGENT_DIR: &str = crate::ClaSource::CONFIG_DIR;
#[cfg(test)]
const EXTERNAL_AGENT_CONFIG_MD: &str = crate::ClaSource::CONFIG_MD;

const EXTERNAL_AGENT_CONFIG_IMPORT_METRIC: &str = "codex.external_agent_config.import";

#[derive(Clone)]
pub struct ExternalAgentConfigService {
    pub(super) codex_home: PathBuf,
    pub(super) connector_metadata_roots: Vec<PathBuf>,
    pub(crate) external_agent_home: PathBuf,
    pub(crate) analytics_events_client: Option<AnalyticsEventsClient>,
    pub(crate) source: ExternalAgentSource,
    state_db: Option<StateDbHandle>,
}

impl ExternalAgentConfigService {
    pub fn new(
        codex_home: PathBuf,
        analytics_events_client: AnalyticsEventsClient,
        state_db: Option<StateDbHandle>,
    ) -> Self {
        let source = ExternalAgentSource::default();
        let external_agent_home = default_external_agent_home(source);
        let connector_metadata_roots = source.connector_metadata_roots(&external_agent_home);
        Self {
            codex_home,
            connector_metadata_roots,
            external_agent_home,
            analytics_events_client: Some(analytics_events_client),
            source,
            state_db,
        }
    }

    pub fn with_migration_source(&self, migration_source: Option<&str>) -> Self {
        let source = ExternalAgentSource::from_migration_source(migration_source);
        let external_agent_home = default_external_agent_home(source);
        let connector_metadata_roots = source.connector_metadata_roots(&external_agent_home);
        Self {
            codex_home: self.codex_home.clone(),
            connector_metadata_roots,
            external_agent_home,
            analytics_events_client: self.analytics_events_client.clone(),
            source,
            state_db: self.state_db.clone(),
        }
    }

    pub fn session_metadata_mode(&self) -> SessionMetadataMode {
        self.source.session_metadata_mode()
    }

    pub fn connector_metadata_roots(&self) -> &[PathBuf] {
        &self.connector_metadata_roots
    }

    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    #[cfg(test)]
    fn new_for_test(codex_home: PathBuf, external_agent_home: PathBuf) -> Self {
        let source = ExternalAgentSource::default();
        let connector_metadata_roots = source.connector_metadata_roots(&external_agent_home);
        Self {
            codex_home,
            connector_metadata_roots,
            external_agent_home,
            analytics_events_client: None,
            source,
            state_db: None,
        }
    }

    pub fn external_agent_session_source_path(&self, path: &Path) -> io::Result<Option<PathBuf>> {
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

    pub async fn import(
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
                ExternalAgentConfigMigrationItemType::Plugins => {
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
                                    /*sub_error_type*/ None,
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
                                    /*sub_error_type*/ None,
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
                                        /*sub_error_type*/ None,
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
                ExternalAgentConfigMigrationItemType::Memory if self.source.supports_memory() => {
                    async {
                        let selected_memory = migration_item
                            .details
                            .as_ref()
                            .map(|details| details.memory.as_slice())
                            .unwrap_or_default();
                        let memory_outcome = memory_import::import(
                            &self.codex_home,
                            &self.external_agent_home,
                            self.state_db.as_ref(),
                            selected_memory,
                        )
                        .await?;
                        emit_migration_metric(
                            EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                            ExternalAgentConfigMigrationItemType::Memory,
                            /*skills_count*/ None,
                        );
                        let target_path = memory_import::resources_root(&self.codex_home);
                        for project_key in memory_outcome.synchronized_projects {
                            item_result.record_success(
                                Some(project_key),
                                Some(target_path.display().to_string()),
                            );
                        }
                        for failure in memory_outcome.failures {
                            record_import_error(
                                &mut item_result,
                                "memory_import",
                                /*sub_error_type*/ None,
                                failure.message,
                                Some(failure.project_key),
                            );
                        }
                        Ok(())
                    }
                    .await
                }
                ExternalAgentConfigMigrationItemType::Memory => Err(invalid_data_error(
                    "memory import is not supported for the selected migration source".to_string(),
                )),
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

    pub(crate) fn home_target_skills_dir(&self) -> PathBuf {
        self.codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"))
    }

    pub(crate) fn source_config_dir(&self, scope: &MigrationScope) -> PathBuf {
        scope.repo_root().map_or_else(
            || self.external_agent_home.clone(),
            |repo_root| repo_root.join(self.source.config_dir()),
        )
    }

    pub(crate) fn source_settings(&self, scope: &MigrationScope) -> PathBuf {
        self.source_config_dir(scope)
            .join(self.source.settings_file_name(scope))
    }

    pub(crate) fn effective_source_settings(
        &self,
        scope: &MigrationScope,
    ) -> io::Result<Option<JsonValue>> {
        let source_settings = self.source_settings(scope);
        self.source
            .effective_settings(self.source_config_dir(scope).as_path(), &source_settings)
    }

    pub(crate) fn build_mcp_config(
        &self,
        scope: &MigrationScope,
        settings: Option<JsonValue>,
    ) -> io::Result<TomlValue> {
        let settings = self.mcp_settings(scope, settings)?;
        self.source.build_mcp_config(
            self.source_root(scope).as_path(),
            self.source_config_dir(scope).as_path(),
            self.external_agent_home.as_path(),
            settings.as_ref(),
        )
    }

    pub(crate) fn repo_agents_md_source_groups(
        &self,
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        self.source.repo_instruction_source_groups(repo_root)
    }

    pub(crate) fn home_agents_md_sources(&self) -> io::Result<Vec<PathBuf>> {
        self.source
            .home_instruction_sources(self.external_agent_home.as_path())
    }

    fn mcp_settings(
        &self,
        scope: &MigrationScope,
        source_settings: Option<JsonValue>,
    ) -> io::Result<Option<JsonValue>> {
        if !scope.is_home() && source_settings.is_none() {
            let home_scope = MigrationScope::home();
            let home_settings = self.source_settings(&home_scope);
            match self.effective_source_settings(&home_scope) {
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

    pub(crate) fn source_root(&self, scope: &MigrationScope) -> PathBuf {
        scope.repo_root().map_or_else(
            || {
                self.external_agent_home
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            },
            Path::to_path_buf,
        )
    }

    fn import_config(&self, cwd: Option<&Path>) -> io::Result<Option<(String, String)>> {
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(None);
        };
        let source_settings = self.source_settings(&scope);
        let target_config = match &scope {
            MigrationScope::Home => self.codex_home.join("config.toml"),
            MigrationScope::Repository { root } => root.join(".codex").join("config.toml"),
        };
        let Some(settings) = self.effective_source_settings(&scope)? else {
            return Ok(None);
        };
        let migrated = self.source.build_config(&settings)?;
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
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(Vec::new());
        };
        let target_config = match &scope {
            MigrationScope::Home => self.codex_home.join("config.toml"),
            MigrationScope::Repository { root } => root.join(".codex").join("config.toml"),
        };
        let settings = self.effective_source_settings(&scope)?;
        let migrated = self.build_mcp_config(&scope, settings)?;
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
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(Vec::new());
        };
        let (source_agents, target_agents) = match scope {
            MigrationScope::Home => (
                self.external_agent_home.join("agents"),
                self.codex_home.join("agents"),
            ),
            MigrationScope::Repository { root } => (
                root.join(self.source.config_dir()).join("agents"),
                root.join(".codex").join("agents"),
            ),
        };

        self.source.import_subagents(&source_agents, &target_agents)
    }

    fn import_hooks(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(Vec::new());
        };
        let target_hooks = match &scope {
            MigrationScope::Home => self.codex_home.join("hooks.json"),
            MigrationScope::Repository { root } => root.join(".codex").join("hooks.json"),
        };
        let source_external_agent_dir = self.source_config_dir(&scope);

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
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(Vec::new());
        };
        let (source_commands, target_skills) = match scope {
            MigrationScope::Home => (
                self.external_agent_home.join("commands"),
                self.home_target_skills_dir(),
            ),
            MigrationScope::Repository { root } => (
                root.join(self.source.config_dir()).join("commands"),
                root.join(".agents").join("skills"),
            ),
        };

        self.source
            .import_commands(&source_commands, &target_skills)
    }

    fn import_skills(&self, cwd: Option<&Path>) -> io::Result<Vec<String>> {
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(Vec::new());
        };
        let (source_skills, target_skills) = match scope {
            MigrationScope::Home => (
                self.external_agent_home.join("skills"),
                self.home_target_skills_dir(),
            ),
            MigrationScope::Repository { root } => (
                root.join(self.source.config_dir()).join("skills"),
                root.join(".agents").join("skills"),
            ),
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
        let Some(scope) = MigrationScope::from_cwd(cwd)? else {
            return Ok(None);
        };
        let (source_agents_md, target_agents_md) = match scope {
            MigrationScope::Repository { root } => {
                let Some(group) = self
                    .repo_agents_md_source_groups(&root)?
                    .into_iter()
                    .find(|group| group.scope == root)
                else {
                    return Ok(None);
                };
                let target_agents_md = group.scope.join("AGENTS.md");
                (group.sources, target_agents_md)
            }
            MigrationScope::Home => {
                let source_agents_md = self.home_agents_md_sources()?;
                if source_agents_md.is_empty() {
                    return Ok(None);
                }
                (source_agents_md, self.codex_home.join("AGENTS.md"))
            }
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

pub(crate) fn configured_marketplace_plugins(
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

pub(crate) fn missing_subdirectory_names(source: &Path, target: &Path) -> io::Result<Vec<String>> {
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

pub(crate) fn named_migrations(names: Vec<String>) -> Vec<NamedMigration> {
    names
        .into_iter()
        .map(|name| NamedMigration { name })
        .collect()
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
