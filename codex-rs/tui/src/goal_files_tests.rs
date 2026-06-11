use super::*;

use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use pretty_assertions::assert_eq;

#[derive(Default)]
struct RecordingStore {
    writes: Vec<(String, Vec<u8>)>,
}

impl GoalFileStore for RecordingStore {
    async fn create_directory(&mut self, _path: GoalFilePath) -> Result<()> {
        Ok(())
    }

    async fn write_file(&mut self, path: GoalFilePath, bytes: Vec<u8>) -> Result<()> {
        self.writes.push((path.as_str().to_string(), bytes));
        Ok(())
    }

    async fn read_file(&mut self, path: GoalFilePath) -> Result<Vec<u8>> {
        self.writes
            .iter()
            .find(|(write_path, _)| write_path == path.as_str())
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| anyhow::anyhow!("missing recording for {path}"))
    }
}

#[tokio::test]
async fn materializes_oversized_objective_with_remote_windows_path() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let image_path = temp_dir.path().join("local-image.png");
    fs::write(&image_path, b"png bytes").expect("write image");
    let paste_placeholder = "[Pasted Content 5 chars]";
    let image_placeholder = "[Image #1]";
    let objective = format!(
        "Use {paste_placeholder} and {image_placeholder}. {}",
        "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1)
    );
    let text_elements = [paste_placeholder, image_placeholder]
        .into_iter()
        .map(|placeholder| {
            let start = objective.find(placeholder).expect("placeholder");
            TextElement::new(
                (start..start + placeholder.len()).into(),
                Some(placeholder.to_string()),
            )
        })
        .collect();
    let codex_home =
        codex_app_server_client::AppServerPath::from_app_server(r"C:\Users\codex\.codex");
    let mut store = RecordingStore::default();

    let reference = materialize_goal_draft(
        &mut store,
        Some(&codex_home),
        GoalDraft {
            objective: objective.clone(),
            text_elements,
            pending_pastes: vec![(paste_placeholder.to_string(), "hello".to_string())],
            local_images: vec![LocalImageAttachment {
                placeholder: image_placeholder.to_string(),
                path: image_path,
            }],
            ..Default::default()
        },
    )
    .await
    .expect("materialize goal draft");

    let path = objective_file_path(&reference).expect("goal file path");
    assert!(
        path.as_str()
            .starts_with(r"C:\Users\codex\.codex\attachments\")
    );
    assert!(path.as_str().ends_with(r"\goal-objective.md"));
    let edit_text = objective_text_for_edit(&mut store, &reference)
        .await
        .expect("read objective text");
    assert!(edit_text.contains(r"pasted text file: C:\Users\codex\.codex\attachments\"));
    assert!(edit_text.contains(r"image file: C:\Users\codex\.codex\attachments\"));
    assert!(store.writes.iter().any(|(_, bytes)| bytes == b"hello"));
    assert!(store.writes.iter().any(|(_, bytes)| bytes == b"png bytes"));
}

#[tokio::test]
async fn plain_objective_does_not_need_codex_home() {
    let mut store = RecordingStore::default();

    let objective = materialize_goal_draft(
        &mut store,
        /*codex_home*/ None,
        GoalDraft {
            objective: "read src/lib.rs".to_string(),
            ..Default::default()
        },
    )
    .await
    .expect("materialize plain goal draft");

    assert_eq!(objective, "read src/lib.rs");
}

#[tokio::test]
async fn deleted_placeholders_do_not_materialize_or_need_codex_home() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let image_path = temp_dir.path().join("local-image.png");
    fs::write(&image_path, b"png bytes").expect("write image");
    let mut store = RecordingStore::default();

    let objective = materialize_goal_draft(
        &mut store,
        /*codex_home*/ None,
        GoalDraft {
            objective: "small goal".to_string(),
            pending_pastes: vec![("[Pasted Content 5 chars]".to_string(), "hello".to_string())],
            local_images: vec![LocalImageAttachment {
                placeholder: "[Image #1]".to_string(),
                path: image_path,
            }],
            ..Default::default()
        },
    )
    .await
    .expect("materialize plain goal draft");

    assert_eq!(objective, "small goal");
    assert!(store.writes.is_empty());
}
