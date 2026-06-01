use crate::path_utils::write_atomically;
use anyhow::Context;
use codex_config::CONFIG_TOML_FILE;
use codex_config::types::McpServerConfig;
use codex_config::types::SessionPickerViewMode;
use codex_config::types::ToolSuggestDisabledTool;
use codex_config::with_config_write_lock;
use codex_features::FEATURES;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::openai_models::ReasoningEffort;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use tokio::task;
use toml_edit::ArrayOfTables;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

const NOTICE_TABLE_KEY: &str = "notice";

mod document_helpers;

/// Discrete config mutations supported by the persistence engine.
#[derive(Clone, Debug)]
pub enum ConfigEdit {
    /// Update the active (or default) model selection and optional reasoning effort.
    SetModel {
        model: Option<String>,
        effort: Option<ReasoningEffort>,
    },
    /// Update the service tier preference for future turns.
    SetServiceTier { service_tier: Option<String> },
    /// Update the active (or default) model personality.
    SetModelPersonality { personality: Option<Personality> },
    /// Toggle the acknowledgement flag under `[notice]`.
    SetNoticeHideFullAccessWarning(bool),
    /// Toggle the Windows world-writable directories warning acknowledgement flag.
    SetNoticeHideWorldWritableWarning(bool),
    /// Toggle the rate limit model nudge acknowledgement flag.
    SetNoticeHideRateLimitModelNudge(bool),
    /// Toggle the model migration prompt acknowledgement flag.
    SetNoticeHideModelMigrationPrompt(String, bool),
    /// Toggle the home external config migration prompt acknowledgement flag.
    SetNoticeHideExternalConfigMigrationPromptHome(bool),
    /// Record when the home external config migration prompt was last shown.
    SetNoticeExternalConfigMigrationPromptHomeLastPromptedAt(i64),
    /// Toggle the project external config migration prompt acknowledgement flag.
    SetNoticeHideExternalConfigMigrationPromptProject(String, bool),
    /// Record when the project external config migration prompt was last shown.
    SetNoticeExternalConfigMigrationPromptProjectLastPromptedAt(String, i64),
    /// Record that a migration prompt was shown for an old->new model mapping.
    RecordModelMigrationSeen { from: String, to: String },
    /// Replace the entire `[mcp_servers]` table.
    ReplaceMcpServers(BTreeMap<String, McpServerConfig>),
    /// Add a disabled tool suggestion under `[tool_suggest].disabled_tools`.
    AddToolSuggestDisabledTool(ToolSuggestDisabledTool),
    /// Set or clear a skill config entry under `[[skills.config]]` by path.
    SetSkillConfig { path: PathBuf, enabled: bool },
    /// Set or clear a skill config entry under `[[skills.config]]` by name.
    SetSkillConfigByName { name: String, enabled: bool },
    /// Set trust_level under `[projects."<path>"]`,
    /// migrating inline tables to explicit tables.
    SetProjectTrustLevel { path: PathBuf, level: TrustLevel },
    /// Set the value stored at the exact dotted path.
    SetPath {
        segments: Vec<String>,
        value: TomlItem,
    },
    /// Remove the value stored at the exact dotted path.
    ClearPath { segments: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SkillConfigSelector {
    Name(String),
    Path(PathBuf),
}

/// Produces a config edit that sets `[tui].theme = "<name>"`.
pub fn syntax_theme_edit(name: &str) -> ConfigEdit {
    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "theme".to_string()],
        value: value(name.to_string()),
    }
}

/// Produces a config edit that sets [tui].pet = "<name>".
pub fn tui_pet_edit(name: &str) -> ConfigEdit {
    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "pet".to_string()],
        value: value(name.to_string()),
    }
}

/// Produces a config edit that sets `[tui].session_picker_view = "<mode>"`.
pub fn session_picker_view_edit(mode: SessionPickerViewMode) -> ConfigEdit {
    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "session_picker_view".to_string()],
        value: value(mode.to_string()),
    }
}

/// Produces a config edit that sets `[tui].status_line` to an explicit ordered list.
///
/// The array is written even when it is empty so "hide the status line" stays
/// distinct from "unset, so use defaults".
pub fn status_line_items_edit(items: &[String]) -> ConfigEdit {
    let array = items.iter().cloned().collect::<toml_edit::Array>();

    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "status_line".to_string()],
        value: TomlItem::Value(array.into()),
    }
}

/// Produces a config edit that sets `[tui].status_line_use_colors`.
pub fn status_line_use_colors_edit(enabled: bool) -> ConfigEdit {
    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "status_line_use_colors".to_string()],
        value: value(enabled),
    }
}

/// Produces a config edit that sets `[tui].terminal_title` to an explicit ordered list.
///
/// The array is written even when it is empty so "disabled title updates" stays
/// distinct from "unset, so use defaults".
pub fn terminal_title_items_edit(items: &[String]) -> ConfigEdit {
    let array = items.iter().cloned().collect::<toml_edit::Array>();

    ConfigEdit::SetPath {
        segments: vec!["tui".to_string(), "terminal_title".to_string()],
        value: TomlItem::Value(array.into()),
    }
}

fn keymap_binding_value(keys: &[String]) -> TomlItem {
    if let [key] = keys {
        value(key.to_string())
    } else {
        let array = keys.iter().cloned().collect::<toml_edit::Array>();
        TomlItem::Value(array.into())
    }
}

/// Produces a config edit that replaces one root-level TUI keymap binding list.
pub fn keymap_bindings_edit(context: &str, action: &str, keys: &[String]) -> ConfigEdit {
    ConfigEdit::SetPath {
        segments: vec![
            "tui".to_string(),
            "keymap".to_string(),
            context.to_string(),
            action.to_string(),
        ],
        value: keymap_binding_value(keys),
    }
}

/// Produces a config edit that replaces one root-level TUI keymap binding.
pub fn keymap_binding_edit(context: &str, action: &str, key: &str) -> ConfigEdit {
    keymap_bindings_edit(context, action, &[key.to_string()])
}

/// Produces a config edit that removes one root-level TUI keymap binding.
pub fn keymap_binding_clear_edit(context: &str, action: &str) -> ConfigEdit {
    ConfigEdit::ClearPath {
        segments: vec![
            "tui".to_string(),
            "keymap".to_string(),
            context.to_string(),
            action.to_string(),
        ],
    }
}

pub fn model_availability_nux_count_edits(shown_count: &HashMap<String, u32>) -> Vec<ConfigEdit> {
    let mut shown_count_entries: Vec<_> = shown_count.iter().collect();
    shown_count_entries.sort_unstable_by_key(|(left, _)| *left);

    let mut edits = vec![ConfigEdit::ClearPath {
        segments: vec!["tui".to_string(), "model_availability_nux".to_string()],
    }];
    for (model_slug, count) in shown_count_entries {
        edits.push(ConfigEdit::SetPath {
            segments: vec![
                "tui".to_string(),
                "model_availability_nux".to_string(),
                model_slug.clone(),
            ],
            value: value(i64::from(*count)),
        });
    }

    edits
}

struct ConfigDocument {
    doc: DocumentMut,
}

#[derive(Copy, Clone)]
enum TraversalMode {
    Create,
    Existing,
}

impl ConfigDocument {
    fn new(doc: DocumentMut) -> Self {
        Self { doc }
    }

    fn apply(&mut self, edit: &ConfigEdit) -> anyhow::Result<bool> {
        match edit {
            ConfigEdit::SetModel { model, effort } => Ok({
                let mut mutated = false;
                mutated |= self.write_optional_value(
                    &["model"],
                    model.as_ref().map(|model_value| value(model_value.clone())),
                );
                mutated |= self.write_optional_value(
                    &["model_reasoning_effort"],
                    effort.map(|effort| value(effort.to_string())),
                );
                mutated
            }),
            ConfigEdit::SetServiceTier { service_tier } => Ok(self.write_optional_value(
                &["service_tier"],
                service_tier.as_ref().map(|service_tier| {
                    // Keep the legacy config spelling stable. Runtime values use
                    // `priority`, but config.toml continues to store it as `fast`.
                    let config_value = match ServiceTier::from_request_value(service_tier) {
                        Some(ServiceTier::Fast) => "fast",
                        Some(ServiceTier::Flex) => "flex",
                        None => service_tier.as_str(),
                    };
                    value(config_value)
                }),
            )),
            ConfigEdit::SetModelPersonality { personality } => Ok(self.write_optional_value(
                &["personality"],
                personality.map(|personality| value(personality.to_string())),
            )),
            ConfigEdit::SetNoticeHideFullAccessWarning(acknowledged) => Ok(self.write_value(
                &[NOTICE_TABLE_KEY, "hide_full_access_warning"],
                value(*acknowledged),
            )),
            ConfigEdit::SetNoticeHideWorldWritableWarning(acknowledged) => Ok(self.write_value(
                &[NOTICE_TABLE_KEY, "hide_world_writable_warning"],
                value(*acknowledged),
            )),
            ConfigEdit::SetNoticeHideRateLimitModelNudge(acknowledged) => Ok(self.write_value(
                &[NOTICE_TABLE_KEY, "hide_rate_limit_model_nudge"],
                value(*acknowledged),
            )),
            ConfigEdit::SetNoticeHideModelMigrationPrompt(migration_config, acknowledged) => {
                Ok(self.write_value(
                    &[NOTICE_TABLE_KEY, migration_config.as_str()],
                    value(*acknowledged),
                ))
            }
            ConfigEdit::SetNoticeHideExternalConfigMigrationPromptHome(acknowledged) => Ok(self
                .write_value(
                    &[
                        NOTICE_TABLE_KEY,
                        "external_config_migration_prompts",
                        "home",
                    ],
                    value(*acknowledged),
                )),
            ConfigEdit::SetNoticeExternalConfigMigrationPromptHomeLastPromptedAt(timestamp) => {
                Ok(self.write_value(
                    &[
                        NOTICE_TABLE_KEY,
                        "external_config_migration_prompts",
                        "home_last_prompted_at",
                    ],
                    value(*timestamp),
                ))
            }
            ConfigEdit::SetNoticeHideExternalConfigMigrationPromptProject(
                project,
                acknowledged,
            ) => Ok(self.write_value(
                &[
                    NOTICE_TABLE_KEY,
                    "external_config_migration_prompts",
                    "projects",
                    project.as_str(),
                ],
                value(*acknowledged),
            )),
            ConfigEdit::SetNoticeExternalConfigMigrationPromptProjectLastPromptedAt(
                project,
                timestamp,
            ) => Ok(self.write_value(
                &[
                    NOTICE_TABLE_KEY,
                    "external_config_migration_prompts",
                    "project_last_prompted_at",
                    project.as_str(),
                ],
                value(*timestamp),
            )),
            ConfigEdit::RecordModelMigrationSeen { from, to } => Ok(self.write_value(
                &[NOTICE_TABLE_KEY, "model_migrations", from.as_str()],
                value(to.clone()),
            )),
            ConfigEdit::ReplaceMcpServers(servers) => Ok(self.replace_mcp_servers(servers)),
            ConfigEdit::AddToolSuggestDisabledTool(disabled_tool) => {
                Ok(self.add_tool_suggest_disabled_tool(disabled_tool))
            }
            ConfigEdit::SetSkillConfig { path, enabled } => {
                Ok(self.set_skill_config(SkillConfigSelector::Path(path.clone()), *enabled))
            }
            ConfigEdit::SetSkillConfigByName { name, enabled } => {
                Ok(self.set_skill_config(SkillConfigSelector::Name(name.clone()), *enabled))
            }
            ConfigEdit::SetPath { segments, value } => Ok(self.insert(segments, value.clone())),
            ConfigEdit::ClearPath { segments } => Ok(self.clear_owned(segments)),
            ConfigEdit::SetProjectTrustLevel { path, level } => {
                // Delegate to the existing, tested logic in config.rs to
                // ensure tables are explicit and migration is preserved.
                crate::config::set_project_trust_level_inner(
                    &mut self.doc,
                    path.as_path(),
                    *level,
                )?;
                Ok(true)
            }
        }
    }

    fn write_optional_value(&mut self, segments: &[&str], value: Option<TomlItem>) -> bool {
        match value {
            Some(item) => self.write_value(segments, item),
            None => self.clear(segments),
        }
    }

    fn write_value(&mut self, segments: &[&str], value: TomlItem) -> bool {
        let resolved = segments
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        self.insert(&resolved, value)
    }

    fn clear(&mut self, segments: &[&str]) -> bool {
        let resolved = segments
            .iter()
            .map(|segment| (*segment).to_string())
            .collect::<Vec<_>>();
        self.remove(&resolved)
    }

    fn add_tool_suggest_disabled_tool(&mut self, disabled_tool: &ToolSuggestDisabledTool) -> bool {
        let disabled_tools_item = self
            .doc
            .get("tool_suggest")
            .and_then(|item| item.as_table_like())
            .and_then(|table| table.get("disabled_tools"));
        let existing_from_array = disabled_tools_item
            .and_then(|item| item.as_value())
            .and_then(|value| value.as_array())
            .into_iter()
            .flat_map(|array| array.iter())
            .filter_map(document_helpers::parse_tool_suggest_disabled_tool);
        let existing_from_tables = disabled_tools_item
            .and_then(|item| match item {
                TomlItem::ArrayOfTables(array) => Some(array),
                _ => None,
            })
            .into_iter()
            .flat_map(|array| array.iter())
            .filter_map(document_helpers::parse_tool_suggest_disabled_tool_table);

        let mut seen = HashSet::new();
        let disabled_tools = existing_from_array
            .chain(existing_from_tables)
            .chain(std::iter::once(disabled_tool.clone()))
            .filter_map(|disabled_tool| disabled_tool.normalized())
            .filter(|disabled_tool| seen.insert(disabled_tool.clone()))
            .collect::<Vec<_>>();
        self.write_value(
            &["tool_suggest", "disabled_tools"],
            document_helpers::tool_suggest_disabled_tools_value(&disabled_tools),
        )
    }

    fn clear_owned(&mut self, segments: &[String]) -> bool {
        self.remove(segments)
    }

    fn replace_mcp_servers(&mut self, servers: &BTreeMap<String, McpServerConfig>) -> bool {
        if servers.is_empty() {
            return self.clear(&["mcp_servers"]);
        }

        let root = self.doc.as_table_mut();
        if !root.contains_key("mcp_servers") {
            root.insert(
                "mcp_servers",
                TomlItem::Table(document_helpers::new_implicit_table()),
            );
        }

        let Some(item) = root.get_mut("mcp_servers") else {
            return false;
        };

        if document_helpers::ensure_table_for_write(item).is_none() {
            *item = TomlItem::Table(document_helpers::new_implicit_table());
        }

        let Some(table) = item.as_table_mut() else {
            return false;
        };

        let keys_to_remove: Vec<String> = table
            .iter()
            .map(|(key, _)| key.to_string())
            .filter(|key| !servers.contains_key(key.as_str()))
            .collect();

        for key in keys_to_remove {
            table.remove(&key);
        }

        for (name, config) in servers {
            if let Some(existing) = table.get_mut(name.as_str()) {
                if let TomlItem::Value(value) = existing
                    && let Some(inline) = value.as_inline_table_mut()
                {
                    let replacement = document_helpers::serialize_mcp_server_inline(config);
                    document_helpers::merge_inline_table(inline, replacement);
                } else {
                    *existing = document_helpers::serialize_mcp_server(config);
                }
            } else {
                table.insert(name, document_helpers::serialize_mcp_server(config));
            }
        }

        true
    }

    fn set_skill_config(&mut self, selector: SkillConfigSelector, enabled: bool) -> bool {
        let selector = match selector {
            SkillConfigSelector::Name(name) => SkillConfigSelector::Name(name.trim().to_string()),
            SkillConfigSelector::Path(path) => {
                SkillConfigSelector::Path(PathBuf::from(normalize_skill_config_path(&path)))
            }
        };
        if matches!(&selector, SkillConfigSelector::Name(name) if name.is_empty()) {
            return false;
        }
        let mut remove_skills_table = false;
        let mut mutated = false;

        {
            let root = self.doc.as_table_mut();
            let skills_item = match root.get_mut("skills") {
                Some(item) => item,
                None => {
                    if enabled {
                        return false;
                    }
                    root.insert(
                        "skills",
                        TomlItem::Table(document_helpers::new_implicit_table()),
                    );
                    let Some(item) = root.get_mut("skills") else {
                        return false;
                    };
                    item
                }
            };

            if document_helpers::ensure_table_for_write(skills_item).is_none() {
                if enabled {
                    return false;
                }
                *skills_item = TomlItem::Table(document_helpers::new_implicit_table());
            }
            let Some(skills_table) = skills_item.as_table_mut() else {
                return false;
            };

            let config_item = match skills_table.get_mut("config") {
                Some(item) => item,
                None => {
                    if enabled {
                        return false;
                    }
                    skills_table.insert("config", TomlItem::ArrayOfTables(ArrayOfTables::new()));
                    let Some(item) = skills_table.get_mut("config") else {
                        return false;
                    };
                    item
                }
            };

            if !matches!(config_item, TomlItem::ArrayOfTables(_)) {
                if enabled {
                    return false;
                }
                *config_item = TomlItem::ArrayOfTables(ArrayOfTables::new());
            }

            let TomlItem::ArrayOfTables(overrides) = config_item else {
                return false;
            };

            let existing_index = overrides.iter().enumerate().find_map(|(idx, table)| {
                skill_config_selector_from_table(table)
                    .filter(|value| value == &selector)
                    .map(|_| idx)
            });

            if enabled {
                if let Some(index) = existing_index {
                    overrides.remove(index);
                    mutated = true;
                    if overrides.is_empty() {
                        skills_table.remove("config");
                        if skills_table.is_empty() {
                            remove_skills_table = true;
                        }
                    }
                }
            } else if let Some(index) = existing_index {
                for (idx, table) in overrides.iter_mut().enumerate() {
                    if idx == index {
                        write_skill_config_selector(table, &selector);
                        table["enabled"] = value(false);
                        mutated = true;
                        break;
                    }
                }
            } else {
                let mut entry = TomlTable::new();
                entry.set_implicit(false);
                write_skill_config_selector(&mut entry, &selector);
                entry["enabled"] = value(false);
                overrides.push(entry);
                mutated = true;
            }
        }

        if remove_skills_table {
            let root = self.doc.as_table_mut();
            root.remove("skills");
        }

        mutated
    }

    fn insert(&mut self, segments: &[String], value: TomlItem) -> bool {
        let Some((last, parents)) = segments.split_last() else {
            return false;
        };

        let Some(parent) = self.descend(parents, TraversalMode::Create) else {
            return false;
        };

        let mut value = value;
        if let Some(existing) = parent.get(last) {
            Self::preserve_decor(existing, &mut value);
        }
        parent[last] = value;
        true
    }

    fn remove(&mut self, segments: &[String]) -> bool {
        let Some((last, parents)) = segments.split_last() else {
            return false;
        };

        let Some(parent) = self.descend(parents, TraversalMode::Existing) else {
            return false;
        };

        parent.remove(last).is_some()
    }

    fn descend(&mut self, segments: &[String], mode: TraversalMode) -> Option<&mut TomlTable> {
        let mut current = self.doc.as_table_mut();

        for segment in segments {
            match mode {
                TraversalMode::Create => {
                    if !current.contains_key(segment.as_str()) {
                        current.insert(
                            segment.as_str(),
                            TomlItem::Table(document_helpers::new_implicit_table()),
                        );
                    }

                    let item = current.get_mut(segment.as_str())?;
                    current = document_helpers::ensure_table_for_write(item)?;
                }
                TraversalMode::Existing => {
                    let item = current.get_mut(segment.as_str())?;
                    current = document_helpers::ensure_table_for_read(item)?;
                }
            }
        }

        Some(current)
    }

    fn preserve_decor(existing: &TomlItem, replacement: &mut TomlItem) {
        match (existing, replacement) {
            (TomlItem::Table(existing_table), TomlItem::Table(replacement_table)) => {
                replacement_table
                    .decor_mut()
                    .clone_from(existing_table.decor());
                for (key, existing_item) in existing_table.iter() {
                    if let (Some(existing_key), Some(mut replacement_key)) =
                        (existing_table.key(key), replacement_table.key_mut(key))
                    {
                        replacement_key
                            .leaf_decor_mut()
                            .clone_from(existing_key.leaf_decor());
                        replacement_key
                            .dotted_decor_mut()
                            .clone_from(existing_key.dotted_decor());
                    }
                    if let Some(replacement_item) = replacement_table.get_mut(key) {
                        Self::preserve_decor(existing_item, replacement_item);
                    }
                }
            }
            (TomlItem::Value(existing_value), TomlItem::Value(replacement_value)) => {
                replacement_value
                    .decor_mut()
                    .clone_from(existing_value.decor());
            }
            _ => {}
        }
    }
}

fn normalize_skill_config_path(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn skill_config_selector_from_table(table: &TomlTable) -> Option<SkillConfigSelector> {
    let path = table
        .get("path")
        .and_then(|item| item.as_str())
        .map(Path::new)
        .map(|path| SkillConfigSelector::Path(PathBuf::from(normalize_skill_config_path(path))));
    let name = table
        .get("name")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| SkillConfigSelector::Name(name.to_string()));

    match (path, name) {
        (Some(selector), None) | (None, Some(selector)) => Some(selector),
        _ => None,
    }
}

fn write_skill_config_selector(table: &mut TomlTable, selector: &SkillConfigSelector) {
    match selector {
        SkillConfigSelector::Name(name) => {
            table.remove("path");
            table["name"] = value(name.clone());
        }
        SkillConfigSelector::Path(path) => {
            table.remove("name");
            table["path"] = value(path.to_string_lossy().to_string());
        }
    }
}

/// Persist edits using a blocking strategy.
pub fn apply_blocking(codex_home: &Path, edits: &[ConfigEdit]) -> anyhow::Result<()> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    apply_blocking_to_resolved_file(&config_path, edits)
}

fn apply_blocking_to_resolved_file(
    resolved_config_file: &Path,
    edits: &[ConfigEdit],
) -> anyhow::Result<()> {
    if edits.is_empty() {
        return Ok(());
    }

    with_config_write_lock(resolved_config_file, |write_paths| {
        let serialized = match write_paths.read_path.as_ref() {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(contents) => contents,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(err) => return Err(err.into()),
            },
            None => String::new(),
        };

        let doc = if serialized.is_empty() {
            DocumentMut::new()
        } else {
            serialized.parse::<DocumentMut>()?
        };

        let mut document = ConfigDocument::new(doc);
        let mut mutated = false;

        for edit in edits {
            mutated |= document.apply(edit)?;
        }

        if !mutated {
            return Ok(());
        }

        write_atomically(&write_paths.write_path, &document.doc.to_string()).with_context(
            || {
                format!(
                    "failed to persist config at {}",
                    write_paths.write_path.display()
                )
            },
        )?;

        Ok(())
    })
}

/// Persist edits asynchronously by offloading the blocking writer.
///
pub async fn apply(codex_home: &Path, edits: Vec<ConfigEdit>) -> anyhow::Result<()> {
    let codex_home = codex_home.to_path_buf();
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    task::spawn_blocking(move || apply_blocking_to_resolved_file(&config_path, &edits))
        .await
        .context("config persistence task panicked")?
}

/// Fluent builder to batch config edits and apply them atomically.
#[derive(Default)]
pub struct ConfigEditsBuilder {
    config_path: PathBuf,
    edits: Vec<ConfigEdit>,
}

impl ConfigEditsBuilder {
    pub fn new(codex_home: &Path) -> Self {
        Self::for_config_path(&codex_home.join(CONFIG_TOML_FILE))
    }

    pub fn for_config(config: &crate::config::Config) -> Self {
        let config_path = config
            .config_layer_stack
            .get_user_config_file()
            .map(codex_utils_absolute_path::AbsolutePathBuf::to_path_buf)
            .unwrap_or_else(|| config.codex_home.join(CONFIG_TOML_FILE).to_path_buf());
        Self::for_config_path(&config_path)
    }

    pub fn for_config_path(config_path: &Path) -> Self {
        Self {
            config_path: config_path.to_path_buf(),
            edits: Vec::new(),
        }
    }

    pub fn set_model(mut self, model: Option<&str>, effort: Option<ReasoningEffort>) -> Self {
        self.edits.push(ConfigEdit::SetModel {
            model: model.map(ToOwned::to_owned),
            effort,
        });
        self
    }

    pub fn set_service_tier(mut self, service_tier: Option<String>) -> Self {
        self.edits.push(ConfigEdit::SetServiceTier { service_tier });
        self
    }

    pub fn set_personality(mut self, personality: Option<Personality>) -> Self {
        self.edits
            .push(ConfigEdit::SetModelPersonality { personality });
        self
    }

    pub fn set_hide_full_access_warning(mut self, acknowledged: bool) -> Self {
        self.edits
            .push(ConfigEdit::SetNoticeHideFullAccessWarning(acknowledged));
        self
    }

    pub fn set_hide_world_writable_warning(mut self, acknowledged: bool) -> Self {
        self.edits
            .push(ConfigEdit::SetNoticeHideWorldWritableWarning(acknowledged));
        self
    }

    pub fn set_hide_rate_limit_model_nudge(mut self, acknowledged: bool) -> Self {
        self.edits
            .push(ConfigEdit::SetNoticeHideRateLimitModelNudge(acknowledged));
        self
    }

    pub fn set_hide_model_migration_prompt(mut self, model: &str, acknowledged: bool) -> Self {
        self.edits
            .push(ConfigEdit::SetNoticeHideModelMigrationPrompt(
                model.to_string(),
                acknowledged,
            ));
        self
    }

    pub fn set_hide_external_config_migration_prompt_home(mut self, acknowledged: bool) -> Self {
        self.edits
            .push(ConfigEdit::SetNoticeHideExternalConfigMigrationPromptHome(
                acknowledged,
            ));
        self
    }

    pub fn set_hide_external_config_migration_prompt_project(
        mut self,
        project: &str,
        acknowledged: bool,
    ) -> Self {
        self.edits.push(
            ConfigEdit::SetNoticeHideExternalConfigMigrationPromptProject(
                project.to_string(),
                acknowledged,
            ),
        );
        self
    }

    pub fn record_model_migration_seen(mut self, from: &str, to: &str) -> Self {
        self.edits.push(ConfigEdit::RecordModelMigrationSeen {
            from: from.to_string(),
            to: to.to_string(),
        });
        self
    }

    pub fn set_model_availability_nux_count(mut self, shown_count: &HashMap<String, u32>) -> Self {
        self.edits
            .extend(model_availability_nux_count_edits(shown_count));
        self
    }

    pub fn replace_mcp_servers(mut self, servers: &BTreeMap<String, McpServerConfig>) -> Self {
        self.edits
            .push(ConfigEdit::ReplaceMcpServers(servers.clone()));
        self
    }

    pub fn set_project_trust_level<P: Into<PathBuf>>(
        mut self,
        project_path: P,
        trust_level: TrustLevel,
    ) -> Self {
        self.edits.push(ConfigEdit::SetProjectTrustLevel {
            path: project_path.into(),
            level: trust_level,
        });
        self
    }

    /// Enable or disable a feature flag by key under the `[features]` table.
    ///
    /// Disabling a default-false feature clears the key instead of
    /// persisting `false`, so the config does not pin the feature once it
    /// graduates to globally enabled.
    pub fn set_feature_enabled(mut self, key: &str, enabled: bool) -> Self {
        let segments = vec!["features".to_string(), key.to_string()];
        let is_default_false_feature = FEATURES
            .iter()
            .find(|spec| spec.key == key)
            .is_some_and(|spec| !spec.default_enabled);
        if enabled || !is_default_false_feature {
            self.edits.push(ConfigEdit::SetPath {
                segments,
                value: value(enabled),
            });
        } else {
            self.edits.push(ConfigEdit::ClearPath { segments });
        }
        self
    }

    pub fn set_windows_sandbox_mode(mut self, mode: &str) -> Self {
        self.edits.push(ConfigEdit::SetPath {
            segments: vec!["windows".to_string(), "sandbox".to_string()],
            value: value(mode),
        });
        self
    }

    pub fn set_realtime_microphone(mut self, microphone: Option<&str>) -> Self {
        let segments = vec!["audio".to_string(), "microphone".to_string()];
        match microphone {
            Some(microphone) => self.edits.push(ConfigEdit::SetPath {
                segments,
                value: value(microphone),
            }),
            None => self.edits.push(ConfigEdit::ClearPath { segments }),
        }
        self
    }

    pub fn set_realtime_speaker(mut self, speaker: Option<&str>) -> Self {
        let segments = vec!["audio".to_string(), "speaker".to_string()];
        match speaker {
            Some(speaker) => self.edits.push(ConfigEdit::SetPath {
                segments,
                value: value(speaker),
            }),
            None => self.edits.push(ConfigEdit::ClearPath { segments }),
        }
        self
    }

    pub fn set_realtime_voice(mut self, voice: Option<&str>) -> Self {
        let segments = vec!["realtime".to_string(), "voice".to_string()];
        match voice {
            Some(voice) => self.edits.push(ConfigEdit::SetPath {
                segments,
                value: value(voice),
            }),
            None => self.edits.push(ConfigEdit::ClearPath { segments }),
        }
        self
    }

    pub fn clear_legacy_windows_sandbox_keys(mut self) -> Self {
        for key in [
            "experimental_windows_sandbox",
            "elevated_windows_sandbox",
            "enable_experimental_windows_sandbox",
        ] {
            let segments = vec!["features".to_string(), key.to_string()];
            self.edits.push(ConfigEdit::ClearPath { segments });
        }
        self
    }

    pub fn set_session_picker_view(mut self, mode: SessionPickerViewMode) -> Self {
        self.edits.push(ConfigEdit::SetPath {
            segments: vec!["tui".to_string(), "session_picker_view".to_string()],
            value: value(mode.to_string()),
        });
        self
    }

    pub fn with_edits<I>(mut self, edits: I) -> Self
    where
        I: IntoIterator<Item = ConfigEdit>,
    {
        self.edits.extend(edits);
        self
    }

    /// Apply edits on a blocking thread.
    pub fn apply_blocking(self) -> anyhow::Result<()> {
        apply_blocking_to_resolved_file(&self.config_path, &self.edits)
    }

    /// Apply edits asynchronously via a blocking offload.
    pub async fn apply(self) -> anyhow::Result<()> {
        task::spawn_blocking(move || {
            apply_blocking_to_resolved_file(&self.config_path, &self.edits)
        })
        .await
        .context("config persistence task panicked")?
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
