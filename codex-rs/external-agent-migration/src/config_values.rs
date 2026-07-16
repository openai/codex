use std::fs;
use std::io;
use std::path::Path;
use toml::Value as TomlValue;

use crate::utils::invalid_data_error;

pub(super) fn merge_missing_toml_values(
    existing: &mut TomlValue,
    incoming: &TomlValue,
) -> io::Result<bool> {
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

pub(super) fn merge_missing_mcp_servers(
    existing: &mut TomlValue,
    incoming: &TomlValue,
) -> io::Result<Vec<String>> {
    let existing_root = existing
        .as_table_mut()
        .ok_or_else(|| invalid_data_error("expected existing config to be a TOML table"))?;
    let incoming_root = incoming
        .as_table()
        .ok_or_else(|| invalid_data_error("expected migrated MCP config to be a TOML table"))?;
    let Some(incoming_servers) = incoming_root.get("mcp_servers") else {
        return Ok(Vec::new());
    };
    let incoming_servers = incoming_servers
        .as_table()
        .ok_or_else(|| invalid_data_error("expected migrated MCP servers to be a TOML table"))?;
    let Some(existing_servers) = existing_root.get_mut("mcp_servers") else {
        existing_root.insert(
            "mcp_servers".to_string(),
            TomlValue::Table(incoming_servers.clone()),
        );
        return Ok(incoming_servers.keys().cloned().collect());
    };
    let Some(existing_servers) = existing_servers.as_table_mut() else {
        return Ok(Vec::new());
    };

    let mut merged_server_names = Vec::new();
    for (server_name, incoming_server) in incoming_servers {
        if !existing_servers.contains_key(server_name) {
            existing_servers.insert(server_name.clone(), incoming_server.clone());
            merged_server_names.push(server_name.clone());
        }
    }
    Ok(merged_server_names)
}

pub(super) fn write_toml_file(path: &Path, value: &TomlValue) -> io::Result<()> {
    let serialized = toml::to_string_pretty(value)
        .map_err(|err| invalid_data_error(format!("failed to serialize config.toml: {err}")))?;
    fs::write(path, format!("{}\n", serialized.trim_end()))
}

pub(super) fn migrated_mcp_server_names(value: &TomlValue) -> Vec<String> {
    value
        .get("mcp_servers")
        .and_then(TomlValue::as_table)
        .map(|servers| servers.keys().cloned().collect())
        .unwrap_or_default()
}

pub(super) fn is_empty_toml_table(value: &TomlValue) -> bool {
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
