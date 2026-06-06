use std::fs;

use codex_extension_api::GlobalInstruction;
use codex_extension_api::GlobalInstructions;
use codex_extension_api::GlobalInstructionsContributor;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::CodexHomeInstructionsContributor;

fn contributor(temp: &TempDir) -> CodexHomeInstructionsContributor {
    CodexHomeInstructionsContributor::new(
        AbsolutePathBuf::try_from(temp.path().to_path_buf()).expect("absolute temp path"),
    )
}

#[tokio::test]
async fn loads_override_before_default() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("AGENTS.md"), "default").expect("write default");
    fs::write(temp.path().join("AGENTS.override.md"), "override").expect("write override");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(
        actual,
        GlobalInstructions {
            instructions: vec![GlobalInstruction::new(
                "override",
                Some(
                    AbsolutePathBuf::try_from(temp.path().join("AGENTS.override.md"))
                        .expect("absolute override path")
                ),
            )],
            warnings: Vec::new(),
        }
    );
}

#[tokio::test]
async fn falls_back_from_empty_override_to_default() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("AGENTS.override.md"), " \n").expect("write override");
    fs::write(temp.path().join("AGENTS.md"), "default").expect("write default");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(
        actual,
        GlobalInstructions {
            instructions: vec![GlobalInstruction::new(
                "default",
                Some(
                    AbsolutePathBuf::try_from(temp.path().join("AGENTS.md"))
                        .expect("absolute default path")
                ),
            )],
            warnings: Vec::new(),
        }
    );
}

#[tokio::test]
async fn falls_back_from_directory_override_to_default() {
    let temp = TempDir::new().expect("temp dir");
    fs::create_dir(temp.path().join("AGENTS.override.md")).expect("create override directory");
    fs::write(temp.path().join("AGENTS.md"), "default").expect("write default");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(
        actual.instructions,
        vec![GlobalInstruction::new(
            "default",
            Some(
                AbsolutePathBuf::try_from(temp.path().join("AGENTS.md"))
                    .expect("absolute default path")
            ),
        )]
    );
}

#[tokio::test]
async fn whitespace_only_files_return_empty_instructions() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("AGENTS.override.md"), " \n").expect("write override");
    fs::write(temp.path().join("AGENTS.md"), "\t").expect("write default");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(actual, GlobalInstructions::default());
}

#[tokio::test]
async fn loads_default_without_an_execution_environment() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("AGENTS.md"), "default").expect("write default");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(
        actual.instructions,
        vec![GlobalInstruction::new(
            "default",
            Some(
                AbsolutePathBuf::try_from(temp.path().join("AGENTS.md"))
                    .expect("absolute default path")
            ),
        )]
    );
}

#[tokio::test]
async fn missing_files_return_empty_instructions() {
    let temp = TempDir::new().expect("temp dir");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(actual, Default::default());
}

#[tokio::test]
async fn invalid_utf8_is_lossy_and_warns() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("AGENTS.md");
    let data = vec![
        b'g', b'l', b'o', b'b', b'a', b'l', 0xff, b' ', b'd', b'o', b'c',
    ];
    fs::write(&path, &data).expect("write default");
    let utf8_error = std::str::from_utf8(&data).expect_err("fixture should be invalid UTF-8");

    let actual = contributor(&temp).contribute().await.expect("load");

    assert_eq!(
        actual,
        GlobalInstructions {
            instructions: vec![GlobalInstruction::new(
                "global\u{fffd} doc",
                Some(AbsolutePathBuf::try_from(path.clone()).expect("absolute default path")),
            )],
            warnings: vec![format!(
                "Global AGENTS.md instructions from `{}` contain invalid UTF-8: {utf8_error}. Invalid byte sequences were replaced.",
                path.display()
            )],
        }
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_candidates_fail_instead_of_deleting_the_previous_snapshot() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("AGENTS.override.md");
    symlink(&path, &path).expect("create symlink loop");

    let error = contributor(&temp)
        .contribute()
        .await
        .expect_err("unreadable candidate should fail");

    assert!(error.contains("Failed to read global AGENTS.md instructions"));
    assert!(error.contains(&path.display().to_string()));
}
