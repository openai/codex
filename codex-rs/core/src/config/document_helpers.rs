use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use toml_edit::Array as TomlArray;
use toml_edit::InlineTable;
use toml_edit::Item as TomlItem;
use toml_edit::Table as TomlTable;
use toml_edit::value;

pub(super) fn ensure_table_for_write(item: &mut TomlItem) -> Option<&mut TomlTable> {
    match item {
        TomlItem::Table(table) => Some(table),
        TomlItem::Value(value) => {
            if let Some(inline) = value.as_inline_table() {
                *item = TomlItem::Table(table_from_inline(inline));
                item.as_table_mut()
            } else {
                *item = TomlItem::Table(new_implicit_table());
                item.as_table_mut()
            }
        }
        TomlItem::None => {
            *item = TomlItem::Table(new_implicit_table());
            item.as_table_mut()
        }
        _ => None,
    }
}

pub(super) fn ensure_table_for_read(item: &mut TomlItem) -> Option<&mut TomlTable> {
    match item {
        TomlItem::Table(table) => Some(table),
        TomlItem::Value(value) => {
            let inline = value.as_inline_table()?;
            *item = TomlItem::Table(table_from_inline(inline));
            item.as_table_mut()
        }
        _ => None,
    }
}

fn serialize_mcp_server_table(config: &McpServerConfig) -> TomlTable {
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
                entry["args"] = array_from_iter(args.iter().cloned());
            }
            if let Some(env) = env
                && !env.is_empty()
            {
                entry["env"] = table_from_pairs(env.iter());
            }
            if !env_vars.is_empty() {
                entry["env_vars"] = array_from_iter(env_vars.iter().cloned());
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
    if let Some(timeout) = config.startup_timeout_sec {
        entry["startup_timeout_sec"] = value(timeout.as_secs_f64());
    }
    if let Some(timeout) = config.tool_timeout_sec {
        entry["tool_timeout_sec"] = value(timeout.as_secs_f64());
    }
    if let Some(enabled_tools) = &config.enabled_tools
        && !enabled_tools.is_empty()
    {
        entry["enabled_tools"] = array_from_iter(enabled_tools.iter().cloned());
    }
    if let Some(disabled_tools) = &config.disabled_tools
        && !disabled_tools.is_empty()
    {
        entry["disabled_tools"] = array_from_iter(disabled_tools.iter().cloned());
    }

    entry
}

pub(super) fn serialize_mcp_server(config: &McpServerConfig) -> TomlItem {
    TomlItem::Table(serialize_mcp_server_table(config))
}

pub(super) fn serialize_mcp_server_inline(config: &McpServerConfig) -> InlineTable {
    serialize_mcp_server_table(config).into_inline_table()
}

pub(super) fn merge_inline_table(existing: &mut InlineTable, replacement: InlineTable) {
    existing.retain(|key, _| replacement.get(key).is_some());

    for (key, value) in replacement.iter() {
        if let Some(existing_value) = existing.get_mut(key) {
            let mut updated_value = value.clone();
            *updated_value.decor_mut() = existing_value.decor().clone();
            *existing_value = updated_value;
        } else {
            existing.insert(key.to_string(), value.clone());
        }
    }
}

fn table_from_inline(inline: &InlineTable) -> TomlTable {
    let mut table = new_implicit_table();
    for (key, value) in inline.iter() {
        let mut value = value.clone();
        let decor = value.decor_mut();
        decor.set_suffix("");
        table.insert(key, TomlItem::Value(value));
    }
    table
}

pub(super) fn new_implicit_table() -> TomlTable {
    let mut table = TomlTable::new();
    table.set_implicit(true);
    table
}

fn array_from_iter<I>(iter: I) -> TomlItem
where
    I: Iterator<Item = String>,
{
    let mut array = TomlArray::new();
    for value in iter {
        array.push(value);
    }
    TomlItem::Value(array.into())
}

fn table_from_pairs<'a, I>(pairs: I) -> TomlItem
where
    I: IntoIterator<Item = (&'a String, &'a String)>,
{
    let mut entries: Vec<_> = pairs.into_iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut table = TomlTable::new();
    table.set_implicit(false);
    for (key, val) in entries {
        table.insert(key, value(val.clone()));
    }
    TomlItem::Table(table)
}
