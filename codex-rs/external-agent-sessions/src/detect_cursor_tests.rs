use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn detects_cursor_transcript_with_project_cwd() {
    let root = TempDir::new().expect("tempdir");
    let project_root = root.path().join("workspace-with-dashes");
    fs::create_dir_all(&project_root).expect("project root");
    let external_agent_home = root.path().join(".cursor");
    let encoded_project = encode_project_path(&project_root);
    let session_id = "a-session";
    let transcript = external_agent_home
        .join("projects")
        .join(encoded_project)
        .join("agent-transcripts")
        .join(session_id)
        .join(format!("{session_id}.jsonl"));
    fs::create_dir_all(transcript.parent().expect("transcript parent"))
        .expect("transcript directory");
    fs::write(
        &transcript,
        concat!(
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"first request\"}]}}\n",
            "{\"role\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"first answer\"}]}}\n"
        ),
    )
    .expect("transcript");

    let sessions =
        detect_recent_cursor_sessions(&external_agent_home, root.path()).expect("detect sessions");

    assert_eq!(
        sessions,
        vec![ExternalAgentSessionMigration {
            path: transcript,
            cwd: project_root,
            title: Some("first request".to_string()),
        }]
    );
}

#[cfg(not(windows))]
fn encode_project_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "-")
}

#[cfg(windows)]
fn encode_project_path(path: &Path) -> String {
    path.to_string_lossy().replace([':', '\\'], "-")
}
