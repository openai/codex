use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn resolves_attributed_server_name_from_session_manifest() {
    let metadata_root = TempDir::new().expect("tempdir");
    let manifests_root = metadata_root.path().join(SESSION_MANIFESTS_DIR);
    std::fs::create_dir_all(&manifests_root).expect("manifest directory");
    std::fs::write(
        manifests_root.join("session.json"),
        serde_json::json!({
            "cliSessionId": "session-1",
            "remoteMcpServersConfig": [
                {
                    "uuid": "c58ac595-58b5-48f8-ac77-d8d01523dede",
                    "name": "Figma",
                },
            ],
        })
        .to_string(),
    )
    .expect("manifest");
    let session_path = metadata_root.path().join("session-1.jsonl");
    std::fs::write(
        &session_path,
        serde_json::json!({"attributionMcpServer": "figma"}).to_string(),
    )
    .expect("session");
    let sessions = vec![ExternalAgentSessionMigration {
        path: session_path,
        cwd: metadata_root.path().to_path_buf(),
        title: None,
    }];

    let connectors =
        detect_cla_session_connectors(&sessions, &[metadata_root.path().to_path_buf()]);

    assert_eq!(
        connectors,
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 1,
            source: DetectedConnectorSource::RemoteMcpServersConfig,
        }]
    );
}

#[test]
fn still_resolves_attributed_server_uuid_from_session_manifest() {
    let metadata_root = TempDir::new().expect("tempdir");
    let manifests_root = metadata_root.path().join(SESSION_MANIFESTS_DIR);
    std::fs::create_dir_all(&manifests_root).expect("manifest directory");
    std::fs::write(
        manifests_root.join("session.json"),
        serde_json::json!({
            "cliSessionId": "session-1",
            "remoteMcpServersConfig": [
                {
                    "uuid": "c58ac595-58b5-48f8-ac77-d8d01523dede",
                    "name": "Figma",
                },
            ],
        })
        .to_string(),
    )
    .expect("manifest");
    let session_path = metadata_root.path().join("session-1.jsonl");
    std::fs::write(
        &session_path,
        serde_json::json!({
            "attributionMcpServer": "c58ac595-58b5-48f8-ac77-d8d01523dede"
        })
        .to_string(),
    )
    .expect("session");
    let sessions = vec![ExternalAgentSessionMigration {
        path: session_path,
        cwd: metadata_root.path().to_path_buf(),
        title: None,
    }];

    let connectors =
        detect_cla_session_connectors(&sessions, &[metadata_root.path().to_path_buf()]);

    assert_eq!(
        connectors,
        vec![DetectedConnectorCandidate {
            name: "Figma".to_string(),
            session_count: 1,
            source: DetectedConnectorSource::RemoteMcpServersConfig,
        }]
    );
}
