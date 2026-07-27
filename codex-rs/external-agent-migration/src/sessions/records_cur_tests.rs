use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn unwraps_user_query_after_context_wrappers() {
    let root = TempDir::new().expect("tempdir");
    let path = root.path().join("session.jsonl");
    let contents = serde_json::json!({
        "role": "user",
        "message": {
            "content": [{
                "type": "text",
                "text": concat!(
                    "<cursor_commands>\n",
                    "--- Cursor Command: verify ---\n",
                    "Run the verification checklist.\n",
                    "</cursor_commands>\n",
                    "<timestamp>Saturday, Jul 25, 2026, 10:27 AM (UTC-7)</timestamp>\n",
                    "<user_query>\n",
                    "Verify the project without modifying files.\n",
                    "</user_query>",
                ),
            }],
        },
    })
    .to_string();
    std::fs::write(&path, contents).expect("session");

    let parsed = read_session_import(&path, Some(root.path())).expect("parse session");
    let summary = summarize_session(&path, Some(root.path()))
        .expect("summarize session")
        .expect("session summary");

    assert_eq!(
        parsed.messages[0].text,
        "Verify the project without modifying files."
    );
    assert_eq!(
        summary.migration.title.as_deref(),
        Some("Verify the project without modifying files.")
    );
}

#[test]
fn preserves_user_query_tags_after_unrecognized_context() {
    let text = "<user_note>keep this</user_note>\n<user_query>query</user_query>".to_string();

    assert_eq!(unwrap_user_query(text.clone()), text);
}

#[test]
fn embedded_cwd_overrides_migration_fallback() {
    let root = TempDir::new().expect("tempdir");
    let embedded_cwd = root.path().join("embedded");
    let fallback_cwd = root.path().join("fallback");
    let path = root.path().join("session.jsonl");
    std::fs::write(
        &path,
        serde_json::json!({
            "cwd": embedded_cwd,
            "role": "user",
            "message": {"content": "first request"},
        })
        .to_string(),
    )
    .expect("session");

    let parsed = read_session_import(&path, Some(&fallback_cwd)).expect("parse session");
    let summary = summarize_session(&path, Some(&fallback_cwd))
        .expect("summarize session")
        .expect("session summary");

    assert_eq!(parsed.cwd.as_deref(), Some(embedded_cwd.as_path()));
    assert_eq!(summary.migration.cwd, embedded_cwd);
}
