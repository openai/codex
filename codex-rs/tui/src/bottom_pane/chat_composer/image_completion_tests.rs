#![cfg(test)]
//! Image attachment and text fallback for file picker selections.

use super::tests::complete_file;
use super::tests::new_test_composer;
use super::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn image_completion_submits_the_file_from_the_search_root() {
    let tmp = tempdir().expect("create TempDir");
    let path = tmp.path().join("snap4.png");
    image::RgbaImage::new(1, 1).save(&path).expect("write PNG");

    for mentions_v2 in [false, true] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_mentions_v2_enabled(mentions_v2);
        complete_file(&mut composer, "@snap", "@snap".len(), "snap", path.clone());
        insta::allow_duplicates! {
            insta::assert_snapshot!(composer.current_text().trim_end(), @"[Image #1]");
        }
        let (result, _) =
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, InputResult::Submitted { .. }));
        assert_eq!(composer.take_recent_submission_images(), vec![path.clone()]);
    }
}

#[cfg(unix)]
#[test]
fn image_completion_keeps_a_named_pipe_as_relative_text() {
    let tmp = tempdir().expect("create TempDir");
    let path = tmp.path().join("pipe.png");
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("run mkfifo");
    assert!(status.success());

    let (mut composer, _rx) = new_test_composer();
    complete_file(&mut composer, "@pipe", "@pipe".len(), "pipe", path);
    insta::assert_snapshot!(composer.current_text().trim_end(), @"pipe.png");
}
