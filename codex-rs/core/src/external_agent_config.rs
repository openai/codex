use crate::plugins::MarketplaceAddRequest;
use crate::plugins::PluginId;
use crate::plugins::PluginInstallRequest;
use crate::plugins::PluginsManager;
use crate::plugins::add_marketplace;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

const EXTERNAL_AGENT_CONFIG_DETECT_METRIC: &str = "codex.external_agent_config.detect";
const EXTERNAL_AGENT_CONFIG_IMPORT_METRIC: &str = "codex.external_agent_config.import";
// Installed marketplace roots always expose their manifest at this relative path.
const INSTALLED_MARKETPLACE_MANIFEST_RELATIVE_PATH: &str = ".agents/plugins/marketplace.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigDetectOptions {
    pub include_home: bool,
    pub cwds: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalAgentConfigMigrationItemType {
    Config,
    Skills,
    AgentsMd,
    Plugins,
    McpServerConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginsMigration {
    pub marketplace_name: String,
    pub plugin_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDetails {
    pub plugins: Vec<PluginsMigration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PluginImportOutcome {
    succeeded_marketplaces: Vec<String>,
    succeeded_plugin_ids: Vec<String>,
    failed_marketplaces: Vec<String>,
    failed_plugin_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAgentConfigMigrationItem {
    pub item_type: ExternalAgentConfigMigrationItemType,
    pub description: String,
    pub cwd: Option<PathBuf>,
    pub details: Option<MigrationDetails>,
}

#[derive(Clone)]
pub struct ExternalAgentConfigService {
    codex_home: PathBuf,
    external_agent_home: PathBuf,
}

impl ExternalAgentConfigService {
    pub fn new(codex_home: PathBuf) -> Self {
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

    pub fn detect(
        &self,
        params: ExternalAgentConfigDetectOptions,
    ) -> io::Result<Vec<ExternalAgentConfigMigrationItem>> {
        let mut items = Vec::new();
        if params.include_home {
            self.detect_migrations(/*repo_root*/ None, &mut items)?;
        }

        for cwd in params.cwds.as_deref().unwrap_or(&[]) {
            let Some(repo_root) = find_repo_root(Some(cwd))? else {
                continue;
            };
            self.detect_migrations(Some(&repo_root), &mut items)?;
        }

        Ok(items)
    }

    pub async fn import(
        &self,
        migration_items: Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
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
                    let service = self.clone();
                    let cwd = migration_item.cwd;
                    let details = migration_item.details;
                    tokio::spawn(async move {
                        match service.import_plugins(cwd.as_deref(), details).await {
                            Ok(outcome) => {
                                tracing::info!(
                                    succeeded_marketplaces = outcome.succeeded_marketplaces.len(),
                                    succeeded_plugin_ids = outcome.succeeded_plugin_ids.len(),
                                    failed_marketplaces = outcome.failed_marketplaces.len(),
                                    failed_plugin_ids = outcome.failed_plugin_ids.len(),
                                    "external agent config plugin import completed"
                                );
                            }
                            Err(err) => {
                                tracing::warn!(
                                    error = %err,
                                    "external agent config plugin import failed"
                                );
                            }
                        }
                    });
                    emit_migration_metric(
                        EXTERNAL_AGENT_CONFIG_IMPORT_METRIC,
                        ExternalAgentConfigMigrationItemType::Plugins,
                        /*skills_count*/ None,
                    );
                }
                ExternalAgentConfigMigrationItemType::McpServerConfig => {}
            }
        }

        Ok(())
    }

    fn detect_migrations(
        &self,
        repo_root: Option<&Path>,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) -> io::Result<()> {
        let cwd = repo_root.map(Path::to_path_buf);
        let source_settings = repo_root.map_or_else(
            || self.external_agent_home.join("settings.json"),
            |repo_root| repo_root.join(".claude").join("settings.json"),
        );
        let settings = read_external_settings(&source_settings)?;
        let target_config = repo_root.map_or_else(
            || self.codex_home.join("config.toml"),
            |repo_root| repo_root.join(".codex").join("config.toml"),
        );
        if let Some(settings) = settings.as_ref() {
            let migrated = build_config_from_external(&settings)?;
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

        self.detect_plugin_migration(
            source_settings.as_path(),
            cwd.clone(),
            settings.as_ref(),
            items,
        );

        let source_skills = repo_root.map_or_else(
            || self.external_agent_home.join("skills"),
            |repo_root| repo_root.join(".claude").join("skills"),
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
                    "Copy skill folders from {} to {}",
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

        let source_agents_md = if let Some(repo_root) = repo_root {
            find_repo_agents_md_source(repo_root)?
        } else {
            let path = self.external_agent_home.join("CLAUDE.md");
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
                    "Import {} to {}",
                    source_agents_md.display(),
                    target_agents_md.display()
                ),
                cwd,
                details: None,
            });
            emit_migration_metric(
                EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
                ExternalAgentConfigMigrationItemType::AgentsMd,
                /*skills_count*/ None,
            );
        }

        Ok(())
    }

    fn home_target_skills_dir(&self) -> PathBuf {
        self.codex_home
            .parent()
            .map(|parent| parent.join(".agents").join("skills"))
            .unwrap_or_else(|| PathBuf::from(".agents").join("skills"))
    }

    fn detect_plugin_migration(
        &self,
        source_settings: &Path,
        cwd: Option<PathBuf>,
        settings: Option<&JsonValue>,
        items: &mut Vec<ExternalAgentConfigMigrationItem>,
    ) {
        let Some(plugin_details) = settings.and_then(extract_plugin_migration_details) else {
            return;
        };

        items.push(ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!("Import enabled plugins from {}", source_settings.display()),
            cwd,
            details: Some(plugin_details),
        });
        emit_migration_metric(
            EXTERNAL_AGENT_CONFIG_DETECT_METRIC,
            ExternalAgentConfigMigrationItemType::Plugins,
            /*skills_count*/ None,
        );
    }

    async fn import_plugins(
        &self,
        cwd: Option<&Path>,
        details: Option<MigrationDetails>,
    ) -> io::Result<PluginImportOutcome> {
        let Some(MigrationDetails { plugins }) = details else {
            return Err(invalid_data_error(
                "plugins migration item is missing details".to_string(),
            ));
        };
        let mut outcome = PluginImportOutcome::default();
        let plugins_manager = PluginsManager::new(self.codex_home.clone());
        for plugin_group in plugins {
            let marketplace_name = plugin_group.marketplace_name.clone();
            let plugin_ids = plugin_group.plugin_ids;
            let import_source =
                read_marketplace_import_source(cwd, &self.external_agent_home, &marketplace_name)?;
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
                    match AbsolutePathBuf::try_from(
                        add_marketplace_outcome
                            .installed_root
                            .join(INSTALLED_MARKETPLACE_MANIFEST_RELATIVE_PATH),
                    ) {
                        Ok(path) => {
                            outcome
                                .succeeded_marketplaces
                                .push(marketplace_name.clone());
                            path
                        }
                        Err(_) => {
                            outcome.failed_marketplaces.push(marketplace_name);
                            outcome.failed_plugin_ids.extend(plugin_ids);
                            continue;
                        }
                    }
                }
                Err(_) => {
                    outcome.failed_marketplaces.push(marketplace_name);
                    outcome.failed_plugin_ids.extend(plugin_ids);
                    continue;
                }
            };
            for raw_plugin_id in plugin_ids {
                let Ok(plugin_id) = PluginId::parse(&raw_plugin_id) else {
                    outcome.failed_plugin_ids.push(raw_plugin_id);
                    continue;
                };
                if plugin_id.marketplace_name != marketplace_name {
                    outcome.failed_plugin_ids.push(plugin_id.as_key());
                    continue;
                }
                match plugins_manager
                    .install_plugin(PluginInstallRequest {
                        plugin_name: plugin_id.plugin_name.clone(),
                        marketplace_path: marketplace_path.clone(),
                    })
                    .await
                {
                    Ok(_) => outcome.succeeded_plugin_ids.push(plugin_id.as_key()),
                    Err(_) => outcome.failed_plugin_ids.push(plugin_id.as_key()),
                }
            }
        }

        Ok(outcome)
    }

    fn import_config(&self, cwd: Option<&Path>) -> io::Result<()> {
        let (source_settings, target_config) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(".claude").join("settings.json"),
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
        if !source_settings.is_file() {
            return Ok(());
        }

        let raw_settings = fs::read_to_string(&source_settings)?;
        let settings: JsonValue = serde_json::from_str(&raw_settings)
            .map_err(|err| invalid_data_error(err.to_string()))?;
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

    fn import_skills(&self, cwd: Option<&Path>) -> io::Result<usize> {
        let (source_skills, target_skills) = if let Some(repo_root) = find_repo_root(cwd)? {
            (
                repo_root.join(".claude").join("skills"),
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
                self.external_agent_home.join("CLAUDE.md"),
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
        return PathBuf::from(home).join(".claude");
    }

    PathBuf::from(".claude")
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

fn extract_plugin_migration_details(settings: &JsonValue) -> Option<MigrationDetails> {
    let mut plugins = BTreeMap::new();
    for plugin_id in collect_enabled_plugin_ids(settings) {
        let Ok(plugin_id) = PluginId::parse(&plugin_id) else {
            continue;
        };
        let plugin_group = plugins
            .entry(plugin_id.marketplace_name.clone())
            .or_insert_with(|| PluginsMigration {
                marketplace_name: plugin_id.marketplace_name.clone(),
                plugin_ids: Vec::new(),
            });
        plugin_group.plugin_ids.push(plugin_id.as_key());
    }

    let plugins = plugins
        .into_values()
        .filter_map(|mut plugin_group| {
            if plugin_group.plugin_ids.is_empty() {
                return None;
            }
            plugin_group.plugin_ids.sort();
            Some(plugin_group)
        })
        .collect::<Vec<_>>();
    if plugins.is_empty() {
        return None;
    }

    Some(MigrationDetails { plugins })
}

fn collect_enabled_plugin_ids(settings: &JsonValue) -> Vec<String> {
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

fn collect_marketplace_import_sources(
    settings: &JsonValue,
) -> BTreeMap<String, MarketplaceImportSource> {
    let Some(extra_known_marketplaces) = settings
        .as_object()
        .and_then(|settings| settings.get("extraKnownMarketplaces"))
        .and_then(JsonValue::as_object)
    else {
        return BTreeMap::new();
    };

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketplaceImportSource {
    source: String,
    ref_name: Option<String>,
}

fn read_marketplace_import_source(
    cwd: Option<&Path>,
    external_agent_home: &Path,
    marketplace_name: &str,
) -> io::Result<Option<MarketplaceImportSource>> {
    let source_settings = cwd.map_or_else(
        || external_agent_home.join("settings.json"),
        |cwd| cwd.join(".claude").join("settings.json"),
    );
    let Some(settings) = read_external_settings(&source_settings)? else {
        return Ok(None);
    };

    Ok(collect_marketplace_import_sources(&settings).remove(marketplace_name))
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
        repo_root.join("CLAUDE.md"),
        repo_root.join(".claude").join("CLAUDE.md"),
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
    let mut rewritten = replace_case_insensitive_with_boundaries(content, "claude.md", "AGENTS.md");
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
    };
    let mut tags = vec![("migration_type", migration_type.to_string())];
    if item_type == ExternalAgentConfigMigrationItemType::Skills {
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
