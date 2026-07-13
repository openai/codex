//! Migration helpers for importing external-agent configuration into Codex.

mod hooks_cla;
pub mod sessions;

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

pub use hooks_cla::hook_migration_event_names_cla;
pub use hooks_cla::hooks_migration_description_cla;
pub use hooks_cla::import_hooks_cla;

const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
const EXTERNAL_AGENT_MCP_CONFIG_FILE: &str = ".mcp.json";
const EXTERNAL_AGENT_HOOKS_SUBDIR: &str = "hooks";
const EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR: &str = "hooks";
const COMMAND_SKILL_PREFIX: &str = "source-command";
const MAX_SKILL_NAME_LEN: usize = 64;

/// Describes source-specific terms that should be rewritten in migrated artifacts.
#[derive(Clone, Copy)]
pub struct RewriteProfile {
    doc_file_name: &'static str,
    term_variants: &'static [&'static str],
    case_sensitive_term_variants: &'static [&'static str],
}

impl RewriteProfile {
    pub const fn new(doc_file_name: &'static str, term_variants: &'static [&'static str]) -> Self {
        Self {
            doc_file_name,
            term_variants,
            case_sensitive_term_variants: &[],
        }
    }

    pub const fn with_case_sensitive_term_variants(
        mut self,
        term_variants: &'static [&'static str],
    ) -> Self {
        self.case_sensitive_term_variants = term_variants;
        self
    }

    pub const fn doc_file_name(self) -> &'static str {
        self.doc_file_name
    }

    pub const fn term_variants(self) -> &'static [&'static str] {
        self.term_variants
    }

    pub const fn case_sensitive_term_variants(self) -> &'static [&'static str] {
        self.case_sensitive_term_variants
    }
}

/// Controls how migrated commands obtain the description required by a Codex skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandDescriptionMode {
    /// Skip source commands that do not declare a non-empty frontmatter description.
    RequireFrontmatter,
    /// Derive a stable description from the source command name when frontmatter is absent.
    UseSourceNameFallback,
}

/// Describes source-specific command migration behavior.
#[derive(Clone, Copy)]
pub struct CommandMigrationProfile {
    rewrite_profile: RewriteProfile,
    description_mode: CommandDescriptionMode,
}

impl CommandMigrationProfile {
    pub const fn new(
        rewrite_profile: RewriteProfile,
        description_mode: CommandDescriptionMode,
    ) -> Self {
        Self {
            rewrite_profile,
            description_mode,
        }
    }
}

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
    build_mcp_config(mcp_servers, settings)
}

pub fn build_mcp_config_from_json_file(source_file: &Path) -> io::Result<TomlValue> {
    if !source_file.is_file() {
        return Ok(TomlValue::Table(Default::default()));
    }
    let raw = fs::read_to_string(source_file)?;
    let parsed: JsonValue = serde_json::from_str(&raw)
        .map_err(|err| invalid_data_error(format!("invalid MCP config: {err}")))?;
    let mut mcp_servers = BTreeMap::new();
    append_mcp_servers_from_value(&parsed, &mut mcp_servers, McpServerMerge::Overwrite);
    build_mcp_config(mcp_servers, /*settings*/ None)
}

fn build_mcp_config(
    mcp_servers: BTreeMap<String, JsonValue>,
    settings: Option<&JsonValue>,
) -> io::Result<TomlValue> {
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

pub fn import_subagents_with_rewrite_profile(
    source_agents: &Path,
    target_agents: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<Vec<String>> {
    if !source_agents.is_dir() {
        return Ok(Vec::new());
    }

    fs::create_dir_all(target_agents)?;
    let mut imported = Vec::new();
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
        fs::write(
            &target,
            render_agent_toml(&document.body, &metadata, rewrite_profile)?,
        )?;
        imported.push(metadata.name);
    }

    Ok(imported)
}

pub fn count_missing_commands_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<usize> {
    Ok(missing_command_names_with_profile(source_commands, target_skills, profile)?.len())
}

pub fn missing_command_names_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<Vec<String>> {
    Ok(
        unique_supported_command_sources(source_commands, profile.description_mode)?
            .into_iter()
            .filter(|(_source_file, name)| !target_skills.join(name).exists())
            .map(|(_source_file, name)| name)
            .collect(),
    )
}

pub fn import_commands_with_profile(
    source_commands: &Path,
    target_skills: &Path,
    profile: CommandMigrationProfile,
) -> io::Result<Vec<String>> {
    if !source_commands.is_dir() {
        return Ok(Vec::new());
    }

    fs::create_dir_all(target_skills)?;
    let mut imported = Vec::new();
    for (source_file, name) in
        unique_supported_command_sources(source_commands, profile.description_mode)?
    {
        let document = parse_document(&source_file)?;
        let target_dir = target_skills.join(&name);
        if target_dir.exists() {
            continue;
        }
        fs::create_dir_all(&target_dir)?;
        let source_name = command_source_name(source_commands, &source_file);
        let Some(description) =
            command_skill_description(&document, &source_name, profile.description_mode)
        else {
            continue;
        };
        fs::write(
            target_dir.join("SKILL.md"),
            render_command_skill(
                &document.body,
                &name,
                &description,
                &source_name,
                profile.rewrite_profile,
            ),
        )?;
        imported.push(name);
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

fn write_hook_migration(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
    migration: serde_json::Map<String, JsonValue>,
) -> io::Result<bool> {
    if migration.is_empty() || !is_missing_or_empty_text_file(target_hooks)? {
        return Ok(false);
    }
    let Some(parent) = target_hooks.parent() else {
        return Err(invalid_data_error("hooks target path has no parent"));
    };
    fs::create_dir_all(parent)?;
    copy_hook_scripts(source_external_agent_dir, parent)?;
    let mut payload = serde_json::Map::new();
    payload.insert("hooks".to_string(), JsonValue::Object(migration));
    let rendered = serde_json::to_string_pretty(&JsonValue::Object(payload))
        .map_err(|err| invalid_data_error(format!("failed to serialize hooks.json: {err}")))?;
    fs::write(target_hooks, format!("{rendered}\n"))?;
    Ok(true)
}

fn rewrite_hook_command_for_source(
    command: &str,
    target_config_dir: Option<&Path>,
    source_external_agent_dir: &Path,
) -> String {
    let Some(target_config_dir) = target_config_dir else {
        return command.to_string();
    };
    if looks_like_windows_hook_command(command) {
        return command.to_string();
    }
    let target_hooks_dir = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    let source_config_dir = source_external_agent_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(external_agent_config_dir);
    let source_hooks_path = format!("{source_config_dir}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/");
    let command = replace_quoted_hook_paths(command, '\'', &source_hooks_path, &target_hooks_dir);
    let command = replace_quoted_hook_paths(&command, '"', &source_hooks_path, &target_hooks_dir);
    replace_unquoted_hook_paths(&command, &source_hooks_path, &target_hooks_dir)
}

fn replace_quoted_hook_paths(
    command: &str,
    quote: char,
    source_hooks_path: &str,
    target_hooks_dir: &Path,
) -> String {
    let mut rewritten = command.to_string();
    let mut search_start = 0usize;
    while let Some(relative_start) = rewritten[search_start..].find(quote) {
        let start = search_start + relative_start;
        let content_start = start + quote.len_utf8();
        let Some(relative_end) = rewritten[content_start..].find(quote) else {
            break;
        };
        let end = content_start + relative_end;
        let content = &rewritten[content_start..end];
        if let Some(source_hooks_start) = content.find(source_hooks_path) {
            let suffix_start = source_hooks_start + source_hooks_path.len();
            let suffix = &content[suffix_start..];
            let Some(replacement) =
                target_hook_path_replacement(target_hooks_dir, content, source_hooks_start, suffix)
            else {
                search_start = end + quote.len_utf8();
                continue;
            };
            rewritten.replace_range(start..end + quote.len_utf8(), &replacement);
            search_start = start + replacement.len();
        } else {
            search_start = end + quote.len_utf8();
        }
    }
    rewritten
}

fn replace_unquoted_hook_paths(
    command: &str,
    source_hooks_path: &str,
    target_hooks_dir: &Path,
) -> String {
    let mut rewritten = command.to_string();
    let mut search_start = 0usize;
    while let Some(source_hooks_start) =
        find_unquoted_source_hook_path(&rewritten, source_hooks_path, search_start)
    {
        let path_start = shell_path_start(&rewritten, source_hooks_start);
        let path_end = shell_path_end(&rewritten, source_hooks_start + source_hooks_path.len());
        if is_assignment_value_start(&rewritten, path_start) {
            search_start = source_hooks_start + source_hooks_path.len();
            continue;
        }
        let path = rewritten[path_start..path_end].to_string();
        let suffix = rewritten[source_hooks_start + source_hooks_path.len()..path_end].to_string();
        if let Some(replacement) = target_hook_path_replacement(
            target_hooks_dir,
            &path,
            source_hooks_start - path_start,
            &suffix,
        ) {
            rewritten.replace_range(path_start..path_end, &replacement);
            search_start = path_start + replacement.len();
        } else {
            search_start = source_hooks_start + source_hooks_path.len();
        }
    }
    rewritten
}

fn find_unquoted_source_hook_path(
    command: &str,
    source_hooks_path: &str,
    start: usize,
) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;
    for (offset, ch) in command[start..].char_indices() {
        let index = start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if !in_single_quote && ch == '\\' {
            escaped = true;
            continue;
        }
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            _ if !in_single_quote
                && !in_double_quote
                && command[index..].starts_with(source_hooks_path) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn is_pure_shell_path_content(content: &str, source_hooks_start: usize) -> bool {
    let prefix = &content[..source_hooks_start];
    (prefix.is_empty() || prefix == "./" || prefix.ends_with('/'))
        && !prefix.chars().any(is_shell_path_boundary)
}

fn shell_path_start(command: &str, end: usize) -> usize {
    command[..end]
        .char_indices()
        .filter_map(|(index, ch)| is_shell_path_boundary(ch).then_some(index + ch.len_utf8()))
        .next_back()
        .unwrap_or(0)
}

fn shell_path_end(command: &str, start: usize) -> usize {
    let mut escaped = false;
    for (offset, ch) in command[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if is_shell_path_boundary(ch) {
            return start + offset;
        }
    }
    command.len()
}

fn is_shell_path_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '=' | ';' | '|' | '&' | '<' | '>' | '(' | ')')
}

fn is_assignment_value_start(command: &str, path_start: usize) -> bool {
    command[..path_start]
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '=')
}

fn target_hook_path_replacement(
    target_hooks_dir: &Path,
    path: &str,
    source_hooks_start: usize,
    suffix: &str,
) -> Option<String> {
    if !is_pure_shell_path_content(path, source_hooks_start) || !is_static_hook_path_suffix(suffix)
    {
        return None;
    }
    Some(shell_single_quote(
        target_hooks_dir.join(suffix).to_string_lossy().as_ref(),
    ))
}

fn is_static_hook_path_suffix(suffix: &str) -> bool {
    !suffix.is_empty()
        && !suffix
            .chars()
            .any(|ch| matches!(ch, '\\' | '$' | '`' | '*' | '?' | '[' | '{' | '}'))
}

fn looks_like_windows_hook_command(command: &str) -> bool {
    let source_hooks_backslash_path = format!(
        r"{}\{EXTERNAL_AGENT_HOOKS_SUBDIR}\",
        external_agent_config_dir()
    );
    let project_dir_env_var = external_agent_project_dir_env_var();
    command.contains(&source_hooks_backslash_path)
        || command.contains(&format!("%{project_dir_env_var}%"))
        || command.contains(&format!("$env:{project_dir_env_var}"))
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

fn unique_supported_command_sources(
    source_commands: &Path,
    description_mode: CommandDescriptionMode,
) -> io::Result<Vec<(PathBuf, String)>> {
    let mut by_name = BTreeMap::<String, Vec<PathBuf>>::new();
    for source_file in command_source_files(source_commands)? {
        let document = parse_document(&source_file)?;
        let Some(name) = command_skill_name_if_supported(
            source_commands,
            &source_file,
            &document,
            description_mode,
        ) else {
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

fn render_agent_toml(
    body: &str,
    metadata: &AgentMetadata,
    rewrite_profile: RewriteProfile,
) -> io::Result<String> {
    let mut document = toml::map::Map::new();
    document.insert("name".to_string(), TomlValue::String(metadata.name.clone()));
    document.insert(
        "description".to_string(),
        TomlValue::String(rewrite_external_agent_terms(
            &metadata.description,
            rewrite_profile,
        )),
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
        TomlValue::String(render_agent_body(body, rewrite_profile)),
    );

    let serialized = toml::to_string_pretty(&TomlValue::Table(document))
        .map_err(|err| invalid_data_error(format!("failed to serialize agent TOML: {err}")))?;
    Ok(format!("{}\n", serialized.trim_end()))
}

fn render_agent_body(body: &str, rewrite_profile: RewriteProfile) -> String {
    let body = rewrite_external_agent_terms(body.trim(), rewrite_profile);
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
    description_mode: CommandDescriptionMode,
) -> Option<String> {
    if source_file.file_stem().and_then(|stem| stem.to_str()) == Some("README") {
        return None;
    }
    let source_name = command_source_name(source_commands, source_file);
    command_skill_description(document, &source_name, description_mode)?;
    let name = command_skill_name(source_commands, source_file);
    if name.chars().count() > MAX_SKILL_NAME_LEN {
        return None;
    }
    if has_unsupported_command_template_features(&document.body) {
        return None;
    }
    Some(name)
}

fn command_skill_description(
    document: &ParsedDocument,
    source_name: &str,
    description_mode: CommandDescriptionMode,
) -> Option<String> {
    let frontmatter_description = document
        .frontmatter
        .get("description")
        .and_then(FrontmatterValue::as_scalar)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    frontmatter_description.or_else(|| match description_mode {
        CommandDescriptionMode::RequireFrontmatter => None,
        CommandDescriptionMode::UseSourceNameFallback => {
            Some(format!("Migrated source command `{source_name}`"))
        }
    })
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

fn render_command_skill(
    body: &str,
    name: &str,
    description: &str,
    source_name: &str,
    rewrite_profile: RewriteProfile,
) -> String {
    let body = rewrite_external_agent_terms(body.trim(), rewrite_profile);
    let template_body = if body.is_empty() {
        "No command template body was found.".to_string()
    } else {
        body
    };
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {name}\n\nUse this skill when the user asks to run the migrated source command `{source_name}`.\n\n## Command Template\n\n{template_body}\n",
        yaml_string(name),
        yaml_string(&rewrite_external_agent_terms(description, rewrite_profile)),
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

fn rewrite_external_agent_terms(content: &str, rewrite_profile: RewriteProfile) -> String {
    let mut rewritten = replace_case_insensitive_with_boundaries(
        content,
        rewrite_profile.doc_file_name,
        "AGENTS.md",
    );
    for from in rewrite_profile.term_variants {
        rewritten = replace_case_insensitive_with_boundaries(&rewritten, from, "Codex");
    }
    for from in rewrite_profile.case_sensitive_term_variants {
        rewritten = replace_with_boundaries(&rewritten, from, "Codex");
    }
    rewritten
}

fn replace_with_boundaries(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }

    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut last_emitted = 0usize;
    let mut search_start = 0usize;

    while let Some(relative_pos) = input[search_start..].find(needle) {
        let start = search_start + relative_pos;
        let end = start + needle.len();
        let boundary_before = start == 0 || !is_word_byte(bytes[start - 1]);
        let boundary_after = end == bytes.len() || !is_word_byte(bytes[end]);

        if boundary_before && boundary_after {
            output.push_str(&input[last_emitted..start]);
            output.push_str(replacement);
            last_emitted = end;
        }

        search_start = end;
    }

    if last_emitted == 0 {
        return input.to_string();
    }

    output.push_str(&input[last_emitted..]);
    output
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

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
