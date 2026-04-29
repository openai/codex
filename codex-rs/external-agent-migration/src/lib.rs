//! Migration helpers for importing external-agent configuration into Codex.

use codex_hooks::HOOK_EVENT_NAMES;
use codex_hooks::HOOK_EVENT_NAMES_WITH_MATCHERS;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
const EXTERNAL_AGENT_MCP_CONFIG_FILE: &str = ".mcp.json";
const EXTERNAL_AGENT_HOOKS_SUBDIR: &str = "hooks";
const EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR: &str = "hooks";
const COMMAND_SKILL_PREFIX: &str = "source-command";
const MAX_SKILL_NAME_LEN: usize = 64;
const MAX_SKILL_DESCRIPTION_LEN: usize = 1024;

#[derive(Debug)]
struct ParsedDocument {
    frontmatter: BTreeMap<String, FrontmatterValue>,
    body: String,
    frontmatter_error: Option<String>,
}

#[derive(Debug)]
enum FrontmatterValue {
    Scalar(String),
    Other,
}

#[derive(Debug)]
struct AgentMetadata {
    name: String,
    description: String,
    permission_mode: Option<String>,
    effort: Option<String>,
}

pub fn build_mcp_config_from_external(
    source_root: &Path,
    external_agent_home: Option<&Path>,
    settings: Option<&JsonValue>,
) -> io::Result<TomlValue> {
    let mcp_servers = read_external_mcp_servers(source_root, external_agent_home)?;
    if mcp_servers.is_empty() {
        return Ok(TomlValue::Table(Default::default()));
    }

    let enabled_servers = settings
        .and_then(|settings| settings.get("enabledMcpjsonServers"))
        .map(json_string_vec)
        .unwrap_or_default();
    let disabled_servers = settings
        .and_then(|settings| settings.get("disabledMcpjsonServers"))
        .map(json_string_vec)
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>();

    let mut servers = toml::map::Map::new();
    for (server_name, server_config) in mcp_servers {
        if let Some(server) = mcp_server_toml_table(
            &server_name,
            server_config.as_object(),
            &enabled_servers,
            &disabled_servers,
        ) {
            servers.insert(server_name.clone(), TomlValue::Table(server));
        }
    }

    if servers.is_empty() {
        return Ok(TomlValue::Table(Default::default()));
    }

    let mut root = toml::map::Map::new();
    root.insert("mcp_servers".to_string(), TomlValue::Table(servers));
    Ok(TomlValue::Table(root))
}

pub fn hooks_migration_description(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
) -> io::Result<Option<String>> {
    if hook_migration_event_names(source_external_agent_dir, target_hooks)?.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!(
        "Migrate hooks from {} to {}",
        source_external_agent_dir.display(),
        target_hooks.display()
    )))
}

pub fn hook_migration_event_names(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
) -> io::Result<Vec<String>> {
    let migration = hook_migration(source_external_agent_dir, target_hooks.parent())?;
    Ok(migration.keys().cloned().collect())
}

pub fn import_hooks(source_external_agent_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
    let Some(parent) = target_hooks.parent() else {
        return Err(invalid_data_error("hooks target path has no parent"));
    };
    let migration = hook_migration(source_external_agent_dir, Some(parent))?;
    if migration.is_empty() {
        return Ok(false);
    }

    fs::create_dir_all(parent)?;

    let mut wrote_active_hooks = false;
    if is_missing_or_empty_text_file(target_hooks)? {
        copy_hook_scripts(source_external_agent_dir, parent)?;
        let mut payload = serde_json::Map::new();
        payload.insert("hooks".to_string(), JsonValue::Object(migration));
        let rendered = serde_json::to_string_pretty(&JsonValue::Object(payload))
            .map_err(|err| invalid_data_error(format!("failed to serialize hooks.json: {err}")))?;
        fs::write(target_hooks, format!("{rendered}\n"))?;
        wrote_active_hooks = true;
    }

    Ok(wrote_active_hooks)
}

pub fn count_missing_subagents(source_agents: &Path, target_agents: &Path) -> io::Result<usize> {
    Ok(missing_subagent_names(source_agents, target_agents)?.len())
}

pub fn missing_subagent_names(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for source_file in agent_source_files(source_agents)? {
        let document = parse_document(&source_file)?;
        let Some(metadata) = agent_metadata(&document) else {
            continue;
        };
        let Some(target) = subagent_target_file(&source_file, target_agents) else {
            continue;
        };
        if !target.exists() {
            names.push(metadata.name);
        }
    }
    Ok(names)
}

pub fn import_subagents(source_agents: &Path, target_agents: &Path) -> io::Result<usize> {
    if !source_agents.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(target_agents)?;
    let mut imported = 0usize;
    for source_file in agent_source_files(source_agents)? {
        let Some(target) = subagent_target_file(&source_file, target_agents) else {
            continue;
        };
        if target.exists() {
            continue;
        }
        let document = parse_document(&source_file)?;
        let Some(metadata) = agent_metadata(&document) else {
            continue;
        };
        fs::write(&target, render_agent_toml(&document.body, &metadata)?)?;
        imported += 1;
    }

    Ok(imported)
}

pub fn count_missing_commands(source_commands: &Path, target_skills: &Path) -> io::Result<usize> {
    Ok(missing_command_names(source_commands, target_skills)?.len())
}

pub fn missing_command_names(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<Vec<String>> {
    Ok(unique_supported_command_sources(source_commands)?
        .into_iter()
        .filter(|(_source_file, name)| !target_skills.join(name).exists())
        .map(|(_source_file, name)| name)
        .collect())
}

pub fn import_commands(source_commands: &Path, target_skills: &Path) -> io::Result<usize> {
    if !source_commands.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(target_skills)?;
    let mut imported = 0usize;
    for (source_file, name) in unique_supported_command_sources(source_commands)? {
        let document = parse_document(&source_file)?;
        let target_dir = target_skills.join(&name);
        if target_dir.exists() {
            continue;
        }
        fs::create_dir_all(&target_dir)?;
        let source_name = command_source_name(source_commands, &source_file);
        let description = command_skill_description(&document, &source_name);
        fs::write(
            target_dir.join("SKILL.md"),
            render_command_skill(&document.body, &name, &description, &source_name),
        )?;
        imported += 1;
    }

    Ok(imported)
}

fn read_external_mcp_servers(
    source_root: &Path,
    external_agent_home: Option<&Path>,
) -> io::Result<BTreeMap<String, JsonValue>> {
    let mut servers = BTreeMap::new();
    let project_config_file = external_agent_project_config_file();
    for relative_path in [
        EXTERNAL_AGENT_MCP_CONFIG_FILE.to_string(),
        project_config_file.clone(),
    ] {
        let source_file = source_root.join(&relative_path);
        if !source_file.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&source_file)?;
        let parsed: JsonValue = serde_json::from_str(&raw)
            .map_err(|err| invalid_data_error(format!("invalid MCP config: {err}")))?;
        append_mcp_servers_from_value(&parsed, &mut servers, McpServerMerge::Overwrite);
        if relative_path == project_config_file
            && let Some(projects) = parsed.get("projects").and_then(JsonValue::as_object)
        {
            for (project_path, project_config) in projects {
                if project_path_matches_source_root(project_path, source_root) {
                    append_mcp_servers_from_value(
                        project_config,
                        &mut servers,
                        McpServerMerge::Overwrite,
                    );
                }
            }
        }
    }
    if let Some(external_agent_root) = external_agent_home.and_then(Path::parent)
        && external_agent_root != source_root
    {
        append_external_agent_project_mcp_servers(
            &external_agent_root.join(external_agent_project_config_file()),
            source_root,
            &mut servers,
        )?;
    }

    Ok(servers)
}

fn append_external_agent_project_mcp_servers(
    source_file: &Path,
    source_root: &Path,
    servers: &mut BTreeMap<String, JsonValue>,
) -> io::Result<()> {
    if !source_file.is_file() {
        return Ok(());
    }
    let raw = fs::read_to_string(source_file)?;
    let parsed: JsonValue = serde_json::from_str(&raw)
        .map_err(|err| invalid_data_error(format!("invalid MCP config: {err}")))?;
    let Some(projects) = parsed.get("projects").and_then(JsonValue::as_object) else {
        return Ok(());
    };
    for (project_path, project_config) in projects {
        if project_path_matches_source_root(project_path, source_root) {
            append_mcp_servers_from_value(
                project_config,
                servers,
                McpServerMerge::PreserveExisting,
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum McpServerMerge {
    Overwrite,
    PreserveExisting,
}

fn append_mcp_servers_from_value(
    value: &JsonValue,
    servers: &mut BTreeMap<String, JsonValue>,
    merge: McpServerMerge,
) {
    let Some(mcp_servers) = value.get("mcpServers").and_then(JsonValue::as_object) else {
        return;
    };
    for (server_name, server_config) in mcp_servers {
        match merge {
            McpServerMerge::Overwrite => {
                servers.insert(server_name.clone(), server_config.clone());
            }
            McpServerMerge::PreserveExisting => {
                servers
                    .entry(server_name.clone())
                    .or_insert_with(|| server_config.clone());
            }
        }
    }
}

fn project_path_matches_source_root(project_path: &str, source_root: &Path) -> bool {
    let project_path = Path::new(project_path);
    if project_path == source_root {
        return true;
    }
    let Ok(project_path) = project_path.canonicalize() else {
        return false;
    };
    source_root
        .canonicalize()
        .is_ok_and(|source_root| source_root == project_path)
}

fn mcp_server_toml_table(
    server_name: &str,
    server_config: Option<&serde_json::Map<String, JsonValue>>,
    enabled_servers: &[String],
    disabled_servers: &BTreeSet<String>,
) -> Option<toml::map::Map<String, TomlValue>> {
    let mut table = toml::map::Map::new();
    let server_config = server_config?;
    let transport_type = server_config.get("type").and_then(JsonValue::as_str);
    if mcp_server_is_disabled(
        server_name,
        server_config,
        enabled_servers,
        disabled_servers,
    ) {
        return None;
    }

    if let Some(command) = server_config.get("command").and_then(json_string) {
        if !matches!(transport_type, None | Some("stdio")) {
            return None;
        }
        if contains_env_placeholder(&command) {
            return None;
        }
        table.insert("command".to_string(), TomlValue::String(command));
        if let Some(args) = server_config.get("args") {
            let args = json_string_vec(args);
            if args.iter().any(|arg| contains_env_placeholder(arg)) {
                return None;
            }
            let args = args.into_iter().map(TomlValue::String).collect::<Vec<_>>();
            if !args.is_empty() {
                table.insert("args".to_string(), TomlValue::Array(args));
            }
        }
        if let Some(env) = server_config.get("env").and_then(JsonValue::as_object) {
            append_env_config(&mut table, env)?;
        }
    } else if let Some(url) = server_config.get("url").and_then(json_string) {
        if !matches!(
            transport_type,
            None | Some("http") | Some("streamable_http")
        ) {
            return None;
        }
        if contains_env_placeholder(&url) {
            return None;
        }
        table.insert("url".to_string(), TomlValue::String(url));
        if let Some(headers) = server_config.get("headers").and_then(JsonValue::as_object) {
            append_header_config(&mut table, headers)?;
        }
    } else {
        return None;
    }

    Some(table)
}

fn mcp_server_is_disabled(
    server_name: &str,
    server_config: &serde_json::Map<String, JsonValue>,
    enabled_servers: &[String],
    disabled_servers: &BTreeSet<String>,
) -> bool {
    server_config
        .get("enabled")
        .and_then(JsonValue::as_bool)
        .is_some_and(|enabled| !enabled)
        || server_config
            .get("disabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false)
        || (!enabled_servers.is_empty() && !enabled_servers.iter().any(|name| name == server_name))
        || disabled_servers.contains(server_name)
}

fn append_header_config(
    table: &mut toml::map::Map<String, TomlValue>,
    headers: &serde_json::Map<String, JsonValue>,
) -> Option<()> {
    let mut static_headers = toml::map::Map::new();
    let mut env_headers = toml::map::Map::new();

    for (key, value) in headers {
        let header_value = json_string(value).unwrap_or_else(|| value.to_string());
        if key.eq_ignore_ascii_case("authorization")
            && let Some(token_env) = header_value
                .strip_prefix("Bearer ")
                .and_then(parse_env_placeholder)
        {
            table.insert(
                "bearer_token_env_var".to_string(),
                TomlValue::String(token_env),
            );
            continue;
        }

        if let Some(env_var) = parse_env_placeholder(&header_value) {
            env_headers.insert(key.clone(), TomlValue::String(env_var));
        } else if contains_env_placeholder(&header_value) {
            return None;
        } else {
            static_headers.insert(key.clone(), TomlValue::String(header_value));
        }
    }

    if !static_headers.is_empty() {
        table.insert("http_headers".to_string(), TomlValue::Table(static_headers));
    }
    if !env_headers.is_empty() {
        table.insert(
            "env_http_headers".to_string(),
            TomlValue::Table(env_headers),
        );
    }
    Some(())
}

fn append_env_config(
    table: &mut toml::map::Map<String, TomlValue>,
    env: &serde_json::Map<String, JsonValue>,
) -> Option<()> {
    let mut static_env = toml::map::Map::new();
    let mut env_vars = Vec::new();

    for (key, value) in env {
        let env_value = json_string(value).unwrap_or_else(|| value.to_string());
        if parse_env_placeholder(&env_value).as_deref() == Some(key.as_str()) {
            env_vars.push(TomlValue::String(key.clone()));
        } else if contains_env_placeholder(&env_value) {
            return None;
        } else {
            static_env.insert(key.clone(), TomlValue::String(env_value));
        }
    }

    if !env_vars.is_empty() {
        table.insert("env_vars".to_string(), TomlValue::Array(env_vars));
    }
    if !static_env.is_empty() {
        table.insert("env".to_string(), TomlValue::Table(static_env));
    }
    Some(())
}

fn parse_env_placeholder(value: &str) -> Option<String> {
    let inner = value.strip_prefix("${")?.strip_suffix('}')?;
    let name = inner
        .split_once(":-")
        .map_or(inner, |(name, _default)| name);
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return None;
    }
    Some(name.to_string())
}

fn contains_env_placeholder(value: &str) -> bool {
    value.contains("${")
}

fn hook_migration(
    source_external_agent_dir: &Path,
    target_config_dir: Option<&Path>,
) -> io::Result<serde_json::Map<String, JsonValue>> {
    let mut settings_files = Vec::new();
    let mut disable_all_hooks = None;
    for settings_name in ["settings.json", "settings.local.json"] {
        let settings_file = source_external_agent_dir.join(settings_name);
        if !settings_file.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&settings_file)?;
        let settings: JsonValue = serde_json::from_str(&raw)
            .map_err(|err| invalid_data_error(format!("invalid hooks settings: {err}")))?;
        if let Some(disabled) = settings.get("disableAllHooks").and_then(JsonValue::as_bool) {
            disable_all_hooks = Some(disabled);
        }
        settings_files.push(settings);
    }

    if disable_all_hooks.unwrap_or(false) {
        return Ok(serde_json::Map::new());
    }

    let mut migration = serde_json::Map::new();
    for settings in settings_files {
        append_convertible_hook_groups(&settings, &mut migration, target_config_dir);
    }

    Ok(migration)
}

fn append_convertible_hook_groups(
    settings: &JsonValue,
    hooks_payload: &mut serde_json::Map<String, JsonValue>,
    target_config_dir: Option<&Path>,
) {
    let Some(hooks_config) = settings.get("hooks").and_then(JsonValue::as_object) else {
        return;
    };

    for event_name in HOOK_EVENT_NAMES {
        let Some(groups) = hooks_config.get(event_name).and_then(JsonValue::as_array) else {
            continue;
        };
        for group in groups {
            let Some(group_object) = group.as_object() else {
                continue;
            };
            if group_object.contains_key("if")
                || group_object
                    .keys()
                    .any(|key| !matches!(key.as_str(), "matcher" | "hooks"))
            {
                continue;
            }
            let mut hook_commands = Vec::new();
            if let Some(hooks) = group_object.get("hooks").and_then(JsonValue::as_array) {
                for hook in hooks {
                    let Some(hook_object) = hook.as_object() else {
                        continue;
                    };
                    let hook_type = hook_object
                        .get("type")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("command");
                    if hook_type != "command" {
                        continue;
                    }
                    if hook_object.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "type"
                                | "command"
                                | "timeout"
                                | "timeoutSec"
                                | "statusMessage"
                                | "async"
                        )
                    }) {
                        continue;
                    }
                    if hook_object
                        .get("async")
                        .and_then(JsonValue::as_bool)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if ["asyncRewake", "shell", "once"]
                        .into_iter()
                        .any(|field| hook_object.contains_key(field))
                    {
                        continue;
                    }
                    let Some(command) = hook_object
                        .get("command")
                        .and_then(JsonValue::as_str)
                        .map(str::trim)
                        .filter(|command| !command.is_empty())
                    else {
                        continue;
                    };

                    let mut command_payload = serde_json::Map::new();
                    command_payload
                        .insert("type".to_string(), JsonValue::String("command".to_string()));
                    command_payload.insert(
                        "command".to_string(),
                        JsonValue::String(rewrite_hook_command(command, target_config_dir)),
                    );
                    if let Some(timeout) = hook_object
                        .get("timeout")
                        .or_else(|| hook_object.get("timeoutSec"))
                        .and_then(json_u64)
                    {
                        command_payload.insert(
                            "timeout".to_string(),
                            JsonValue::Number(serde_json::Number::from(timeout)),
                        );
                    }
                    if let Some(status_message) =
                        hook_object.get("statusMessage").and_then(JsonValue::as_str)
                    {
                        command_payload.insert(
                            "statusMessage".to_string(),
                            JsonValue::String(rewrite_external_agent_terms(status_message)),
                        );
                    }
                    hook_commands.push(JsonValue::Object(command_payload));
                }
            }
            if hook_commands.is_empty() {
                continue;
            }

            let mut group_payload = serde_json::Map::new();
            if HOOK_EVENT_NAMES_WITH_MATCHERS.contains(&event_name)
                && let Some(matcher) = group_object.get("matcher").and_then(JsonValue::as_str)
            {
                group_payload.insert(
                    "matcher".to_string(),
                    JsonValue::String(matcher.to_string()),
                );
            }
            group_payload.insert("hooks".to_string(), JsonValue::Array(hook_commands));
            if let Some(groups) = hooks_payload
                .entry(event_name.to_string())
                .or_insert_with(|| JsonValue::Array(Vec::new()))
                .as_array_mut()
            {
                groups.push(JsonValue::Object(group_payload));
            }
        }
    }
}

fn rewrite_hook_command(command: &str, target_config_dir: Option<&Path>) -> String {
    let Some(target_config_dir) = target_config_dir else {
        return command.to_string();
    };
    let target_hooks_dir = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    let hook_dir = shell_quote_path_prefix(&target_hooks_dir);
    let project_dir_env_var = external_agent_project_dir_env_var();
    let source_hooks_path = format!(
        "{}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/",
        external_agent_config_dir()
    );
    let source_hook_prefixes = [
        format!("${{{project_dir_env_var}}}/{source_hooks_path}"),
        format!("${project_dir_env_var}/{source_hooks_path}"),
        format!("./{source_hooks_path}"),
        source_hooks_path.clone(),
    ];
    let command = replace_quoted_hook_path_prefixes(
        command,
        '\'',
        &source_hook_prefixes,
        &hook_dir,
        &target_hooks_dir,
    );
    let command = replace_quoted_hook_path_prefixes(
        &command,
        '"',
        &source_hook_prefixes,
        &hook_dir,
        &target_hooks_dir,
    );
    command
        .replace(
            &format!("\"${project_dir_env_var}\"/{source_hooks_path}"),
            &hook_dir,
        )
        .replace(
            &format!("\"${{{project_dir_env_var}}}\"/{source_hooks_path}"),
            &hook_dir,
        )
        .replace(
            &format!("${{{project_dir_env_var}}}/{source_hooks_path}"),
            &hook_dir,
        )
        .replace(
            &format!("${project_dir_env_var}/{source_hooks_path}"),
            &hook_dir,
        )
        .replace(&format!("./{source_hooks_path}"), &hook_dir)
        .replace(&source_hooks_path, &hook_dir)
}

fn replace_quoted_hook_path_prefixes(
    command: &str,
    quote: char,
    source_hook_prefixes: &[String],
    hook_dir: &str,
    target_hooks_dir: &Path,
) -> String {
    let mut rewritten = command.to_string();
    for source_hook_prefix in source_hook_prefixes {
        let quoted_prefix = format!("{quote}{source_hook_prefix}");
        let mut search_start = 0usize;
        while let Some(relative_start) = rewritten[search_start..].find(&quoted_prefix) {
            let start = search_start + relative_start;
            let path_start = start + quoted_prefix.len();
            if let Some(relative_end) = rewritten[path_start..].find(quote) {
                let end = path_start + relative_end;
                let suffix = rewritten[path_start..end].to_string();
                let replacement =
                    shell_single_quote(target_hooks_dir.join(suffix).to_string_lossy().as_ref());
                rewritten.replace_range(start..end + quote.len_utf8(), &replacement);
                search_start = start + replacement.len();
            } else {
                rewritten.replace_range(start..path_start, hook_dir);
                search_start = start + hook_dir.len();
            }
        }
    }
    rewritten
}

fn shell_quote_path_prefix(path: &Path) -> String {
    format!("{}/", shell_single_quote(path.to_string_lossy().as_ref()))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn copy_hook_scripts(source_external_agent_dir: &Path, target_config_dir: &Path) -> io::Result<()> {
    let source_hooks = source_external_agent_dir.join(EXTERNAL_AGENT_HOOKS_SUBDIR);
    if !source_hooks.is_dir() {
        return Ok(());
    }
    let target_hooks = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    copy_dir_recursive_skip_existing(&source_hooks, &target_hooks)
}

fn copy_dir_recursive_skip_existing(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive_skip_existing(&source_path, &target_path)?;
        } else if file_type.is_file() && !target_path.exists() {
            fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

fn agent_source_files(source_agents: &Path) -> io::Result<Vec<PathBuf>> {
    if !source_agents.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(source_agents)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || path.extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        if path.file_stem().and_then(|stem| stem.to_str()) == Some("README") {
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

fn subagent_target_file(source_file: &Path, target_agents: &Path) -> Option<PathBuf> {
    Some(target_agents.join(format!("{}.toml", source_file.file_stem()?.to_str()?)))
}

fn command_source_files(source_commands: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_markdown_files(source_commands, &mut files)?;
    files.sort();
    Ok(files)
}

fn unique_supported_command_sources(source_commands: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut by_name = BTreeMap::<String, Vec<PathBuf>>::new();
    for source_file in command_source_files(source_commands)? {
        let document = parse_document(&source_file)?;
        let Some(name) = command_skill_name_if_supported(source_commands, &source_file, &document)
        else {
            continue;
        };
        by_name.entry(name).or_default().push(source_file);
    }

    Ok(by_name
        .into_iter()
        .filter_map(|(name, source_files)| {
            let [source_file] = source_files.as_slice() else {
                return None;
            };
            Some((source_file.clone(), name))
        })
        .collect())
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_document(source_file: &Path) -> io::Result<ParsedDocument> {
    let content = fs::read_to_string(source_file)?;
    Ok(parse_document_content(&content))
}

fn parse_document_content(content: &str) -> ParsedDocument {
    let Some(rest) = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };
    let Some((end, body_start)) = frontmatter_end(rest) else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };

    let raw_frontmatter = &rest[..end];
    let body = &rest[body_start..];
    let (frontmatter, frontmatter_error) = parse_frontmatter(raw_frontmatter);
    ParsedDocument {
        frontmatter,
        body: body.to_string(),
        frontmatter_error,
    }
}

fn frontmatter_end(rest: &str) -> Option<(usize, usize)> {
    [
        "\r\n---\r\n",
        "\r\n---\n",
        "\n---\r\n",
        "\n---\n",
        "\r\n---",
        "\n---",
    ]
    .into_iter()
    .filter_map(|delimiter| rest.find(delimiter).map(|end| (end, end + delimiter.len())))
    .min_by_key(|(end, _body_start)| *end)
}

fn parse_frontmatter(
    raw_frontmatter: &str,
) -> (BTreeMap<String, FrontmatterValue>, Option<String>) {
    let parsed: YamlValue = match serde_yaml::from_str(raw_frontmatter) {
        Ok(parsed) => parsed,
        Err(err) => return (BTreeMap::new(), Some(err.to_string())),
    };
    let Some(mapping) = parsed.as_mapping() else {
        return (
            BTreeMap::new(),
            Some("frontmatter is not a YAML mapping".to_string()),
        );
    };

    let mut frontmatter = BTreeMap::new();
    for (key, value) in mapping {
        let Some(key) = key.as_str().map(str::trim).filter(|key| !key.is_empty()) else {
            continue;
        };
        frontmatter.insert(key.to_string(), frontmatter_value_from_yaml(value));
    }

    (frontmatter, None)
}

fn frontmatter_value_from_yaml(value: &YamlValue) -> FrontmatterValue {
    match value {
        YamlValue::String(value) => FrontmatterValue::Scalar(value.trim().to_string()),
        YamlValue::Bool(value) => FrontmatterValue::Scalar(value.to_string()),
        YamlValue::Number(value) => FrontmatterValue::Scalar(value.to_string()),
        YamlValue::Null | YamlValue::Sequence(_) | YamlValue::Mapping(_) | YamlValue::Tagged(_) => {
            FrontmatterValue::Other
        }
    }
}

fn agent_metadata(document: &ParsedDocument) -> Option<AgentMetadata> {
    if document.frontmatter_error.is_some() || document.body.trim().is_empty() {
        return None;
    }
    let name = document
        .frontmatter
        .get("name")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)?;

    let description = document
        .frontmatter
        .get("description")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)?;

    Some(AgentMetadata {
        name,
        description,
        permission_mode: frontmatter_string(&document.frontmatter, "permissionMode"),
        effort: frontmatter_string(&document.frontmatter, "effort"),
    })
}

fn render_agent_toml(body: &str, metadata: &AgentMetadata) -> io::Result<String> {
    let mut document = toml::map::Map::new();
    document.insert("name".to_string(), TomlValue::String(metadata.name.clone()));
    document.insert(
        "description".to_string(),
        TomlValue::String(rewrite_external_agent_terms(&metadata.description)),
    );
    if let Some(effort) = metadata.effort.as_ref()
        && let Some(effort) = map_agent_reasoning_effort(effort)
    {
        document.insert(
            "model_reasoning_effort".to_string(),
            TomlValue::String(effort),
        );
    }
    if let Some(sandbox_mode) = metadata
        .permission_mode
        .as_deref()
        .and_then(map_agent_permission_mode)
    {
        document.insert(
            "sandbox_mode".to_string(),
            TomlValue::String(sandbox_mode.to_string()),
        );
    }
    document.insert(
        "developer_instructions".to_string(),
        TomlValue::String(render_agent_body(body)),
    );

    let serialized = toml::to_string_pretty(&TomlValue::Table(document))
        .map_err(|err| invalid_data_error(format!("failed to serialize agent TOML: {err}")))?;
    Ok(format!("{}\n", serialized.trim_end()))
}

fn render_agent_body(body: &str) -> String {
    let body = rewrite_external_agent_terms(body.trim());
    if body.is_empty() {
        "No subagent instructions were found.".to_string()
    } else {
        body
    }
}

fn command_skill_name(source_commands: &Path, source_file: &Path) -> String {
    slugify_name(&format!(
        "{COMMAND_SKILL_PREFIX}-{}",
        command_source_name(source_commands, source_file)
    ))
}

fn command_skill_name_if_supported(
    source_commands: &Path,
    source_file: &Path,
    document: &ParsedDocument,
) -> Option<String> {
    let name = command_skill_name(source_commands, source_file);
    if name.chars().count() > MAX_SKILL_NAME_LEN {
        return None;
    }
    let source_name = command_source_name(source_commands, source_file);
    let description = command_skill_description(document, &source_name);
    if description.chars().count() > MAX_SKILL_DESCRIPTION_LEN {
        return None;
    }
    if has_unsupported_command_template_features(&document.body) {
        return None;
    }
    Some(name)
}

fn command_skill_description(document: &ParsedDocument, source_name: &str) -> String {
    document
        .frontmatter
        .get("description")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Run the migrated source command `{source_name}`."))
}

fn command_source_name(source_commands: &Path, source_file: &Path) -> String {
    source_file
        .strip_prefix(source_commands)
        .unwrap_or(source_file)
        .with_extension("")
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("-")
}

fn render_command_skill(body: &str, name: &str, description: &str, source_name: &str) -> String {
    let body = rewrite_external_agent_terms(body.trim());
    let template_body = if body.is_empty() {
        "No command template body was found.".to_string()
    } else {
        body
    };
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {name}\n\nUse this skill when the user asks to run the migrated source command `{source_name}`.\n\n## Command Template\n\n{template_body}\n",
        yaml_string(name),
        yaml_string(&rewrite_external_agent_terms(description)),
    )
}

fn has_unsupported_command_template_features(template: &str) -> bool {
    template.contains("$ARGUMENTS")
        || contains_numbered_argument_placeholder(template)
        || (template.contains("{{") && template.contains("}}"))
        || template.contains("!`")
        || template.contains("! `")
        || template
            .split_whitespace()
            .any(|token| token.strip_prefix('@').is_some_and(|rest| !rest.is_empty()))
}

fn contains_numbered_argument_placeholder(template: &str) -> bool {
    let bytes = template.as_bytes();
    bytes
        .windows(2)
        .any(|window| window[0] == b'$' && window[1].is_ascii_digit())
}

fn frontmatter_string(
    frontmatter: &BTreeMap<String, FrontmatterValue>,
    key: &str,
) -> Option<String> {
    frontmatter
        .get(key)
        .and_then(FrontmatterValue::as_scalar)
        .map(ToOwned::to_owned)
}

fn map_agent_reasoning_effort(effort: &str) -> Option<String> {
    let mapped = match effort {
        "max" => "xhigh".to_string(),
        _ => effort.to_string(),
    };
    matches!(
        mapped.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    )
    .then_some(mapped)
}

fn map_agent_permission_mode(permission_mode: &str) -> Option<&'static str> {
    match permission_mode {
        "acceptEdits" => Some("workspace-write"),
        "readOnly" => Some("read-only"),
        _ => None,
    }
}

fn json_string_vec(value: &JsonValue) -> Vec<String> {
    match value {
        JsonValue::Array(values) => values.iter().filter_map(json_string).collect(),
        _ => json_string(value).into_iter().collect(),
    }
}

fn json_string(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::Null => None,
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Bool(value) => Some(value.to_string()),
        JsonValue::Number(value) => Some(value.to_string()),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

fn json_u64(value: &JsonValue) -> Option<u64> {
    if value.is_boolean() || value.is_null() {
        return None;
    }
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn slugify_name(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "migrated".to_string()
    } else {
        slug
    }
}

impl FrontmatterValue {
    fn as_scalar(&self) -> Option<&str> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Other => None,
        }
    }
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

fn rewrite_external_agent_terms(content: &str) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(
        content,
        &external_agent_doc_file_name(),
        "AGENTS.md",
    );
    for from in external_agent_term_variants() {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, &from, "Codex");
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

fn invalid_data_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn external_agent_config_dir() -> String {
    format!(".{SOURCE_EXTERNAL_AGENT_NAME}")
}

fn external_agent_project_config_file() -> String {
    format!(".{SOURCE_EXTERNAL_AGENT_NAME}.json")
}

fn external_agent_project_dir_env_var() -> String {
    format!(
        "{}_PROJECT_DIR",
        SOURCE_EXTERNAL_AGENT_NAME.to_ascii_uppercase()
    )
}

fn external_agent_doc_file_name() -> String {
    format!("{SOURCE_EXTERNAL_AGENT_NAME}.md")
}

fn external_agent_term_variants() -> [String; 5] {
    [
        format!("{SOURCE_EXTERNAL_AGENT_NAME} code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}-code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}_code"),
        format!("{SOURCE_EXTERNAL_AGENT_NAME}code"),
        SOURCE_EXTERNAL_AGENT_NAME.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn source_path(relative_path: &str) -> PathBuf {
        Path::new("/repo")
            .join(external_agent_config_dir())
            .join(relative_path)
    }

    fn source_hook_command(script_name: &str) -> String {
        format!(
            "python3 {}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/{script_name}",
            external_agent_config_dir()
        )
    }

    fn source_hook_command_with_project_dir(script_name: &str) -> String {
        format!(
            "python3 \"${}\"/{}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/{script_name}",
            external_agent_project_dir_env_var(),
            external_agent_config_dir()
        )
    }

    fn migrated_hook_command(script_name: &str) -> String {
        let hook_dir = shell_quote_path_prefix(
            &Path::new("/repo/.codex").join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR),
        );
        format!("python3 {hook_dir}{script_name}")
    }

    fn migrated_quoted_hook_command(script_name: &str) -> String {
        let hook_path = Path::new("/repo/.codex")
            .join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR)
            .join(script_name);
        format!(
            "python3 {}",
            shell_single_quote(hook_path.to_string_lossy().as_ref())
        )
    }

    #[test]
    fn env_placeholder_accepts_defaults() {
        assert_eq!(
            parse_env_placeholder("${TOKEN:-fallback}"),
            Some("TOKEN".to_string())
        );
    }

    #[test]
    fn mcp_migration_skips_placeholder_args() {
        let root = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            root.path().join(".mcp.json"),
            r#"{"mcpServers":{"db":{"command":"db-server","args":["${DATABASE_URL}"]}}}"#,
        )
        .expect("write mcp");

        assert_eq!(
            build_mcp_config_from_external(
                root.path(),
                /*external_agent_home*/ None,
                /*settings*/ None,
            )
            .unwrap(),
            TomlValue::Table(Default::default())
        );
    }

    #[test]
    fn mcp_migration_skips_unsupported_transports() {
        let root = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            root.path().join(".mcp.json"),
            r#"{
              "mcpServers": {
                "legacy-sse": {"type": "sse", "url": "https://example.invalid/sse"},
                "vault": {
                  "url": "https://example.invalid/vault",
                  "headers": {"Authorization": "Bearer ${VAULT_TOKEN:-dev-token}"}
                }
              }
            }"#,
        )
        .expect("write mcp");

        assert_eq!(
            build_mcp_config_from_external(
                root.path(),
                /*external_agent_home*/ None,
                /*settings*/ None,
            )
            .unwrap(),
            toml::from_str(
                r#"
[mcp_servers.vault]
url = "https://example.invalid/vault"
bearer_token_env_var = "VAULT_TOKEN"
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn mcp_migration_reads_matching_project_entries_from_repo_external_project_config() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let project = root.path().join("repo");
        fs::create_dir_all(&project).expect("create repo");
        let other = root.path().join("other");
        fs::create_dir_all(&other).expect("create other");
        fs::write(
            project.join(external_agent_project_config_file()),
            serde_json::json!({
                "mcpServers": {
                    "top": {"command": "top-server"}
                },
                "projects": {
                    project.display().to_string(): {
                        "mcpServers": {
                            "repo": {"command": "repo-server"}
                        }
                    },
                    other.display().to_string(): {
                        "mcpServers": {
                            "other": {"command": "other-server"}
                        }
                    }
                }
            })
            .to_string(),
        )
        .expect("write external agent project config");

        assert_eq!(
            build_mcp_config_from_external(
                &project, /*external_agent_home*/ None, /*settings*/ None,
            )
            .unwrap(),
            toml::from_str(
                r#"
[mcp_servers.repo]
command = "repo-server"

[mcp_servers.top]
command = "top-server"
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn mcp_migration_reads_matching_project_entries_from_home_external_project_config() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let project = root.path().join("repo");
        fs::create_dir_all(&project).expect("create repo");
        let external_agent_home = root.path().join(external_agent_config_dir());
        fs::create_dir_all(&external_agent_home).expect("create external agent home");
        fs::write(
            root.path().join(external_agent_project_config_file()),
            serde_json::json!({
                "projects": {
                    project.display().to_string(): {
                        "mcpServers": {
                            "repo": {"command": "repo-server"}
                        }
                    }
                }
            })
            .to_string(),
        )
        .expect("write external agent project config");

        assert_eq!(
            build_mcp_config_from_external(
                &project,
                Some(&external_agent_home),
                /*settings*/ None,
            )
            .unwrap(),
            toml::from_str(
                r#"
[mcp_servers.repo]
command = "repo-server"
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn mcp_migration_preserves_repo_servers_over_home_project_entries() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let project = root.path().join("repo");
        fs::create_dir_all(&project).expect("create repo");
        let external_agent_home = root.path().join(external_agent_config_dir());
        fs::create_dir_all(&external_agent_home).expect("create external agent home");
        fs::write(
            project.join(EXTERNAL_AGENT_MCP_CONFIG_FILE),
            serde_json::json!({
                "mcpServers": {
                    "shared": {"command": "repo-server"}
                }
            })
            .to_string(),
        )
        .expect("write repo mcp");
        fs::write(
            root.path().join(external_agent_project_config_file()),
            serde_json::json!({
                "projects": {
                    project.display().to_string(): {
                        "mcpServers": {
                            "home-only": {"command": "home-only-server"},
                            "shared": {"command": "home-server"}
                        }
                    }
                }
            })
            .to_string(),
        )
        .expect("write external agent project config");

        assert_eq!(
            build_mcp_config_from_external(
                &project,
                Some(&external_agent_home),
                /*settings*/ None,
            )
            .unwrap(),
            toml::from_str(
                r#"
[mcp_servers.home-only]
command = "home-only-server"

[mcp_servers.shared]
command = "repo-server"
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn mcp_migration_skips_disabled_servers() {
        let root = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            root.path().join(".mcp.json"),
            r#"{
              "mcpServers": {
                "enabled": {"command": "enabled-server"},
                "explicit-disabled": {"command": "disabled-server", "disabled": true},
                "not-enabled": {"command": "not-enabled-server"}
              }
            }"#,
        )
        .expect("write mcp");
        let settings = serde_json::json!({
            "enabledMcpjsonServers": ["enabled"],
            "disabledMcpjsonServers": ["explicit-disabled"]
        });

        assert_eq!(
            build_mcp_config_from_external(
                root.path(),
                /*external_agent_home*/ None,
                Some(&settings),
            )
            .unwrap(),
            toml::from_str(
                r#"
[mcp_servers.enabled]
command = "enabled-server"
"#
            )
            .unwrap()
        );
    }

    #[test]
    fn command_skill_names_include_nested_paths() {
        let root = source_path("commands");
        let file = source_path("commands/pr/review.md");

        assert_eq!(command_skill_name(&root, &file), "source-command-pr-review");
    }

    #[test]
    fn command_skill_names_must_fit_codex_skill_loader_limit() {
        let root = source_path("commands");
        let file = source_path("commands/this/is/a/deeply/nested/command/with/a/very/long/name.md");
        let document = parse_document_content("---\ndescription: Review PR\n---\nReview\n");

        assert!(command_skill_name_if_supported(&root, &file, &document).is_none());
    }

    #[test]
    fn commands_with_provider_runtime_expansion_are_skipped() {
        let root = source_path("commands");
        let file = source_path("commands/deploy.md");
        let document = parse_document_content(
            "---\ndescription: Deploy\n---\nDeploy $ARGUMENTS from @release.yaml\n",
        );

        assert!(command_skill_name_if_supported(&root, &file, &document).is_none());
    }

    #[test]
    fn command_slug_collisions_are_skipped() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let commands = root.path().join("commands");
        fs::create_dir_all(&commands).expect("create commands");
        fs::write(
            commands.join("foo-bar.md"),
            "---\ndescription: First\n---\nRun the first command.\n",
        )
        .expect("write first command");
        fs::write(
            commands.join("foo_bar.md"),
            "---\ndescription: Second\n---\nRun the second command.\n",
        )
        .expect("write second command");

        assert_eq!(
            unique_supported_command_sources(&commands).unwrap(),
            Vec::<(PathBuf, String)>::new()
        );
    }

    #[test]
    fn subagent_accepts_yaml_block_lists_by_ignoring_unsupported_fields() {
        let document = parse_document_content(
            "---\nname: cloud-incident\ndescription: Debug incidents\nskills:\n  - runbook-reader\ntools:\n  - Read\n  - Bash\ndisallowedTools:\n  - Write\n---\nInvestigate carefully.\n",
        );

        assert!(agent_metadata(&document).is_some());
    }

    #[test]
    fn subagent_requires_minimum_codex_agent_fields() {
        let missing_description =
            parse_document_content("---\nname: incomplete\n---\nInvestigate carefully.\n");
        let missing_body =
            parse_document_content("---\nname: incomplete\ndescription: Missing body\n---\n");

        assert!(agent_metadata(&missing_description).is_none());
        assert!(agent_metadata(&missing_body).is_none());
    }

    #[test]
    fn subagent_preserves_default_model_when_source_model_is_present() {
        let document = parse_document_content(
            "---\nname: reviewer\ndescription: Review code\nmodel: source-opus\neffort: max\n---\nReview carefully.\n",
        );
        let metadata = agent_metadata(&document).expect("metadata");
        let rendered: TomlValue =
            toml::from_str(&render_agent_toml(&document.body, &metadata).expect("render agent"))
                .expect("parse rendered agent");
        let expected: TomlValue = toml::from_str(
            r#"
name = "reviewer"
description = "Review code"
model_reasoning_effort = "xhigh"
developer_instructions = """
Review carefully."""
"#,
        )
        .expect("parse expected agent");

        assert_eq!(rendered, expected);
    }

    #[test]
    fn subagent_target_preserves_dotted_file_stem() {
        let target_agents = Path::new("/repo/.codex/agents");
        let source_file = source_path("agents/security.audit.md");

        assert_eq!(
            subagent_target_file(&source_file, target_agents),
            Some(PathBuf::from("/repo/.codex/agents/security.audit.toml"))
        );
    }

    #[test]
    fn frontmatter_accepts_crlf_delimiters() {
        let document = parse_document_content(
            "---\r\nname: reviewer\r\ndescription: Review code\r\n---\r\nReview carefully.\r\n",
        );

        assert_eq!(
            (
                document
                    .frontmatter
                    .get("name")
                    .and_then(FrontmatterValue::as_scalar),
                document
                    .frontmatter
                    .get("description")
                    .and_then(FrontmatterValue::as_scalar),
                document.body.as_str(),
            ),
            (
                Some("reviewer"),
                Some("Review code"),
                "Review carefully.\r\n"
            )
        );
    }

    #[test]
    fn hook_migration_ignores_unsupported_handlers() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "if": "tool_input.command contains 'rm'",
                    "hooks": [{
                        "type": "command",
                        "command": source_hook_command("policy_gate.py")
                    }]
                }, {
                    "matcher": "Edit",
                    "hooks": [
                        {
                            "type": "command",
                            "if": "Bash(rm *)",
                            "command": source_hook_command("policy_gate.py")
                        },
                        {
                            "type": "http",
                            "url": "https://example.invalid/hook"
                        }
                    ]
                }],
                "PermissionRequest": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": source_hook_command("approve.py")
                    }]
                }],
                "SubagentStart": [{
                    "matcher": "worker",
                    "hooks": [{"type": "prompt", "prompt": "check"}]
                }]
            }
        });
        let mut migration = serde_json::Map::new();
        append_convertible_hook_groups(&settings, &mut migration, Some(Path::new("/repo/.codex")));

        assert_eq!(
            migration,
            serde_json::json!({
                "PermissionRequest": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": migrated_hook_command("approve.py")
                    }]
                }]
            })
            .as_object()
            .cloned()
            .expect("object")
        );
    }

    #[test]
    fn hook_migration_honors_disable_all_hooks() {
        let root = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            root.path().join("settings.json"),
            r#"{
              "disableAllHooks": true,
              "hooks": {
                "SessionStart": [{
                  "matcher": "startup",
                  "hooks": [{"type": "command", "command": "echo setup"}]
                }]
              }
            }"#,
        )
        .expect("write settings");

        assert_eq!(
            hook_migration(root.path(), /*target_config_dir*/ None).unwrap(),
            serde_json::Map::new()
        );
    }

    #[test]
    fn hook_migration_honors_settings_local_disable_override() {
        let root = tempfile::TempDir::new().expect("tempdir");
        fs::write(
            root.path().join("settings.json"),
            r#"{
              "disableAllHooks": true,
              "hooks": {
                "SessionStart": [{
                  "matcher": "project",
                  "hooks": [{"type": "command", "command": "echo project"}]
                }]
              }
            }"#,
        )
        .expect("write project settings");
        fs::write(
            root.path().join("settings.local.json"),
            r#"{
              "disableAllHooks": false,
              "hooks": {
                "SessionStart": [{
                  "matcher": "local",
                  "hooks": [{"type": "command", "command": "echo local"}]
                }]
              }
            }"#,
        )
        .expect("write local settings");

        assert_eq!(
            hook_migration(root.path(), /*target_config_dir*/ None).unwrap(),
            serde_json::json!({
                "SessionStart": [{
                    "matcher": "project",
                    "hooks": [{
                        "type": "command",
                        "command": "echo project"
                    }]
                }, {
                    "matcher": "local",
                    "hooks": [{
                        "type": "command",
                        "command": "echo local"
                    }]
                }]
            })
            .as_object()
            .cloned()
            .expect("object")
        );
    }

    #[test]
    fn hook_command_paths_rewrite_to_target_hook_dir() {
        let project_dir_env_var = external_agent_project_dir_env_var();
        let source_hooks_path = format!(
            "{}/{EXTERNAL_AGENT_HOOKS_SUBDIR}",
            external_agent_config_dir()
        );
        assert_eq!(
            rewrite_hook_command(
                &source_hook_command_with_project_dir("check.py"),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_hook_command("check.py")
        );
        assert_eq!(
            rewrite_hook_command(
                &source_hook_command("check.py"),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_hook_command("check.py")
        );
        assert_eq!(
            rewrite_hook_command(
                &format!("python3 ./{source_hooks_path}/check.py"),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_hook_command("check.py")
        );
        assert_eq!(
            rewrite_hook_command(
                &format!("python3 '${{{project_dir_env_var}}}/{source_hooks_path}/check.py'"),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_quoted_hook_command("check.py")
        );
        assert_eq!(
            rewrite_hook_command(
                &format!("python3 \"${{{project_dir_env_var}}}/{source_hooks_path}/check.py\""),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_quoted_hook_command("check.py")
        );
        assert_eq!(
            rewrite_hook_command(
                &format!("python3 '${{{project_dir_env_var}}}/{source_hooks_path}/my script.py'"),
                Some(Path::new("/repo/.codex")),
            ),
            migrated_quoted_hook_command("my script.py")
        );
    }

    #[test]
    fn hook_script_copy_keeps_existing_target_scripts() {
        let root = tempfile::TempDir::new().expect("tempdir");
        let source_external_agent_dir = root.path().join(external_agent_config_dir());
        let source_hooks = source_external_agent_dir.join(EXTERNAL_AGENT_HOOKS_SUBDIR);
        let target_config_dir = root.path().join(".codex");
        let target_hooks = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
        fs::create_dir_all(&source_hooks).expect("create source hooks");
        fs::create_dir_all(&target_hooks).expect("create target hooks");
        fs::write(source_hooks.join("check.py"), "new script").expect("write source hook");
        fs::write(target_hooks.join("check.py"), "existing script").expect("write target hook");

        copy_hook_scripts(&source_external_agent_dir, &target_config_dir).expect("copy hooks");

        assert_eq!(
            fs::read_to_string(target_hooks.join("check.py")).expect("read target hook"),
            "existing script"
        );
    }

    #[test]
    fn hook_migration_drops_negative_timeouts() {
        let settings = serde_json::json!({
            "hooks": {
                "SessionStart": [{
                    "matcher": "startup",
                    "hooks": [{
                        "type": "command",
                        "command": "echo setup",
                        "timeout": -1
                    }]
                }]
            }
        });
        let mut migration = serde_json::Map::new();
        append_convertible_hook_groups(&settings, &mut migration, /*target_config_dir*/ None);

        assert_eq!(
            migration,
            serde_json::json!({
                "SessionStart": [{
                    "matcher": "startup",
                    "hooks": [{
                        "type": "command",
                        "command": "echo setup"
                    }]
                }]
            })
            .as_object()
            .cloned()
            .expect("object")
        );
    }
}
