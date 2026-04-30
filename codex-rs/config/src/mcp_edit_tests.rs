use super::*;
use crate::McpServerToolConfig;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use toml::Value as TomlValue;

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
            experimental_environment: None,
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

#[tokio::test]
async fn config_edits_set_and_clear_nested_paths() -> anyhow::Result<()> {
    let codex_home = tempfile::tempdir()?;
    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"plugins = { "sample@test" = { enabled = false, keep = true } }
"#,
    )?;

    ConfigEditsBuilder::new(codex_home.path())
        .with_edits([
            ConfigEdit::SetPath {
                segments: vec![
                    "plugins".to_string(),
                    "sample@test".to_string(),
                    "enabled".to_string(),
                ],
                value: value(true),
            },
            ConfigEdit::ClearPath {
                segments: vec![
                    "plugins".to_string(),
                    "sample@test".to_string(),
                    "keep".to_string(),
                ],
            },
        ])
        .apply()
        .await?;

    let config: TomlValue = toml::from_str(&std::fs::read_to_string(
        codex_home.path().join(CONFIG_TOML_FILE),
    )?)?;
    let plugin = config
        .get("plugins")
        .and_then(|plugins| plugins.get("sample@test"))
        .and_then(TomlValue::as_table)
        .expect("plugin config should be a table");

    assert_eq!(plugin.get("enabled"), Some(&TomlValue::Boolean(true)));
    assert_eq!(plugin.get("keep"), None);

    Ok(())
}
