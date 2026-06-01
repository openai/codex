use super::*;
use crate::McpServerOAuthConfig;
use crate::McpServerToolConfig;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[tokio::test]
async fn replace_mcp_servers_serializes_per_tool_approval_overrides() -> anyhow::Result<()> {
    let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let codex_home = std::env::temp_dir().join(format!(
        "codex-config-mcp-edit-test-{}-{unique_suffix}",
        std::process::id()
    ));
    let servers = BTreeMap::from([(
        "docs".to_string(),
        McpServerConfig::builder()
            .transport(McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            })
            .supports_parallel_tool_calls(/*value*/ true)
            .default_tools_approval_mode(AppToolApproval::Auto)
            .tools(HashMap::from([
                (
                    "search".to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(AppToolApproval::Approve),
                    },
                ),
                (
                    "read".to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(AppToolApproval::Prompt),
                    },
                ),
            ]))
            .build(),
    )]);

    ConfigEditsBuilder::new(&codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await?;

    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.docs]
command = "docs-server"
supports_parallel_tool_calls = true
default_tools_approval_mode = "auto"

[mcp_servers.docs.tools]

[mcp_servers.docs.tools.read]
approval_mode = "prompt"

[mcp_servers.docs.tools.search]
approval_mode = "approve"
"#
    );

    let loaded = load_global_mcp_servers(&codex_home).await?;
    assert_eq!(loaded, servers);

    std::fs::remove_dir_all(&codex_home)?;

    Ok(())
}

#[tokio::test]
async fn replace_mcp_servers_serializes_oauth_client_id() -> anyhow::Result<()> {
    let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let codex_home = std::env::temp_dir().join(format!(
        "codex-config-mcp-oauth-edit-test-{}-{unique_suffix}",
        std::process::id()
    ));
    let servers = BTreeMap::from([(
        "maas_outlook".to_string(),
        McpServerConfig::builder()
            .transport(McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            })
            .oauth(McpServerOAuthConfig {
                client_id: Some("eci-prd-pub-codex-123".to_string()),
            })
            .build(),
    )]);

    ConfigEditsBuilder::new(&codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await?;

    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let serialized = std::fs::read_to_string(&config_path)?;
    assert_eq!(
        serialized,
        r#"[mcp_servers.maas_outlook]
url = "https://example.com/mcp"

[mcp_servers.maas_outlook.oauth]
client_id = "eci-prd-pub-codex-123"
"#
    );

    let loaded = load_global_mcp_servers(&codex_home).await?;
    assert_eq!(loaded, servers);

    std::fs::remove_dir_all(&codex_home)?;

    Ok(())
}
