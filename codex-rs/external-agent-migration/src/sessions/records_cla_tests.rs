use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn reads_session_import_in_one_pass() {
    let root = TempDir::new().expect("tempdir");
    let path = root.path().join("session.jsonl");
    let contents = [
        serde_json::json!({
            "type": "user",
            "cwd": root.path(),
            "timestamp": "2026-06-03T12:00:00Z",
            "message": { "content": "<user_query>\nfirst request\n</user_query>" },
        })
        .to_string(),
        "not json".to_string(),
        serde_json::json!({
            "type": "ai-title",
            "aiTitle": "generated title",
        })
        .to_string(),
        serde_json::json!({
            "type": "custom-title",
            "customTitle": "custom title",
        })
        .to_string(),
    ]
    .join("\n");
    std::fs::write(&path, &contents).expect("session");

    let parsed = read_session_import(&path).expect("parse session");

    assert_eq!(parsed.cwd.as_deref(), Some(root.path()));
    assert_eq!(parsed.custom_title.as_deref(), Some("custom title"));
    assert_eq!(parsed.ai_title.as_deref(), Some("generated title"));
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.messages[0].text, "first request");
    assert_eq!(
        parsed.content_sha256,
        format!("{:x}", Sha256::digest(contents))
    );
}
