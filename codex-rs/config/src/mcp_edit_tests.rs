use super::*;
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
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            supports_parallel_tool_calls: true,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: Some(AppToolApproval::Auto),
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::from([
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
            ]),
        },
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
