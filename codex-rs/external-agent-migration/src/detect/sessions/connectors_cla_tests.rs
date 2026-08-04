use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
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

#[test]
fn keeps_duplicate_session_stems_separate_when_batched() {
    let metadata_root = TempDir::new().expect("tempdir");
    let manifests_root = metadata_root.path().join(SESSION_MANIFESTS_DIR);
    std::fs::create_dir_all(&manifests_root).expect("manifest directory");
    std::fs::write(
        manifests_root.join("figma-session.json"),
        serde_json::json!({
            "cliSessionId": "session",
            "remoteMcpServersConfig": [
                {
                    "uuid": "figma-server",
                    "name": "Figma",
                },
            ],
        })
        .to_string(),
    )
    .expect("figma manifest");
    std::fs::write(
        manifests_root.join("slack-session.json"),
        serde_json::json!({
            "cliSessionId": "session",
            "remoteMcpServersConfig": [
                {
                    "uuid": "slack-server",
                    "name": "Slack",
                },
            ],
        })
        .to_string(),
    )
    .expect("slack manifest");

    let first_project = metadata_root.path().join("project-a");
    let second_project = metadata_root.path().join("project-b");
    std::fs::create_dir_all(&first_project).expect("first project");
    std::fs::create_dir_all(&second_project).expect("second project");
    let first_session = first_project.join("session.jsonl");
    let second_session = second_project.join("session.jsonl");
    std::fs::write(
        &first_session,
        serde_json::json!({"attributionMcpServer": "figma-server"}).to_string(),
    )
    .expect("first session");
    std::fs::write(
        &second_session,
        serde_json::json!({"attributionMcpServer": "slack-server"}).to_string(),
    )
    .expect("second session");
    let first_source_path = std::fs::canonicalize(&first_session).expect("first source path");
    let second_source_path = std::fs::canonicalize(&second_session).expect("second source path");
    let sessions = [
        ExternalAgentSessionMigration {
            path: first_session,
            cwd: first_project,
            title: None,
        },
        ExternalAgentSessionMigration {
            path: second_session,
            cwd: second_project,
            title: None,
        },
    ];

    assert_eq!(
        detect_cla_session_connectors_by_source_path(
            &sessions,
            &[metadata_root.path().to_path_buf()]
        ),
        BTreeMap::from([
            (
                first_source_path,
                vec![DetectedConnectorCandidate {
                    name: "Figma".to_string(),
                    session_count: 1,
                    source: DetectedConnectorSource::RemoteMcpServersConfig,
                }],
            ),
            (
                second_source_path,
                vec![DetectedConnectorCandidate {
                    name: "Slack".to_string(),
                    session_count: 1,
                    source: DetectedConnectorSource::RemoteMcpServersConfig,
                }],
            ),
        ])
    );
}
