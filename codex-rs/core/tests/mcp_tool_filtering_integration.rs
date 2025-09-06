//! Integration tests for MCP tool filtering functionality.

use codex_core::config_types::McpServerConfig;
use std::collections::HashMap;

#[test]
fn test_mcp_tool_filtering_config_parsing() {
    // Test that the new configuration fields are properly parsed
    let config_toml = r#"
[mcp_servers.test_server]
command = "npx"
args = ["-y", "test-mcp-server"]
env = { "API_KEY" = "test-key" }
exclude_tools = ["dangerous_tool", "bad_tool"]
"#;

    let config: toml::Value = toml::from_str(config_toml).unwrap();
    let mcp_servers = config.get("mcp_servers").unwrap();
    let server_config = mcp_servers.get("test_server").unwrap();

    // Convert to string and parse directly
    let server_config_str = toml::to_string(server_config).unwrap();
    let mcp_config: McpServerConfig = toml::from_str(&server_config_str).unwrap();

    assert_eq!(mcp_config.command, "npx");
    assert_eq!(mcp_config.args, vec!["-y", "test-mcp-server"]);
    assert_eq!(
        mcp_config.env.unwrap().get("API_KEY"),
        Some(&"test-key".to_string())
    );
    assert_eq!(
        mcp_config.exclude_tools,
        Some(vec!["dangerous_tool".to_string(), "bad_tool".to_string()])
    );
    assert_eq!(mcp_config.include_tools, None);
}

#[test]
fn test_mcp_tool_filtering_config_include_tools() {
    let config_toml = r#"
[mcp_servers.test_server]
command = "npx"
args = ["-y", "test-mcp-server"]
include_tools = ["safe_tool", "readonly_tool"]
"#;

    let config: toml::Value = toml::from_str(config_toml).unwrap();
    let mcp_servers = config.get("mcp_servers").unwrap();
    let server_config = mcp_servers.get("test_server").unwrap();

    // Convert to string and parse directly
    let server_config_str = toml::to_string(server_config).unwrap();
    let mcp_config: McpServerConfig = toml::from_str(&server_config_str).unwrap();

    assert_eq!(
        mcp_config.include_tools,
        Some(vec!["safe_tool".to_string(), "readonly_tool".to_string()])
    );
    assert_eq!(mcp_config.exclude_tools, None);
}

#[test]
fn test_mcp_tool_filtering_config_validation() {
    // Test that validation works correctly
    let valid_config = McpServerConfig {
        command: "test".to_string(),
        args: vec![],
        env: None,
        exclude_tools: Some(vec!["bad_tool".to_string()]),
        include_tools: None,
    };
    assert!(valid_config.validate().is_ok());

    let invalid_config = McpServerConfig {
        command: "test".to_string(),
        args: vec![],
        env: None,
        exclude_tools: Some(vec!["bad_tool".to_string()]),
        include_tools: Some(vec!["good_tool".to_string()]),
    };
    assert!(invalid_config.validate().is_err());
    assert_eq!(
        invalid_config.validate().unwrap_err(),
        "exclude_tools and include_tools cannot both be specified"
    );
}

#[test]
fn test_mcp_tool_filtering_config_serialization() {
    // Test that the configuration can be serialized and deserialized
    let config = McpServerConfig {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "test-server".to_string()],
        env: Some(HashMap::from([
            ("API_KEY".to_string(), "test-key".to_string()),
            ("DEBUG".to_string(), "true".to_string()),
        ])),
        exclude_tools: Some(vec!["dangerous_tool".to_string(), "bad_tool".to_string()]),
        include_tools: None,
    };

    // Serialize to TOML
    let toml_string = toml::to_string(&config).unwrap();

    // Deserialize back
    let deserialized: McpServerConfig = toml::from_str(&toml_string).unwrap();

    assert_eq!(config, deserialized);
}
