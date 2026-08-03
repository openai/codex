use super::PluginMcpConfigParseOutcome;
use super::PluginMcpServerParseError;
use codex_config::McpServerConfig;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use url::Host;

// Published Agent Plugins v1 MCP schema:
// https://github.com/agentplugins/agent-plugins-spec/blob/main/schemas/1.0.0/mcp.schema.json
const AGENT_PLUGIN_MCP_SCHEMA_URI: &str = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";
const SUPPORTED_AGENT_PLUGIN_MCP_SCHEMA_URIS: &[&str] = &[AGENT_PLUGIN_MCP_SCHEMA_URI];
const PLUGIN_ROOT_VARIABLE: &str = "PLUGIN_ROOT";
const PLUGIN_DATA_VARIABLE: &str = "PLUGIN_DATA";
const CLIENT_OWNED_HTTP_HEADERS: &[&str] = &[
    "accept",
    "authorization",
    "connection",
    "content-encoding",
    "content-length",
    "content-type",
    "host",
    "last-event-id",
    "mcp-protocol-version",
    "mcp-session-id",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "user-agent",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentPluginMcpFile {
    #[serde(rename = "$schema")]
    schema: String,
    mcp_servers: BTreeMap<String, JsonValue>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum AgentPluginMcpServer {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
        cwd: Option<String>,
    },
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        url: String,
        headers: Option<BTreeMap<String, String>>,
    },
    #[serde(rename = "sse")]
    Sse {
        #[serde(rename = "url")]
        _url: String,
        #[serde(rename = "headers")]
        _headers: Option<BTreeMap<String, String>>,
    },
}

/// Translates an Agent Plugins `mcp.json` into Codex MCP configuration.
pub fn parse_agent_plugin_mcp_config(
    plugin_root: &Path,
    plugin_data_root: &Path,
    contents: &str,
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    parse_agent_plugin_mcp_config_from(contents, plugin_root, plugin_data_root)
}

fn parse_agent_plugin_mcp_config_from(
    contents: &str,
    plugin_root: &Path,
    plugin_data_root: &Path,
) -> Result<PluginMcpConfigParseOutcome, serde_json::Error> {
    let AgentPluginMcpFile {
        schema,
        mcp_servers,
    } = serde_json::from_str(contents)?;
    if !SUPPORTED_AGENT_PLUGIN_MCP_SCHEMA_URIS.contains(&schema.as_str()) {
        return Err(plugin_mcp_json_error(format!(
            "unsupported Agent Plugins MCP schema `{schema}`; supported schemas: {}",
            SUPPORTED_AGENT_PLUGIN_MCP_SCHEMA_URIS.join(", ")
        )));
    }

    let mut outcome = PluginMcpConfigParseOutcome::default();
    for (name, value) in mcp_servers {
        match normalize_agent_plugin_mcp_server(value, plugin_root, plugin_data_root) {
            Ok(config) => {
                outcome.servers.insert(name, config);
            }
            Err(message) => outcome
                .errors
                .push(PluginMcpServerParseError { name, message }),
        }
    }
    Ok(outcome)
}

fn normalize_agent_plugin_mcp_server(
    value: JsonValue,
    plugin_root: &Path,
    plugin_data_root: &Path,
) -> Result<McpServerConfig, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Agent Plugins MCP server must be an object".to_string())?;
    match object.get("type").and_then(JsonValue::as_str) {
        Some("stdio") => reject_explicit_null(object, "cwd")?,
        Some("streamable-http" | "sse") => reject_explicit_null(object, "headers")?,
        _ => {}
    }
    let server =
        serde_json::from_value::<AgentPluginMcpServer>(value).map_err(|err| err.to_string())?;
    let object = match server {
        AgentPluginMcpServer::Stdio {
            command,
            args,
            env,
            cwd,
        } => normalize_agent_plugin_stdio_server(
            command,
            args,
            env,
            cwd,
            plugin_root,
            plugin_data_root,
        )?,
        AgentPluginMcpServer::StreamableHttp { url, headers } => {
            normalize_agent_plugin_http_server(url, headers)?
        }
        AgentPluginMcpServer::Sse { .. } => {
            return Err("Agent Plugins legacy SSE transport is not supported by Codex".to_string());
        }
    };
    serde_json::from_value(JsonValue::Object(object)).map_err(|err| err.to_string())
}

fn normalize_agent_plugin_stdio_server(
    mut command: String,
    mut args: Vec<String>,
    mut env: BTreeMap<String, String>,
    cwd: Option<String>,
    plugin_root: &Path,
    plugin_data_root: &Path,
) -> Result<JsonMap<String, JsonValue>, String> {
    #[cfg(windows)]
    let has_windows_path_prefix = matches!(
        Path::new(&command).components().next(),
        Some(std::path::Component::Prefix(_))
    );
    #[cfg(not(windows))]
    let has_windows_path_prefix = false;
    let is_bare_command = !command.is_empty()
        && !command.contains('/')
        && !command.contains('\\')
        && !has_windows_path_prefix;
    let is_plugin_relative_command =
        command.starts_with("./") && is_portable_relative_path(&command);
    if !is_bare_command && !is_plugin_relative_command {
        return Err(
            "Agent Plugins stdio command must be a bare executable name or a contained `./` path"
                .to_string(),
        );
    }
    for reserved in [PLUGIN_ROOT_VARIABLE, PLUGIN_DATA_VARIABLE] {
        if env
            .keys()
            .any(|name| environment_variable_names_match(name, reserved))
        {
            return Err(format!(
                "Agent Plugins stdio `env` cannot override reserved variable `{reserved}`"
            ));
        }
    }
    #[cfg(windows)]
    {
        let mut normalized_env = BTreeMap::new();
        for (name, value) in env {
            let normalized_name = name.to_ascii_uppercase();
            if normalized_env.insert(normalized_name, value).is_some() {
                return Err(format!(
                    "duplicate case-insensitive Agent Plugins environment variable `{name}`"
                ));
            }
        }
        env = normalized_env;
    }

    let root_path = absolute_plugin_path(plugin_root)?;
    let data_root_path = absolute_plugin_path(plugin_data_root)?;
    let root = host_path_string(&root_path);
    let data_root = host_path_string(&data_root_path);
    if command.starts_with("./") {
        command = host_path_string(&resolve_contained_host_path(
            &command, &root_path, &root_path,
        )?);
    }
    for arg in &mut args {
        *arg = expand_agent_plugin_placeholders(arg, &root, &data_root);
    }
    for value in env.values_mut() {
        *value = expand_agent_plugin_placeholders(value, &root, &data_root);
    }

    let cwd = cwd.as_deref().unwrap_or("${PLUGIN_ROOT}");
    let Some(cwd_root) = parse_agent_plugin_cwd(cwd) else {
        return Err(
            "Agent Plugins stdio `cwd` must be a contained `./`, `${PLUGIN_ROOT}`, or `${PLUGIN_DATA}` path"
                .to_string(),
        );
    };
    let cwd = expand_agent_plugin_placeholders(cwd, &root, &data_root);
    let cwd_root = match cwd_root {
        AgentPluginCwdRoot::Package => &root_path,
        AgentPluginCwdRoot::Data => &data_root_path,
    };
    env.insert(PLUGIN_ROOT_VARIABLE.to_string(), root);
    env.insert(PLUGIN_DATA_VARIABLE.to_string(), data_root);

    Ok(JsonMap::from_iter([
        ("command".to_string(), JsonValue::String(command)),
        (
            "args".to_string(),
            JsonValue::Array(args.into_iter().map(JsonValue::String).collect()),
        ),
        ("env".to_string(), string_map_value(env)),
        (
            "cwd".to_string(),
            JsonValue::String(host_path_string(&resolve_contained_host_path(
                &cwd, cwd_root, cwd_root,
            )?)),
        ),
    ]))
}

fn reject_explicit_null(object: &JsonMap<String, JsonValue>, field: &str) -> Result<(), String> {
    if object.get(field).is_some_and(JsonValue::is_null) {
        return Err(format!(
            "Agent Plugins MCP `{field}` must use its declared type when present"
        ));
    }
    Ok(())
}

fn environment_variable_names_match(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn normalize_agent_plugin_http_server(
    url: String,
    mut headers: Option<BTreeMap<String, String>>,
) -> Result<JsonMap<String, JsonValue>, String> {
    validate_agent_plugin_url(&url)?;
    if let Some(configured_headers) = headers.as_mut() {
        validate_agent_plugin_headers(configured_headers)?;
        configured_headers.retain(|name, _| {
            !CLIENT_OWNED_HTTP_HEADERS
                .iter()
                .any(|owned| name.eq_ignore_ascii_case(owned))
        });
    }
    let mut object = JsonMap::from_iter([("url".to_string(), JsonValue::String(url))]);
    if let Some(headers) = headers.filter(|headers| !headers.is_empty()) {
        object.insert("http_headers".to_string(), string_map_value(headers));
    }
    Ok(object)
}

fn validate_agent_plugin_url(raw_url: &str) -> Result<(), String> {
    if raw_url.is_empty() {
        return Err("Agent Plugins HTTP server requires a non-empty `url`".to_string());
    }
    let parsed = url::Url::parse(raw_url)
        .map_err(|err| format!("invalid Agent Plugins MCP URL `{raw_url}`: {err}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Agent Plugins MCP URL must be absolute HTTP or HTTPS".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(
            "Agent Plugins MCP URL must not contain user information or a fragment".to_string(),
        );
    }
    let is_loopback = match parsed.host() {
        Some(Host::Domain(host)) => host == "localhost",
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if parsed.scheme() == "http" && !is_loopback {
        return Err("non-loopback Agent Plugins MCP endpoints must use HTTPS".to_string());
    }
    Ok(())
}

fn validate_agent_plugin_headers(headers: &BTreeMap<String, String>) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for (name, value) in headers {
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(format!(
                "duplicate case-insensitive Agent Plugins HTTP header `{name}`"
            ));
        }
        if !is_valid_http_header_name(name) {
            return Err(format!("invalid Agent Plugins HTTP header name `{name}`"));
        }
        if value
            .bytes()
            .any(|byte| (byte < 32 && byte != b'\t') || byte == 127)
        {
            return Err(format!(
                "invalid Agent Plugins HTTP header value for `{name}`"
            ));
        }
    }
    Ok(())
}

fn string_map_value(values: BTreeMap<String, String>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(name, value)| (name, JsonValue::String(value)))
            .collect(),
    )
}

#[derive(Clone, Copy, Debug)]
enum AgentPluginCwdRoot {
    Package,
    Data,
}

fn parse_agent_plugin_cwd(value: &str) -> Option<AgentPluginCwdRoot> {
    if value == "./" {
        return Some(AgentPluginCwdRoot::Package);
    }
    if let Some(relative) = value.strip_prefix("./")
        && is_portable_path_suffix(relative)
    {
        return Some(AgentPluginCwdRoot::Package);
    }
    for (placeholder, root) in [
        ("${PLUGIN_ROOT}", AgentPluginCwdRoot::Package),
        ("${PLUGIN_DATA}", AgentPluginCwdRoot::Data),
    ] {
        if value == placeholder {
            return Some(root);
        }
        if let Some(relative) = value.strip_prefix(&format!("{placeholder}/"))
            && (relative.is_empty() || is_portable_path_suffix(relative))
        {
            return Some(root);
        }
    }
    None
}

fn expand_agent_plugin_placeholders(value: &str, plugin_root: &str, plugin_data: &str) -> String {
    const ROOT: &str = "${PLUGIN_ROOT}";
    const DATA: &str = "${PLUGIN_DATA}";
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    loop {
        let next = match (remaining.find(ROOT), remaining.find(DATA)) {
            (Some(root), Some(data)) if root <= data => Some((root, ROOT, plugin_root)),
            (Some(_), Some(data)) => Some((data, DATA, plugin_data)),
            (Some(root), None) => Some((root, ROOT, plugin_root)),
            (None, Some(data)) => Some((data, DATA, plugin_data)),
            (None, None) => None,
        };
        let Some((index, placeholder, replacement)) = next else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..index]);
        output.push_str(replacement);
        remaining = &remaining[index + placeholder.len()..];
    }
    output
}

fn absolute_plugin_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|err| format!("failed to resolve plugin path: {err}"))
    }?;
    resolve_existing_path_prefix(&absolute)
}

fn resolve_contained_host_path(
    value: &str,
    root: &Path,
    allowed_root: &Path,
) -> Result<PathBuf, String> {
    let value = Path::new(value);
    let path = if value.is_absolute() {
        value.to_path_buf()
    } else {
        root.join(value)
    };
    let path = resolve_existing_path_prefix(&path)?;
    if !path.starts_with(allowed_root) {
        return Err(format!(
            "expanded path `{}` must remain within `{}`",
            value.display(),
            allowed_root.display()
        ));
    }
    Ok(path)
}

fn resolve_existing_path_prefix(path: &Path) -> Result<PathBuf, String> {
    let mut existing = path.to_path_buf();
    let mut missing_components = Vec::<OsString>::new();
    loop {
        match std::fs::canonicalize(&existing) {
            Ok(mut resolved) => {
                for component in missing_components.iter().rev() {
                    resolved.push(component);
                }
                return Ok(lexical_normalize(&resolved));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if std::fs::symlink_metadata(&existing)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(format!(
                        "failed to resolve symlinked path `{}`",
                        path.display()
                    ));
                }
                let Some(component) = existing.components().next_back() else {
                    return Err(format!(
                        "failed to resolve path `{}`: {err}",
                        path.display()
                    ));
                };
                if matches!(
                    component,
                    std::path::Component::Prefix(_) | std::path::Component::RootDir
                ) {
                    return Err(format!(
                        "failed to resolve path `{}`: {err}",
                        path.display()
                    ));
                }
                missing_components.push(component.as_os_str().to_os_string());
                if !existing.pop() {
                    return Err(format!(
                        "failed to resolve path `{}`: {err}",
                        path.display()
                    ));
                }
            }
            Err(err) => {
                return Err(format!(
                    "failed to resolve path `{}`: {err}",
                    path.display()
                ));
            }
        }
    }
}

fn host_path_string(path: &Path) -> String {
    let rendered = path.to_string_lossy();
    #[cfg(windows)]
    if let Some(path) = rendered.strip_prefix(r"\\?\") {
        return path
            .strip_prefix(r"UNC\")
            .map(|path| format!(r"\\{path}"))
            .unwrap_or_else(|| path.to_string());
    }
    rendered.into_owned()
}

fn is_portable_relative_path(value: &str) -> bool {
    value
        .strip_prefix("./")
        .is_some_and(is_portable_path_suffix)
}

fn is_portable_path_suffix(value: &str) -> bool {
    !value.is_empty() && !value.contains('\\')
}

fn is_valid_http_header_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn plugin_mcp_json_error(message: impl Into<String>) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.into(),
    ))
}
