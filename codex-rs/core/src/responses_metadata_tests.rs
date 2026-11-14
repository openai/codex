#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

fn metadata_with_remote(url: &str) -> CodexResponsesMetadata {
    let mut metadata = CodexResponsesMetadata::new(
        "install".into(),
        "session".into(),
        "thread".into(),
        "window".into(),
    );
    metadata.request_kind = Some(CodexResponsesRequestKind::Turn);
    metadata.turn_id = Some("turn".into());
    metadata.workspaces.insert(
        "/workspace/東京".into(),
        TurnMetadataWorkspace {
            associated_remote_urls: Some(BTreeMap::from([(
                "origin".into(),
                SanitizedGitUrl::try_from(url).unwrap(),
            )])),
            latest_git_commit_hash: Some("abc123".into()),
            has_changes: Some(false),
        },
    );
    metadata
}

#[test]
fn turn_metadata_keeps_the_exact_boundary_and_omits_an_overlong_remote() {
    let prefix = "https://example.invalid/";
    let initial_len = metadata_with_remote(prefix)
        .turn_metadata_json()
        .unwrap()
        .len();
    let padding = "x".repeat(MAX_TURN_METADATA_BYTES - initial_len);
    let boundary_url = format!("{prefix}{padding}");
    let boundary = metadata_with_remote(&boundary_url);
    let expected = to_ascii_json_string(&boundary.turn_metadata_payload()).unwrap();
    assert_eq!(expected.len(), MAX_TURN_METADATA_BYTES);
    assert_eq!(boundary.turn_metadata_json().unwrap(), expected);

    let over = metadata_with_remote(&format!("{boundary_url}x"));
    let body = over.client_metadata(/*include_internal*/ false);
    let canonical = &body[X_CODEX_TURN_METADATA_HEADER];
    assert_eq!(
        canonical,
        over.compatibility_headers()[X_CODEX_TURN_METADATA_HEADER]
            .to_str()
            .unwrap()
    );
    assert_eq!(
        serde_json::from_str::<Value>(canonical).unwrap()["workspaces"]["/workspace/東京"],
        json!({"latest_git_commit_hash":"abc123","has_changes":false})
    );
}

#[test]
fn oversized_remote_preserves_other_metadata_and_short_remote_urls() {
    let mut metadata =
        metadata_with_remote(&format!("https://example.invalid/{}é", "o".repeat(5000)));
    let workspace = metadata.workspaces.get_mut("/workspace/東京").unwrap();
    let remotes = workspace.associated_remote_urls.as_mut().unwrap();
    remotes.insert(
        "large".into(),
        SanitizedGitUrl::try_from(format!("https://example.invalid/{}", "x".repeat(5000)).as_str())
            .unwrap(),
    );
    remotes.insert(
        "upstream".into(),
        SanitizedGitUrl::try_from("git@example.invalid:upstream.git").unwrap(),
    );
    metadata.extra.insert("custom".into(), "retained".into());
    metadata.tool_namespaces_info = Some(BTreeMap::from([(
        "shell".into(),
        TurnToolNamespaceInfo {
            name: "shell".into(),
            functions: BTreeMap::new(),
        },
    )]));
    let body = metadata.client_metadata(/*include_internal*/ true);
    let canonical = &body[X_CODEX_TURN_METADATA_HEADER];
    assert!(canonical.len() <= MAX_TURN_METADATA_BYTES);
    assert!(canonical.is_ascii());
    let actual: Value = serde_json::from_str(canonical).unwrap();
    let mut expected = metadata.turn_metadata_value().unwrap();
    expected["workspaces"]["/workspace/東京"]["associated_remote_urls"]
        .as_object_mut()
        .unwrap()
        .remove("large");
    assert_eq!(actual, expected);
    let header = metadata.compatibility_headers();
    let header: Value =
        serde_json::from_str(header[X_CODEX_TURN_METADATA_HEADER].to_str().unwrap()).unwrap();
    assert_eq!(header["workspaces"], expected["workspaces"]);
    assert!(header.get("tool_namespaces_info").is_none());
}
