use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn detects_connector_used_by_session_mcp_tool_call() {
    let source_home = TempDir::new().expect("source home");
    write_cached_plugin(source_home.path(), "figma", "Figma", "figma");
    let first_session = write_session(
        source_home.path(),
        "session-1",
        &[
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "GetMcpTools",
                        "input": {"server": "plugin-figma-figma"}
                    }]
                }
            }),
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "CallMcpTool",
                        "input": {"server": "plugin-figma-figma", "toolName": "whoami"}
                    }]
                }
            }),
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "CallMcpTool",
                        "input": {"server": "plugin-figma-figma", "toolName": "get_metadata"}
                    }]
                }
            }),
        ],
    );
    let second_session = write_session(
        source_home.path(),
        "session-2",
        &[serde_json::json!({
            "role": "assistant",
            "message": {
                "content": [{
                    "type": "tool_use",
                    "name": "CallMcpTool",
                    "input": {"server": "plugin-figma-figma", "toolName": "get_screenshot"}
                }]
            }
        })],
    );

    let connectors = detect_cur_session_connectors(
        &[
            ExternalAgentSessionMigration {
                path: first_session,
                cwd: source_home.path().to_path_buf(),
                title: None,
            },
            ExternalAgentSessionMigration {
                path: second_session,
                cwd: source_home.path().to_path_buf(),
                title: None,
            },
        ],
        source_home.path(),
    );

    assert_eq!(
        connectors,
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 2,
            source: DetectedConnectorSource::SessionToolUse,
        }]
    );
}

fn write_cached_plugin(
    source_home: &Path,
    plugin_name: &str,
    display_name: &str,
    server_name: &str,
) {
    let version_root = source_home
        .join(PLUGIN_CACHE_DIR)
        .join("marketplace")
        .join(plugin_name)
        .join("version");
    fs::create_dir_all(version_root.join(".cursor-plugin")).expect("plugin directory");
    fs::write(
        version_root.join(PLUGIN_MANIFEST_PATH),
        serde_json::json!({
            "name": plugin_name,
            "displayName": display_name,
        })
        .to_string(),
    )
    .expect("plugin manifest");
    fs::write(
        version_root.join(MCP_CONFIG_PATH),
        serde_json::json!({
            "mcpServers": {
                (server_name): {"type": "http", "url": "https://example.invalid/mcp"}
            }
        })
        .to_string(),
    )
    .expect("mcp config");
}

fn write_session(source_home: &Path, name: &str, records: &[JsonValue]) -> std::path::PathBuf {
    let path = source_home.join(format!("{name}.jsonl"));
    fs::write(
        &path,
        records
            .iter()
            .map(JsonValue::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("session");
    path
}
