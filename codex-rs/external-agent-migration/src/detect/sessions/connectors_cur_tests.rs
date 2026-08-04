use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
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

    let first_source_path = fs::canonicalize(&first_session).expect("first canonical source");
    let second_source_path = fs::canonicalize(&second_session).expect("second canonical source");
    let sessions = [
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
    ];

    assert_eq!(
        detect_cur_session_connectors_by_source_path(&sessions, source_home.path()),
        BTreeMap::from([
            (
                first_source_path,
                vec![DetectedConnectorCandidate {
                    name: "Figma".to_string(),
                    session_count: 1,
                    source: DetectedConnectorSource::SessionToolUse,
                }],
            ),
            (
                second_source_path,
                vec![DetectedConnectorCandidate {
                    name: "Figma".to_string(),
                    session_count: 1,
                    source: DetectedConnectorSource::SessionToolUse,
                }],
            ),
        ])
    );

    let connectors = detect_cur_session_connectors(&sessions, source_home.path());

    assert_eq!(
        connectors,
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 2,
            source: DetectedConnectorSource::SessionToolUse,
        }]
    );
}

#[test]
fn detects_call_mcp_tool_connector_from_project_mcp_server_metadata() {
    let source_home = TempDir::new().expect("source home");
    let project_root = source_home.path().join("projects/project");
    let session_path = project_root
        .join(AGENT_TRANSCRIPTS_DIR)
        .join("session-1")
        .join("session-1.jsonl");
    fs::create_dir_all(session_path.parent().expect("session parent")).expect("session directory");
    fs::write(
        &session_path,
        [
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "GetMcpTools",
                        "input": {"server": "user-figma"}
                    }]
                }
            }),
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "CallMcpTool",
                        "input": {"server": "user-figma", "toolName": "whoami"}
                    }]
                }
            }),
        ]
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n"),
    )
    .expect("session");
    let server_root = project_root.join(PROJECT_MCP_DIR).join("user-figma");
    fs::create_dir_all(&server_root).expect("server directory");
    fs::write(
        server_root.join(PROJECT_MCP_SERVER_METADATA_PATH),
        serde_json::json!({
            "serverIdentifier": "user-figma",
            "serverName": "Figma",
        })
        .to_string(),
    )
    .expect("server metadata");

    assert_eq!(
        detect_cur_session_connectors(
            &[ExternalAgentSessionMigration {
                path: session_path,
                cwd: source_home.path().to_path_buf(),
                title: None,
            }],
            source_home.path(),
        ),
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 1,
            source: DetectedConnectorSource::SessionToolUse,
        }]
    );
}

#[test]
fn detects_project_backed_mcp_details_connector_from_metadata_name() {
    let source_home = TempDir::new().expect("source home");
    let project_root = source_home.path().join("projects/project");
    let session_path = project_root
        .join(AGENT_TRANSCRIPTS_DIR)
        .join("session-1")
        .join("session-1.jsonl");
    fs::create_dir_all(session_path.parent().expect("session parent")).expect("session directory");
    fs::write(
        &session_path,
        [
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "mcp__figma__get_design_context",
                        "input": {
                            "namespace": "figma",
                            "toolName": "get_design_context",
                            "mcpDetails": {"serverName": "Figma"},
                            "arguments": {"nodeId": "fixture-node"}
                        }
                    }]
                }
            }),
            serde_json::json!({
                "role": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "name": "mcp__untrusted__whoami",
                        "input": {"mcpDetails": {"serverName": "Not In Metadata"}}
                    }]
                }
            }),
        ]
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n"),
    )
    .expect("session");
    let server_root = project_root
        .join(PROJECT_MCP_DIR)
        .join("plugin-figma-figma");
    fs::create_dir_all(&server_root).expect("server directory");
    fs::write(
        server_root.join(PROJECT_MCP_SERVER_METADATA_PATH),
        serde_json::json!({
            "serverIdentifier": "plugin-figma-figma",
            "serverName": "Figma",
        })
        .to_string(),
    )
    .expect("server metadata");

    assert_eq!(
        detect_cur_session_connectors(
            &[ExternalAgentSessionMigration {
                path: session_path,
                cwd: source_home.path().to_path_buf(),
                title: None,
            }],
            source_home.path(),
        ),
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 1,
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
