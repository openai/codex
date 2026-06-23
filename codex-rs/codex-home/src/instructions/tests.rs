use std::fs;
use std::path::Path;

use codex_extension_api::LoadedUserInstructions;
use codex_extension_api::UserInstructions;
use codex_extension_api::UserInstructionsProvider;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::CodexHomeUserInstructionsProvider;
use super::DEFAULT_AGENTS_MD_FILENAME;
use super::INSTRUCTIONS_DIR_NAME;
use super::LOCAL_AGENTS_MD_FILENAME;

fn provider(home: &TempDir) -> CodexHomeUserInstructionsProvider {
    CodexHomeUserInstructionsProvider::new(
        AbsolutePathBuf::try_from(home.path().to_path_buf()).expect("absolute temp dir"),
    )
}

fn expected(
    home: &TempDir,
    filename: &str,
    text: &str,
    warnings: Vec<String>,
) -> LoadedUserInstructions {
    expected_with_source(home.path().join(filename), text, warnings)
}

fn expected_with_source(
    source: impl AsRef<Path>,
    text: &str,
    warnings: Vec<String>,
) -> LoadedUserInstructions {
    LoadedUserInstructions {
        instructions: Some(UserInstructions {
            text: text.to_string(),
            source: AbsolutePathBuf::try_from(source.as_ref().to_path_buf())
                .expect("absolute source path"),
        }),
        warnings,
    }
}

#[cfg(unix)]
fn create_symlink_loop(path: &Path) {
    std::os::unix::fs::symlink(
        path.file_name().expect("override path should have a name"),
        path,
    )
    .expect("create symlink loop");
}

#[cfg(windows)]
fn create_symlink_loop(path: &Path) {
    std::os::windows::fs::symlink_file(
        path.file_name().expect("override path should have a name"),
        path,
    )
    .expect("create symlink loop");
}

#[tokio::test]
async fn missing_files_return_no_instructions() {
    let home = TempDir::new().expect("temp dir");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        LoadedUserInstructions::default()
    );
}

#[tokio::test]
async fn override_takes_precedence_over_default() {
    let home = TempDir::new().expect("temp dir");
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), "default").expect("write default");
    fs::write(home.path().join(LOCAL_AGENTS_MD_FILENAME), "override").expect("write override");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, LOCAL_AGENTS_MD_FILENAME, "override", Vec::new())
    );
}

#[tokio::test]
async fn empty_override_falls_back_to_trimmed_default() {
    let home = TempDir::new().expect("temp dir");
    fs::write(home.path().join(LOCAL_AGENTS_MD_FILENAME), " \n\t").expect("write override");
    fs::write(
        home.path().join(DEFAULT_AGENTS_MD_FILENAME),
        "\n  default instructions  \n",
    )
    .expect("write default");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(
            &home,
            DEFAULT_AGENTS_MD_FILENAME,
            "default instructions",
            Vec::new()
        )
    );
}

#[tokio::test]
async fn directory_override_falls_back_to_default() {
    let home = TempDir::new().expect("temp dir");
    fs::create_dir(home.path().join(LOCAL_AGENTS_MD_FILENAME)).expect("create override directory");
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), "default").expect("write default");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, DEFAULT_AGENTS_MD_FILENAME, "default", Vec::new())
    );
}

#[tokio::test]
async fn instructions_directory_markdown_files_are_loaded_recursively_in_sorted_order() {
    let home = TempDir::new().expect("temp dir");
    let instructions_dir = home.path().join(INSTRUCTIONS_DIR_NAME);
    fs::create_dir_all(instructions_dir.join("eng")).expect("create eng instructions dir");
    fs::create_dir_all(instructions_dir.join("sales")).expect("create sales instructions dir");
    fs::write(
        instructions_dir.join("sales").join("20-team-specifics.md"),
        "third",
    )
    .expect("write third");
    fs::write(instructions_dir.join("00-guardrails.md"), "first").expect("write first");
    fs::write(
        instructions_dir.join("eng").join("10-internal-tooling.md"),
        "second",
    )
    .expect("write second");
    fs::write(instructions_dir.join("c.txt"), "ignored").expect("write ignored");
    fs::write(instructions_dir.join("eng").join("05-empty.md"), " \n\t").expect("write empty");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected_with_source(&instructions_dir, "first\n\nsecond\n\nthird", Vec::new())
    );
}

#[tokio::test]
async fn agents_md_prevents_instructions_directory_fallback() {
    let home = TempDir::new().expect("temp dir");
    let instructions_dir = home.path().join(INSTRUCTIONS_DIR_NAME);
    fs::create_dir(&instructions_dir).expect("create instructions dir");
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), "default").expect("write default");
    fs::write(instructions_dir.join("extra.md"), "extra").expect("write extra");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, DEFAULT_AGENTS_MD_FILENAME, "default", Vec::new())
    );
}

#[tokio::test]
async fn override_prevents_instructions_directory_fallback() {
    let home = TempDir::new().expect("temp dir");
    let instructions_dir = home.path().join(INSTRUCTIONS_DIR_NAME);
    fs::create_dir(&instructions_dir).expect("create instructions dir");
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), "default").expect("write default");
    fs::write(home.path().join(LOCAL_AGENTS_MD_FILENAME), "override").expect("write override");
    fs::write(instructions_dir.join("extra.md"), "extra").expect("write extra");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, LOCAL_AGENTS_MD_FILENAME, "override", Vec::new())
    );
}

#[tokio::test]
async fn empty_agents_files_fall_back_to_instructions_directory() {
    let home = TempDir::new().expect("temp dir");
    let instructions_dir = home.path().join(INSTRUCTIONS_DIR_NAME);
    fs::create_dir(&instructions_dir).expect("create instructions dir");
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), " \n\t").expect("write default");
    fs::write(home.path().join(LOCAL_AGENTS_MD_FILENAME), " \n\t").expect("write override");
    fs::write(instructions_dir.join("fallback.md"), "fallback").expect("write fallback");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, "instructions/fallback.md", "fallback", Vec::new())
    );
}

#[cfg(unix)]
#[tokio::test]
async fn instructions_directory_ignores_symlinked_files_and_directories() {
    let home = TempDir::new().expect("temp dir");
    let instructions_dir = home.path().join(INSTRUCTIONS_DIR_NAME);
    let linked_dir = home.path().join("linked");
    fs::create_dir(&instructions_dir).expect("create instructions dir");
    fs::create_dir(&linked_dir).expect("create linked dir");
    fs::write(instructions_dir.join("real.md"), "real").expect("write real");
    fs::write(linked_dir.join("from-linked-dir.md"), "linked dir").expect("write linked dir");
    fs::write(linked_dir.join("linked-file.md"), "linked file").expect("write linked file");
    std::os::unix::fs::symlink(&linked_dir, instructions_dir.join("linked-dir"))
        .expect("create directory symlink");
    std::os::unix::fs::symlink(
        linked_dir.join("linked-file.md"),
        instructions_dir.join("linked-file.md"),
    )
    .expect("create file symlink");

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, "instructions/real.md", "real", Vec::new())
    );
}

#[tokio::test]
async fn recoverable_override_read_error_warns_and_falls_back_to_default() {
    let home = TempDir::new().expect("temp dir");
    let override_path = home.path().join(LOCAL_AGENTS_MD_FILENAME);
    create_symlink_loop(&override_path);
    fs::write(home.path().join(DEFAULT_AGENTS_MD_FILENAME), "default").expect("write default");
    let read_error = fs::read(&override_path).expect_err("symlink loop should not be readable");
    let warning = format!(
        "Failed to read global AGENTS.md instructions from `{}`: {read_error}",
        override_path.display()
    );

    assert_eq!(
        provider(&home).load_user_instructions().await,
        expected(&home, DEFAULT_AGENTS_MD_FILENAME, "default", vec![warning])
    );
}

#[tokio::test]
async fn invalid_utf8_is_lossy() {
    let home = TempDir::new().expect("temp dir");
    let path = home.path().join(DEFAULT_AGENTS_MD_FILENAME);
    let mut invalid_utf8 = b"global".to_vec();
    invalid_utf8.push(0xff);
    invalid_utf8.extend_from_slice(b" doc");
    fs::write(&path, &invalid_utf8).expect("write invalid utf-8");

    let outcome = provider(&home).load_user_instructions().await;
    assert_eq!(
        outcome,
        expected(
            &home,
            DEFAULT_AGENTS_MD_FILENAME,
            "global\u{fffd} doc",
            Vec::new()
        )
    );
}
