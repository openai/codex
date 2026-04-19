use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use tokio::task;
use toml::Value as TomlValue;
use toml_edit::DocumentMut;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

use crate::AppToolApproval;
use crate::CONFIG_TOML_FILE;
use crate::McpServerConfig;
use crate::McpServerEnvVar;
use crate::McpServerTransportConfig;

pub async fn load_global_mcp_servers(
    codex_home: &Path,
) -> std::io::Result<BTreeMap<String, McpServerConfig>> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let raw = match tokio::fs::read_to_string(&config_path).await {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(err),
    };
    let parsed = toml::from_str::<TomlValue>(&raw)
        .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err))?;
    let Some(servers_value) = parsed.get("mcp_servers") else {
        return Ok(BTreeMap::new());
    };

    ensure_no_inline_bearer_tokens(servers_value)?;

    servers_value
        .clone()
        .try_into()
        .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err))
}

fn ensure_no_inline_bearer_tokens(value: &TomlValue) -> std::io::Result<()> {
    let Some(servers_table) = value.as_table() else {
        return Ok(());
    };

    for (server_name, server_value) in servers_table {
        if let Some(server_table) = server_value.as_table()
            && server_table.contains_key("bearer_token")
        {
            let message = format!(
                "mcp_servers.{server_name} uses unsupported `bearer_token`; set `bearer_token_env_var`."
            );
            return Err(std::io::Error::new(ErrorKind::InvalidData, message));
        }
    }

    Ok(())
}

pub struct ConfigEditsBuilder {
    codex_home: PathBuf,
    mcp_servers: Option<BTreeMap<String, McpServerConfig>>,
}

impl ConfigEditsBuilder {
    pub fn new(codex_home: &Path) -> Self {
        Self {
            codex_home: codex_home.to_path_buf(),
            mcp_servers: None,
        }
    }

    pub fn replace_mcp_servers(mut self, servers: &BTreeMap<String, McpServerConfig>) -> Self {
        self.mcp_servers = Some(servers.clone());
        self
    }

    pub async fn apply(self) -> std::io::Result<()> {
        task::spawn_blocking(move || self.apply_blocking())
            .await
            .map_err(|err| {
                std::io::Error::other(format!("config persistence task panicked: {err}"))
            })?
    }

    fn apply_blocking(self) -> std::io::Result<()> {
        let config_path = self.codex_home.join(CONFIG_TOML_FILE);
        let mut doc = read_or_create_document(&config_path)?;
        if let Some(servers) = self.mcp_servers.as_ref() {
            replace_mcp_servers(&mut doc, servers);
        }
        fs::create_dir_all(&self.codex_home)?;
        fs::write(config_path, doc.to_string())
    }
}

fn read_or_create_document(config_path: &Path) -> std::io::Result<DocumentMut> {
    match fs::read_to_string(config_path) {
        Ok(raw) => raw
            .parse::<DocumentMut>()
            .map_err(|err| std::io::Error::new(ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err),
    }
}

fn replace_mcp_servers(doc: &mut DocumentMut, servers: &BTreeMap<String, McpServerConfig>) {
    let root = doc.as_table_mut();
    if servers.is_empty() {
        root.remove("mcp_servers");
        return;
    }

    let mut table = TomlTable::new();
    table.set_implicit(true);
    for (name, config) in servers {
        table.insert(name, serialize_mcp_server(config));
    }
    root.insert("mcp_servers", TomlItem::Table(table));
}

fn serialize_mcp_server(config: &McpServerConfig) -> TomlItem {
    let mut entry = TomlTable::new();
    entry.set_implicit(false);

    match &config.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            entry["command"] = value(command.clone());
            if !args.is_empty() {
                entry["args"] = array_from_strings(args);
            }
            if let Some(env) = env
                && !env.is_empty()
            {
                entry["env"] = table_from_pairs(env.iter());
            }
            if !env_vars.is_empty() {
                entry["env_vars"] = array_from_env_vars(env_vars);
            }
            if let Some(cwd) = cwd {
                entry["cwd"] = value(cwd.to_string_lossy().to_string());
            }
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            entry["url"] = value(url.clone());
            if let Some(env_var) = bearer_token_env_var {
                entry["bearer_token_env_var"] = value(env_var.clone());
            }
            if let Some(headers) = http_headers
                && !headers.is_empty()
            {
                entry["http_headers"] = table_from_pairs(headers.iter());
            }
            if let Some(headers) = env_http_headers
                && !headers.is_empty()
            {
                entry["env_http_headers"] = table_from_pairs(headers.iter());
            }
        }
    }

    if !config.enabled {
        entry["enabled"] = value(false);
    }
    if let Some(environment) = &config.experimental_environment {
        entry["experimental_environment"] = value(environment.clone());
    }
    if config.required {
        entry["required"] = value(true);
    }
    if config.supports_parallel_tool_calls {
        entry["supports_parallel_tool_calls"] = value(true);
    }
    if let Some(timeout) = config.startup_timeout_sec {
        entry["startup_timeout_sec"] = value(timeout.as_secs_f64());
    }
    if let Some(timeout) = config.tool_timeout_sec {
        entry["tool_timeout_sec"] = value(timeout.as_secs_f64());
    }
    if let Some(approval_mode) = config.default_tools_approval_mode {
        entry["default_tools_approval_mode"] = value(match approval_mode {
            AppToolApproval::Auto => "auto",
            AppToolApproval::Prompt => "prompt",
            AppToolApproval::Approve => "approve",
        });
    }
    if let Some(enabled_tools) = &config.enabled_tools
        && !enabled_tools.is_empty()
    {
        entry["enabled_tools"] = array_from_strings(enabled_tools);
    }
    if let Some(disabled_tools) = &config.disabled_tools
        && !disabled_tools.is_empty()
    {
        entry["disabled_tools"] = array_from_strings(disabled_tools);
    }
    if let Some(scopes) = &config.scopes
        && !scopes.is_empty()
    {
        entry["scopes"] = array_from_strings(scopes);
    }
    if let Some(resource) = &config.oauth_resource
        && !resource.is_empty()
    {
        entry["oauth_resource"] = value(resource.clone());
    }
    if !config.tools.is_empty() {
        let mut tools = TomlTable::new();
        tools.set_implicit(false);
        let mut tool_entries: Vec<_> = config.tools.iter().collect();
        tool_entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (name, tool_config) in tool_entries {
            let mut tool_entry = TomlTable::new();
            tool_entry.set_implicit(false);
            if let Some(approval_mode) = tool_config.approval_mode {
                tool_entry["approval_mode"] = value(match approval_mode {
                    AppToolApproval::Auto => "auto",
                    AppToolApproval::Prompt => "prompt",
                    AppToolApproval::Approve => "approve",
                });
            }
            tools.insert(name, TomlItem::Table(tool_entry));
        }
        entry.insert("tools", TomlItem::Table(tools));
    }

    TomlItem::Table(entry)
}

fn array_from_strings(values: &[String]) -> TomlItem {
    let mut array = toml_edit::Array::new();
    for value in values {
        array.push(value.clone());
    }
    TomlItem::Value(array.into())
}

fn array_from_env_vars(env_vars: &[McpServerEnvVar]) -> TomlItem {
    let mut array = toml_edit::Array::new();
    for env_var in env_vars {
        match env_var {
            McpServerEnvVar::Name(name) => array.push(name.clone()),
            McpServerEnvVar::Config { name, source } => {
                let mut table = toml_edit::InlineTable::new();
                table.insert("name", name.clone().into());
                if let Some(source) = source {
                    table.insert("source", source.clone().into());
                }
                array.push(table);
            }
        }
    }
    TomlItem::Value(array.into())
}

fn table_from_pairs<'a, I>(pairs: I) -> TomlItem
where
    I: IntoIterator<Item = (&'a String, &'a String)>,
{
    let mut entries: Vec<_> = pairs.into_iter().collect();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut table = TomlTable::new();
    table.set_implicit(false);
    for (key, value_str) in entries {
        table.insert(key, value(value_str.clone()));
    }
    TomlItem::Table(table)
}

#[cfg(test)]
#[path = "mcp_edit_tests.rs"]
mod tests;
