use super::invalid_data_error;
use super::is_missing_or_empty_text_file;
use super::rewrite_external_agent_terms;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

const CODEX_HOOK_EVENTS: [&str; 6] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
];
const CODEX_HOOK_MATCHER_EVENTS: [&str; 4] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "SessionStart",
];
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
    model: Option<String>,
    permission_mode: Option<String>,
    effort: Option<String>,
}

#[derive(Debug, Default)]
struct HookMigration {
    hooks_payload: serde_json::Map<String, JsonValue>,
}

impl HookMigration {
    fn has_active_hooks(&self) -> bool {
        !self.hooks_payload.is_empty()
    }
}

pub(super) fn build_mcp_config_from_external(
    source_root: &Path,
    settings: Option<&JsonValue>,
) -> io::Result<TomlValue> {
    let mcp_servers = read_external_mcp_servers(source_root)?;
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

pub(super) fn hooks_migration_description(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
) -> io::Result<Option<String>> {
    let migration = hook_migration(source_external_agent_dir)?;
    if !migration.has_active_hooks() {
        return Ok(None);
    }

    Ok(Some(format!(
        "Migrate hooks from {} to {}",
        source_external_agent_dir.display(),
        target_hooks.display()
    )))
}

pub(super) fn build_hooks_feature_config() -> TomlValue {
    let mut features = toml::map::Map::new();
    features.insert("codex_hooks".to_string(), TomlValue::Boolean(true));

    let mut root = toml::map::Map::new();
    root.insert("features".to_string(), TomlValue::Table(features));
    TomlValue::Table(root)
}

pub(super) fn import_hooks(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
) -> io::Result<bool> {
    let migration = hook_migration(source_external_agent_dir)?;
    if !migration.has_active_hooks() {
        return Ok(false);
    }

    let Some(parent) = target_hooks.parent() else {
        return Err(invalid_data_error("hooks target path has no parent"));
    };
    fs::create_dir_all(parent)?;

    let mut wrote_active_hooks = false;
    if !migration.hooks_payload.is_empty() && is_missing_or_empty_text_file(target_hooks)? {
        copy_hook_scripts(source_external_agent_dir, parent)?;
        let mut payload = serde_json::Map::new();
        payload.insert(
            "hooks".to_string(),
            JsonValue::Object(migration.hooks_payload),
        );
        let rendered = serde_json::to_string_pretty(&JsonValue::Object(payload))
            .map_err(|err| invalid_data_error(format!("failed to serialize hooks.json: {err}")))?;
        fs::write(target_hooks, format!("{rendered}\n"))?;
        wrote_active_hooks = true;
    }

    Ok(wrote_active_hooks)
}

pub(super) fn count_missing_subagents(
    source_agents: &Path,
    target_agents: &Path,
) -> io::Result<usize> {
    let mut count = 0usize;
    for source_file in agent_source_files(source_agents)? {
        let document = parse_document(&source_file)?;
        if agent_metadata(&document).is_none() {
            continue;
        }
        let Some(stem) = source_file.file_stem() else {
            continue;
        };
        if !target_agents.join(stem).with_extension("toml").exists() {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn import_subagents(source_agents: &Path, target_agents: &Path) -> io::Result<usize> {
    if !source_agents.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(target_agents)?;
    let mut imported = 0usize;
    for source_file in agent_source_files(source_agents)? {
        let Some(stem) = source_file.file_stem() else {
            continue;
        };
        let target = target_agents.join(stem).with_extension("toml");
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

pub(super) fn count_missing_commands(
    source_commands: &Path,
    target_skills: &Path,
) -> io::Result<usize> {
    let mut count = 0usize;
    for source_file in command_source_files(source_commands)? {
        let document = parse_document(&source_file)?;
        let Some(name) = command_skill_name_if_supported(source_commands, &source_file, &document)
        else {
            continue;
        };
        if !target_skills.join(name).exists() {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn import_commands(source_commands: &Path, target_skills: &Path) -> io::Result<usize> {
    if !source_commands.is_dir() {
        return Ok(0);
    }

    fs::create_dir_all(target_skills)?;
    let mut imported = 0usize;
    for source_file in command_source_files(source_commands)? {
        let document = parse_document(&source_file)?;
        let Some(name) = command_skill_name_if_supported(source_commands, &source_file, &document)
        else {
            continue;
        };
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

fn read_external_mcp_servers(source_root: &Path) -> io::Result<BTreeMap<String, JsonValue>> {
    let mut servers = BTreeMap::new();
    for relative_path in [".mcp.json", ".claude.json"] {
        let source_file = source_root.join(relative_path);
        if !source_file.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&source_file)?;
        let parsed: JsonValue = serde_json::from_str(&raw)
            .map_err(|err| invalid_data_error(format!("invalid MCP config: {err}")))?;
        let Some(mcp_servers) = parsed.get("mcpServers").and_then(JsonValue::as_object) else {
            continue;
        };
        for (server_name, server_config) in mcp_servers {
            servers.insert(server_name.clone(), server_config.clone());
        }
    }

    Ok(servers)
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

    let disabled_by_server = server_config
        .get("enabled")
        .and_then(JsonValue::as_bool)
        .is_some_and(|enabled| !enabled)
        || server_config
            .get("disabled")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
    if disabled_by_server
        || (!enabled_servers.is_empty() && !enabled_servers.iter().any(|name| name == server_name))
        || disabled_servers.contains(server_name)
    {
        table.insert("enabled".to_string(), TomlValue::Boolean(false));
    }

    Some(table)
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
    if inner.contains(":-") {
        return None;
    }
    let name = inner;
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

fn hook_migration(source_external_agent_dir: &Path) -> io::Result<HookMigration> {
    let mut migration = HookMigration::default();
    for settings_name in ["settings.json", "settings.local.json"] {
        let settings_file = source_external_agent_dir.join(settings_name);
        if !settings_file.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&settings_file)?;
        let settings: JsonValue = serde_json::from_str(&raw)
            .map_err(|err| invalid_data_error(format!("invalid hooks settings: {err}")))?;
        append_convertible_hook_groups(&settings, &mut migration);
    }

    Ok(migration)
}

fn append_convertible_hook_groups(settings: &JsonValue, migration: &mut HookMigration) {
    let Some(hooks_config) = settings.get("hooks").and_then(JsonValue::as_object) else {
        return;
    };

    for event_name in CODEX_HOOK_EVENTS {
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
                        JsonValue::String(rewrite_hook_command(command)),
                    );
                    if let Some(timeout) = hook_object
                        .get("timeout")
                        .or_else(|| hook_object.get("timeoutSec"))
                        .and_then(json_i64)
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
            if CODEX_HOOK_MATCHER_EVENTS.contains(&event_name)
                && let Some(matcher) = group_object.get("matcher").and_then(JsonValue::as_str)
            {
                group_payload.insert(
                    "matcher".to_string(),
                    JsonValue::String(matcher.to_string()),
                );
            }
            group_payload.insert("hooks".to_string(), JsonValue::Array(hook_commands));
            if let Some(groups) = migration
                .hooks_payload
                .entry(event_name.to_string())
                .or_insert_with(|| JsonValue::Array(Vec::new()))
                .as_array_mut()
            {
                groups.push(JsonValue::Object(group_payload));
            }
        }
    }
}

fn rewrite_hook_command(command: &str) -> String {
    command
        .replace("\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/", ".codex/hooks/")
        .replace("${CLAUDE_PROJECT_DIR}/.claude/hooks/", ".codex/hooks/")
        .replace("$CLAUDE_PROJECT_DIR/.claude/hooks/", ".codex/hooks/")
        .replace(".claude/hooks/", ".codex/hooks/")
}

fn copy_hook_scripts(source_external_agent_dir: &Path, target_config_dir: &Path) -> io::Result<()> {
    let source_hooks = source_external_agent_dir.join("hooks");
    if !source_hooks.is_dir() {
        return Ok(());
    }
    let target_hooks = target_config_dir.join("hooks");
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

fn command_source_files(source_commands: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_markdown_files(source_commands, &mut files)?;
    files.sort();
    Ok(files)
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
    let Some(rest) = content.strip_prefix("---\n") else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };
    let Some(end) = rest.find("\n---") else {
        return ParsedDocument {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
            frontmatter_error: None,
        };
    };

    let raw_frontmatter = &rest[..end];
    let body_start = end + "\n---".len();
    let body = rest[body_start..]
        .strip_prefix('\n')
        .unwrap_or(&rest[body_start..]);
    let (frontmatter, frontmatter_error) = parse_frontmatter(raw_frontmatter);
    ParsedDocument {
        frontmatter,
        body: body.to_string(),
        frontmatter_error,
    }
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
        model: frontmatter_string(&document.frontmatter, "model"),
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
    if let Some(model) = metadata.model.as_ref() {
        document.insert(
            "model".to_string(),
            TomlValue::String(map_model_name(model)),
        );
    }
    if let Some(effort) = metadata.effort.as_ref()
        && let Some(effort) = map_model_effort(metadata.model.as_deref(), effort)
    {
        document.insert(
            "model_reasoning_effort".to_string(),
            TomlValue::String(effort),
        );
    }
    if let Some(sandbox_mode) = metadata
        .permission_mode
        .as_deref()
        .and_then(map_permission_mode)
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

fn map_model_name(model: &str) -> String {
    if model.starts_with("claude-opus") {
        "gpt-5.4".to_string()
    } else if model.starts_with("claude-sonnet") || model.starts_with("claude-haiku") {
        "gpt-5.4-mini".to_string()
    } else {
        rewrite_external_agent_terms(model)
    }
}

fn map_model_effort(model: Option<&str>, effort: &str) -> Option<String> {
    let mapped = match (model, effort) {
        (Some(model), "max")
            if model.starts_with("claude-opus")
                || model.starts_with("claude-sonnet")
                || model.starts_with("claude-haiku") =>
        {
            "xhigh".to_string()
        }
        (Some(model), "medium") if model.starts_with("claude-sonnet") => "high".to_string(),
        (Some(model), "high") if model.starts_with("claude-sonnet") => "xhigh".to_string(),
        _ => effort.to_string(),
    };
    matches!(
        mapped.as_str(),
        "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
    )
    .then_some(mapped)
}

fn map_permission_mode(permission_mode: &str) -> Option<&'static str> {
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

fn json_i64(value: &JsonValue) -> Option<i64> {
    if value.is_boolean() || value.is_null() {
        return None;
    }
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn env_placeholder_rejects_defaults() {
        assert_eq!(parse_env_placeholder("${TOKEN:-fallback}"), None);
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
            build_mcp_config_from_external(root.path(), /*settings*/ None).unwrap(),
            TomlValue::Table(Default::default())
        );
    }

    #[test]
    fn mcp_migration_skips_unsupported_transports_and_placeholders() {
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
            build_mcp_config_from_external(root.path(), /*settings*/ None).unwrap(),
            TomlValue::Table(Default::default())
        );
    }

    #[test]
    fn command_skill_names_include_nested_paths() {
        let root = Path::new("/repo/.claude/commands");
        let file = Path::new("/repo/.claude/commands/pr/review.md");

        assert_eq!(command_skill_name(root, file), "source-command-pr-review");
    }

    #[test]
    fn command_skill_names_must_fit_codex_skill_loader_limit() {
        let root = Path::new("/repo/.claude/commands");
        let file = Path::new(
            "/repo/.claude/commands/this/is/a/deeply/nested/command/with/a/very/long/name.md",
        );
        let document = parse_document_content("---\ndescription: Review PR\n---\nReview\n");

        assert!(command_skill_name_if_supported(root, file, &document).is_none());
    }

    #[test]
    fn commands_with_provider_runtime_expansion_are_skipped() {
        let root = Path::new("/repo/.claude/commands");
        let file = Path::new("/repo/.claude/commands/deploy.md");
        let document = parse_document_content(
            "---\ndescription: Deploy\n---\nDeploy $ARGUMENTS from @release.yaml\n",
        );

        assert!(command_skill_name_if_supported(root, file, &document).is_none());
    }

    #[test]
    fn hook_feature_merges_into_existing_config() {
        let mut existing: TomlValue = toml::from_str("[features]\nother = true\n").unwrap();
        let incoming = build_hooks_feature_config();

        assert!(super::super::merge_missing_toml_values(&mut existing, &incoming).unwrap());
        assert_eq!(
            existing,
            toml::from_str("[features]\nother = true\ncodex_hooks = true\n").unwrap()
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
    fn hook_migration_ignores_unsupported_handlers() {
        let settings = serde_json::json!({
            "disableAllHooks": true,
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "if": "tool_input.command contains 'rm'",
                    "hooks": [{
                        "type": "command",
                        "command": "python3 .claude/hooks/policy_gate.py"
                    }]
                }, {
                    "matcher": "Edit",
                    "hooks": [
                        {
                            "type": "command",
                            "if": "Bash(rm *)",
                            "command": "python3 .claude/hooks/policy_gate.py"
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
                        "command": "python3 .claude/hooks/approve.py"
                    }]
                }],
                "SubagentStart": [{
                    "matcher": "worker",
                    "hooks": [{"type": "prompt", "prompt": "check"}]
                }]
            }
        });
        let mut migration = HookMigration::default();
        append_convertible_hook_groups(&settings, &mut migration);

        assert_eq!(
            migration.hooks_payload,
            serde_json::json!({
                "PermissionRequest": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "python3 .codex/hooks/approve.py"
                    }]
                }]
            })
            .as_object()
            .cloned()
            .expect("object")
        );
    }
}
