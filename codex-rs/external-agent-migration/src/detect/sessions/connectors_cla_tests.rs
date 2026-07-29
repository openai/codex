use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
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
    let attributions = vec![ImportedSessionConnectorAttribution {
        session_id: "session-1".to_string(),
        server_ids: BTreeSet::from(["figma".to_string()]),
    }];

    let connector_names = detect_imported_cla_session_connectors(
        &attributions,
        &[metadata_root.path().to_path_buf()],
    );

    assert_eq!(
        connector_names,
        BTreeMap::from([("session-1".to_string(), vec!["Figma".to_string()])])
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
    let attributions = vec![ImportedSessionConnectorAttribution {
        session_id: "session-1".to_string(),
        server_ids: BTreeSet::from(["c58ac595-58b5-48f8-ac77-d8d01523dede".to_string()]),
    }];

    let connector_names = detect_imported_cla_session_connectors(
        &attributions,
        &[metadata_root.path().to_path_buf()],
    );

    assert_eq!(
        connector_names,
        BTreeMap::from([("session-1".to_string(), vec!["Figma".to_string()])])
    );
}
