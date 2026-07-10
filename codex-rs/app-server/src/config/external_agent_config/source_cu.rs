use codex_external_agent_migration::CommandDescriptionMode;
use codex_external_agent_migration::CommandMigrationProfile;
use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::build_mcp_config_from_json_file;
use codex_external_agent_migration::count_missing_commands_with_profile;
use codex_external_agent_migration::hook_migration_event_names_from_json_file;
use codex_external_agent_migration::import_commands_with_profile;
use codex_external_agent_migration::import_hooks_from_json_file;
use codex_external_agent_migration::import_subagents_with_rewrite_profile;
use codex_external_agent_migration::missing_command_names_with_profile;
use serde_json::Value as JsonValue;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::is_non_empty_text_file;
use super::source::InstructionSourceGroup;

pub(super) const CONFIG_DIR: &str = ".cursor";
pub(super) const LEGACY_RULES_FILE: &str = ".cursorrules";
pub(super) const HOME_CONFIG_FILE: &str = "cli-config.json";
pub(super) const PROJECT_CONFIG_FILE: &str = "cli.json";
pub(super) const SANDBOX_CONFIG_FILE: &str = "sandbox.json";
pub(super) const HOOKS_CONFIG_FILE: &str = "hooks.json";
const SANDBOX_SETTINGS_KEY: &str = "__cursorSandbox";
pub(super) const REWRITE_PROFILE: RewriteProfile =
    RewriteProfile::new(LEGACY_RULES_FILE, &[]).with_case_sensitive_term_variants(&["Cursor"]);
const COMMAND_MIGRATION_PROFILE: CommandMigrationProfile = CommandMigrationProfile::new(
    REWRITE_PROFILE,
    CommandDescriptionMode::UseSourceNameFallback,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CachedMarketplacePlugins {
    pub(super) name: String,
    pub(super) source: PathBuf,
    pub(super) plugin_names: Vec<String>,
}

pub(super) fn build_mcp_config(source_dir: &Path) -> io::Result<TomlValue> {
    build_mcp_config_from_json_file(&source_dir.join("mcp.json"))
}

pub(super) fn effective_settings(
    source_dir: &Path,
    source_settings: &Path,
) -> io::Result<Option<JsonValue>> {
    let mut effective = super::read_external_settings(source_settings)?;
    let sandbox_settings = super::read_external_settings(&source_dir.join(SANDBOX_CONFIG_FILE))?;
    if let Some(sandbox_settings) = sandbox_settings {
        let effective = effective.get_or_insert_with(|| JsonValue::Object(serde_json::Map::new()));
        let Some(effective) = effective.as_object_mut() else {
            return Err(super::invalid_data_error(
                "external agent settings root must be an object",
            ));
        };
        effective.insert(SANDBOX_SETTINGS_KEY.to_string(), sandbox_settings);
    }
    Ok(effective)
}

pub(super) fn append_sandbox_config(
    root: &mut toml::map::Map<String, TomlValue>,
    settings: &serde_json::Map<String, JsonValue>,
) {
    let Some(sandbox) = settings
        .get(SANDBOX_SETTINGS_KEY)
        .and_then(JsonValue::as_object)
    else {
        return;
    };
    let sandbox_mode = match sandbox.get("type").and_then(JsonValue::as_str) {
        Some("workspace_readwrite") => Some("workspace-write"),
        Some("read_only") => Some("read-only"),
        _ => None,
    };
    if let Some(sandbox_mode) = sandbox_mode {
        root.insert(
            "sandbox_mode".to_string(),
            TomlValue::String(sandbox_mode.to_string()),
        );
    }
    if sandbox_mode != Some("workspace-write") {
        return;
    }

    let mut workspace_write = toml::map::Map::new();
    if let Some(paths) = sandbox
        .get("additionalReadwritePaths")
        .and_then(JsonValue::as_array)
    {
        let paths = paths
            .iter()
            .filter_map(JsonValue::as_str)
            .filter(|path| Path::new(path).is_absolute())
            .map(|path| TomlValue::String(path.to_string()))
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            workspace_write.insert("writable_roots".to_string(), TomlValue::Array(paths));
        }
    }
    if sandbox.get("disableTmpWrite").and_then(JsonValue::as_bool) == Some(true) {
        workspace_write.insert("exclude_slash_tmp".to_string(), TomlValue::Boolean(true));
        workspace_write.insert(
            "exclude_tmpdir_env_var".to_string(),
            TomlValue::Boolean(true),
        );
    }
    if sandbox
        .get("networkPolicy")
        .and_then(JsonValue::as_object)
        .and_then(|network| network.get("default"))
        .and_then(JsonValue::as_str)
        == Some("allow")
    {
        workspace_write.insert("network_access".to_string(), TomlValue::Boolean(true));
    }
    if !workspace_write.is_empty() {
        root.insert(
            "sandbox_workspace_write".to_string(),
            TomlValue::Table(workspace_write),
        );
    }
}

pub(super) fn repo_instruction_source_groups(
    repo_root: &Path,
) -> io::Result<Vec<InstructionSourceGroup>> {
    let mut scopes = rule_scope_directories(repo_root)?;
    if !scopes.iter().any(|scope| scope == repo_root) {
        scopes.push(repo_root.to_path_buf());
    }
    scopes.sort();
    scopes.dedup();

    let mut groups = Vec::new();
    for scope in scopes {
        let legacy_rules = (scope == repo_root).then(|| repo_root.join(LEGACY_RULES_FILE));
        let sources = instruction_sources(legacy_rules, &scope.join(CONFIG_DIR).join("rules"))?;
        if !sources.is_empty() {
            groups.push(InstructionSourceGroup { scope, sources });
        }
    }
    Ok(groups)
}

pub(super) fn home_instruction_sources(external_agent_home: &Path) -> io::Result<Vec<PathBuf>> {
    instruction_sources(
        /*legacy_rules*/ None,
        &external_agent_home.join("rules"),
    )
}

pub(super) fn read_instruction_source(path: &Path) -> io::Result<String> {
    let contents = fs::read_to_string(path)?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("mdc") {
        Ok(strip_markdown_frontmatter(&contents).to_string())
    } else {
        Ok(contents)
    }
}

pub(super) fn import_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    import_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn count_missing_source_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<usize> {
    count_missing_commands_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn missing_source_command_names(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    missing_command_names_with_profile(source_commands, target_skills, COMMAND_MIGRATION_PROFILE)
}

pub(super) fn import_source_subagents(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<Vec<String>> {
    import_subagents_with_rewrite_profile(source_agents, target_agents, REWRITE_PROFILE)
}

pub(super) fn source_hook_event_names(
    source_dir: &Path,
    target_hooks: &Path,
) -> io::Result<Vec<String>> {
    hook_migration_event_names_from_json_file(
        source_dir,
        &source_dir.join(HOOKS_CONFIG_FILE),
        target_hooks,
        REWRITE_PROFILE,
    )
}

pub(super) fn import_source_hooks(source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
    import_hooks_from_json_file(
        source_dir,
        &source_dir.join(HOOKS_CONFIG_FILE),
        target_hooks,
        REWRITE_PROFILE,
    )
}

pub(super) fn cached_marketplace_plugins(
    external_agent_home: &Path,
) -> io::Result<Vec<CachedMarketplacePlugins>> {
    let marketplaces_root = external_agent_home.join("plugins/marketplaces");
    let cache_root = external_agent_home.join("plugins/cache");
    if !marketplaces_root.is_dir() || !cache_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut marketplaces = Vec::new();
    for entry in fs::read_dir(marketplaces_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let marketplace_root = entry.path();
        let manifest_path = marketplace_root.join(".cursor-plugin/marketplace.json");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = match fs::read_to_string(&manifest_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                tracing::warn!(
                    path = %manifest_path.display(),
                    error = %err,
                    "ignoring unreadable external marketplace manifest"
                );
                continue;
            }
        };
        let manifest: JsonValue = match serde_json::from_str(&manifest) {
            Ok(manifest) => manifest,
            Err(err) => {
                tracing::warn!(
                    path = %manifest_path.display(),
                    error = %err,
                    "ignoring invalid external marketplace manifest"
                );
                continue;
            }
        };
        let Some(name) = manifest.get("name").and_then(JsonValue::as_str) else {
            continue;
        };
        let available_plugins = manifest
            .get("plugins")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
            .filter_map(|plugin| plugin.get("name").and_then(JsonValue::as_str))
            .collect::<BTreeSet<_>>();
        let cache_marketplace = cache_root.join(entry.file_name());
        if !cache_marketplace.is_dir() {
            continue;
        }
        let mut plugin_names = fs::read_dir(cache_marketplace)?
            .filter_map(Result::ok)
            .filter_map(|plugin| {
                plugin
                    .file_type()
                    .ok()
                    .filter(std::fs::FileType::is_dir)
                    .and_then(|_| plugin.file_name().into_string().ok())
            })
            .filter(|plugin_name| available_plugins.contains(plugin_name.as_str()))
            .collect::<Vec<_>>();
        plugin_names.sort();
        if !plugin_names.is_empty() {
            marketplaces.push(CachedMarketplacePlugins {
                name: name.to_string(),
                source: marketplace_root,
                plugin_names,
            });
        }
    }
    marketplaces.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(marketplaces)
}

fn instruction_sources(
    legacy_rules: Option<PathBuf>,
    rules_dir: &Path,
) -> io::Result<Vec<PathBuf>> {
    let mut sources = Vec::new();
    if let Some(legacy_rules) = legacy_rules
        && is_non_empty_text_file(&legacy_rules)?
    {
        sources.push(legacy_rules);
    }

    collect_rule_sources(rules_dir, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn rule_scope_directories(repo_root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut scopes = Vec::new();
    let mut pending = vec![repo_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(err) if directory == repo_root => return Err(err),
            Err(_) => continue,
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let path = entry.path();
            if entry.file_name().to_str() == Some(CONFIG_DIR) {
                if path.join("rules").is_dir() {
                    scopes.push(directory.clone());
                }
                continue;
            }
            if !should_skip_rule_scope_directory(&path) {
                pending.push(path);
            }
        }
    }
    scopes.sort();
    scopes.dedup();
    Ok(scopes)
}

fn should_skip_rule_scope_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn" | "node_modules" | "target"))
}

fn collect_rule_sources(rules_dir: &Path, sources: &mut Vec<PathBuf>) -> io::Result<()> {
    if !rules_dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(rules_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_rule_sources(&path, sources)?;
            continue;
        }
        if !file_type.is_file()
            || !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("md" | "mdc")
            )
        {
            continue;
        }
        let contents = fs::read_to_string(&path)?;
        if rule_is_always_applied(&contents)
            && !strip_markdown_frontmatter(&contents).trim().is_empty()
        {
            sources.push(path);
        }
    }
    Ok(())
}

fn rule_is_always_applied(contents: &str) -> bool {
    let Some((frontmatter, _body)) = split_markdown_frontmatter(contents) else {
        return false;
    };
    frontmatter.lines().any(|line| {
        line.split_once(':').is_some_and(|(key, value)| {
            key.trim() == "alwaysApply"
                && yaml_scalar_without_comment(value).eq_ignore_ascii_case("true")
        })
    })
}

fn split_markdown_frontmatter(contents: &str) -> Option<(&str, &str)> {
    let rest = contents.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let mut frontmatter_end = 0usize;
    for line in rest.split_inclusive('\n') {
        let line_contents = line.trim_end_matches(['\r', '\n']);
        if line_contents == "---" {
            let body = &rest[frontmatter_end + line.len()..];
            return Some((&rest[..frontmatter_end], body));
        }
        frontmatter_end += line.len();
    }
    None
}

fn strip_markdown_frontmatter(contents: &str) -> &str {
    split_markdown_frontmatter(contents).map_or(contents, |(_frontmatter, body)| body)
}

fn yaml_scalar_without_comment(value: &str) -> &str {
    let value = value.trim();
    let comment_start = value.char_indices().find_map(|(index, character)| {
        (character == '#'
            && value[..index]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace))
        .then_some(index)
    });
    comment_start.map_or(value, |comment_start| value[..comment_start].trim_end())
}
