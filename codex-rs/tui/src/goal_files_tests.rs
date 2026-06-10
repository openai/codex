use super::*;

use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::TextElement;
use pretty_assertions::assert_eq;

struct LocalStore;

impl GoalFileStore for LocalStore {
    async fn create_directory(&mut self, path: AbsolutePathBuf) -> Result<()> {
        fs::create_dir_all(path.as_path())?;
        Ok(())
    }

    async fn write_file(&mut self, path: AbsolutePathBuf, bytes: Vec<u8>) -> Result<()> {
        fs::write(path.as_path(), bytes)?;
        Ok(())
    }

    async fn read_file(&mut self, path: AbsolutePathBuf) -> Result<Vec<u8>> {
        Ok(fs::read(path.as_path())?)
    }
}

#[tokio::test]
async fn materializes_and_reads_oversized_objective_through_store() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let codex_home =
        AbsolutePathBuf::from_absolute_path_checked(temp_dir.path()).expect("absolute codex home");
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    let mut store = LocalStore;

    let reference = materialize_goal_draft(
        &mut store,
        &codex_home,
        GoalDraft {
            objective: objective.clone(),
            ..Default::default()
        },
    )
    .await
    .expect("materialize goal draft");

    let path = objective_file_path(&reference).expect("goal file path");
    assert_eq!(
        fs::read_to_string(path.as_path()).expect("read file"),
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
    let codex_home =
        AbsolutePathBuf::from_absolute_path_checked(temp_dir.path()).expect("absolute codex home");
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
        &codex_home,
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
    assert_eq!(
        fs::read_to_string(paste_path.as_path()).expect("read paste"),
        "hello"
    );
    assert_eq!(
        fs::read(image_path.as_path()).expect("read image"),
        b"png bytes"
    );
}

fn path_after(text: &str, prefix: &str) -> AbsolutePathBuf {
    let path = text
        .split_once(prefix)
        .unwrap_or_else(|| panic!("expected {prefix:?} in {text:?}"))
        .1
        .split_whitespace()
        .next()
        .expect("path");
    AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute path")
}
