use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::Path;

use toml::Value as TomlValue;

use crate::CONFIG_TOML_FILE;
use crate::McpServerConfig;

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
