use super::*;

use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use pretty_assertions::assert_eq;
use std::path::Path;

struct LocalStore;

impl GoalFileStore for LocalStore {
    async fn create_directory(&mut self, path: GoalFilePath) -> Result<()> {
        fs::create_dir_all(path.as_str())?;
        Ok(())
    }

    async fn write_file(&mut self, path: GoalFilePath, bytes: Vec<u8>) -> Result<()> {
        fs::write(path.as_str(), bytes)?;
        Ok(())
    }

    async fn read_file(&mut self, path: GoalFilePath) -> Result<Vec<u8>> {
        Ok(fs::read(path.as_str())?)
    }
}

#[derive(Default)]
struct RecordingStore {
    created_dirs: Vec<String>,
    writes: Vec<(String, Vec<u8>)>,
}

impl GoalFileStore for RecordingStore {
    async fn create_directory(&mut self, path: GoalFilePath) -> Result<()> {
        self.created_dirs.push(path.as_str().to_string());
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
            .ok_or_else(|| anyhow::anyhow!("missing recording for {}", path.display()))
    }
}

#[tokio::test]
async fn materializes_and_reads_oversized_objective_through_store() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let codex_home = local_goal_home(temp_dir.path());
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    let mut store = LocalStore;

    let reference = materialize_goal_draft(
        &mut store,
        Some(&codex_home),
        GoalDraft {
            objective: objective.clone(),
            ..Default::default()
        },
    )
    .await
    .expect("materialize goal draft");

    let path = objective_file_path(&reference).expect("goal file path");
    assert_eq!(
        fs::read_to_string(path.as_str()).expect("read file"),
        objective
    );
    let edit_text = objective_text_for_edit(&mut store, &reference)
        .await
        .expect("read objective text");
    assert_eq!(edit_text, objective);
}

#[tokio::test]
async fn materializes_paste_and_image_through_store() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let codex_home = local_goal_home(temp_dir.path());
    let image_path = temp_dir.path().join("local-image.png");
    fs::write(&image_path, b"png bytes").expect("write image");
    let mut store = LocalStore;
    let objective = "Use [Pasted Content 5 chars] and [Image #1]".to_string();
    let paste_placeholder = "[Pasted Content 5 chars]";
    let image_placeholder = "[Image #1]";
    let paste_start = objective
        .find(paste_placeholder)
        .expect("paste placeholder");
    let image_start = objective
        .find(image_placeholder)
        .expect("image placeholder");

    let objective = materialize_goal_draft(
        &mut store,
        Some(&codex_home),
        GoalDraft {
            objective,
            text_elements: vec![
                TextElement::new(
                    (paste_start..paste_start + paste_placeholder.len()).into(),
                    Some(paste_placeholder.to_string()),
                ),
                TextElement::new(
                    (image_start..image_start + image_placeholder.len()).into(),
                    Some(image_placeholder.to_string()),
                ),
            ],
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

    let paste_path = path_after(&objective, "pasted text file: ");
    let image_path = path_after(&objective, "image file: ");
    assert_eq!(fs::read_to_string(paste_path).expect("read paste"), "hello");
    assert_eq!(fs::read(image_path).expect("read image"), b"png bytes");
}

#[tokio::test]
async fn materializes_oversized_objective_with_windows_remote_path() {
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    let codex_home = GoalFilePath::from_remote(r"C:\Users\codex\.codex", Some("windows"));
    let mut store = RecordingStore::default();

    let reference = materialize_goal_draft(
        &mut store,
        Some(&codex_home),
        GoalDraft {
            objective: objective.clone(),
            ..Default::default()
        },
    )
    .await
    .expect("materialize goal draft");

    assert_eq!(store.created_dirs.len(), 1);
    assert_eq!(store.writes.len(), 1);
    let (path, bytes) = &store.writes[0];
    assert!(path.starts_with(r"C:\Users\codex\.codex\attachments\"));
    assert!(path.ends_with(r"\goal-objective.md"));
    assert_eq!(bytes, objective.as_bytes());
    assert_eq!(
        objective_file_path(&reference)
            .expect("goal file path")
            .as_str(),
        path
    );
}

fn path_after(text: &str, prefix: &str) -> String {
    let path = text
        .split_once(prefix)
        .unwrap_or_else(|| panic!("expected {prefix:?} in {text:?}"))
        .1
        .split_whitespace()
        .next()
        .expect("path");
    path.to_string()
}

fn local_goal_home(path: &Path) -> GoalFilePath {
    let path = AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute codex home");
    GoalFilePath::from_local(&path)
}

#[tokio::test]
async fn plain_objective_does_not_need_codex_home() {
    let mut store = LocalStore;

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
async fn oversized_objective_requires_codex_home() {
    let mut store = LocalStore;

    let err = materialize_goal_draft(
        &mut store,
        /*codex_home*/ None,
        GoalDraft {
            objective: "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1),
            ..Default::default()
        },
    )
    .await
    .expect_err("oversized objective should require codex home");

    assert!(
        err.to_string().contains("$CODEX_HOME"),
        "expected codex home error, got {err:#}"
    );
}
