use super::run_from_args;
use crate::READ_FILE_OPERATION_ARG2;
use pretty_assertions::assert_eq;
use std::io::Cursor;
use tempfile::tempdir;

#[test]
fn run_from_args_streams_file_bytes_to_stdout() {
    let tempdir = tempdir().expect("tempdir");
    let path = tempdir.path().join("image.bin");
    let expected = b"hello\x00world".to_vec();
    std::fs::write(&path, &expected).expect("write test file");

    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_from_args(
        [
            READ_FILE_OPERATION_ARG2,
            path.to_str().expect("utf-8 test path"),
        ]
        .into_iter()
        .map(Into::into),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    )
    .expect("read should succeed");

    assert_eq!(stdout, expected);
    assert_eq!(stderr, Vec::<u8>::new());
}

#[test]
#[cfg(unix)]
fn rejects_path_that_is_not_a_regular_file() {
    let path = std::path::PathBuf::from("/dev/zero");

    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = run_from_args(
        [
            READ_FILE_OPERATION_ARG2,
            path.to_str().expect("utf-8 test path"),
        ]
        .into_iter()
        .map(Into::into),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    )
    .expect_err(
        r#"reading a non-regular file should fail or else
the user risks hanging the process by trying
to read from something like /dev/zero"#,
    );

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(stdout, Vec::<u8>::new());
    assert_eq!(
        "error: `/dev/zero` is not a regular file\n",
        String::from_utf8_lossy(&stderr),
    );
}

#[test]
fn read_reports_directory_error() {
    let tempdir = tempdir().expect("tempdir");
    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let error = run_from_args(
        [
            READ_FILE_OPERATION_ARG2,
            tempdir.path().to_str().expect("utf-8 test path"),
        ]
        .into_iter()
        .map(Into::into),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    )
    .expect_err("reading a directory should fail");

    #[cfg(not(windows))]
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    #[cfg(windows)]
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

#[test]
fn run_from_args_serializes_errors_to_stderr() {
    let tempdir = tempdir().expect("tempdir");
    let missing = tempdir.path().join("missing.txt");
    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let result = run_from_args(
        [
            READ_FILE_OPERATION_ARG2,
            missing.to_str().expect("utf-8 test path"),
        ]
        .into_iter()
        .map(Into::into),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    );

    assert!(result.is_err(), "missing file should fail");
    assert_eq!(stdout, Vec::<u8>::new());
}

#[test]
fn run_from_args_streams_stdin_bytes_to_file() {
    let tempdir = tempdir().expect("tempdir");
    let path = tempdir.path().join("image.bin");
    let expected = b"hello\x00world".to_vec();

    let mut stdin = Cursor::new(expected.clone());
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_from_args(
        ["write", path.to_str().expect("utf-8 test path")]
            .into_iter()
            .map(Into::into),
        &mut stdin,
        &mut stdout,
        &mut stderr,
    )
    .expect("write should succeed");

    assert_eq!(std::fs::read(&path).expect("read test file"), expected);
    assert_eq!(stdout, Vec::<u8>::new());
    assert_eq!(stderr, Vec::<u8>::new());
}

#[test]
fn write_reports_directory_error() {
    let tempdir = tempdir().expect("tempdir");
    let mut stdin = Cursor::new(b"hello world".to_vec());
    let mut stdout = Vec::new();

    let error = execute(
        FsCommand::WriteFile {
            path: tempdir.path().to_path_buf(),
        },
        &mut stdin,
        &mut stdout,
    )
    .expect_err("writing to a directory should fail");

    #[cfg(target_os = "windows")]
    assert_eq!(error.kind, FsErrorKind::PermissionDenied);
    #[cfg(not(target_os = "windows"))]
    assert_eq!(error.kind, FsErrorKind::IsADirectory);
}
