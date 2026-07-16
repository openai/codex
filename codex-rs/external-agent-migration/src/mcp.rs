use crate::invalid_data_error;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

pub(super) const EXTERNAL_AGENT_MCP_CONFIG_FILE: &str = ".mcp.json";
const EXTERNAL_AGENT_PROJECT_CONFIG_FILE: &str = ".claude.json";

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

fn read_external_mcp_servers(
    source_root: &Path,
    external_agent_home: Option<&Path>,
) -> io::Result<BTreeMap<String, JsonValue>> {
    let mut servers = BTreeMap::new();
    let project_config_file = external_agent_project_config_file();
    for relative_path in [
        EXTERNAL_AGENT_MCP_CONFIG_FILE.to_string(),
        project_config_file.to_string(),
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

pub(crate) fn parse_env_placeholder(value: &str) -> Option<String> {
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

pub(crate) fn external_agent_project_config_file() -> &'static str {
    EXTERNAL_AGENT_PROJECT_CONFIG_FILE
}
