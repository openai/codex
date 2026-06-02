use super::write_last_message_to_path;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn writes_last_message_to_regular_file() {
    let temp_dir = tempdir().expect("tempdir");
    let output_path = temp_dir
        .path()
        .canonicalize()
        .expect("canonical tempdir")
        .join("output.md");

    write_last_message_to_path(&output_path, "hello").expect("write output");

    assert_eq!(
        std::fs::read_to_string(&output_path).expect("read output"),
        "hello"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_last_message_path() {
    let temp_dir = tempdir().expect("tempdir");
    let temp_dir = temp_dir.path().canonicalize().expect("canonical tempdir");
    let target_path = temp_dir.join("target.md");
    let output_path = temp_dir.join("output.md");
    std::fs::write(&target_path, "original").expect("write target");
    std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

    let err = write_last_message_to_path(&output_path, "hello").expect_err("symlink should fail");

    assert_eq!(err.raw_os_error(), Some(libc::ELOOP));
    assert_eq!(
        std::fs::read_to_string(&target_path).expect("read target"),
        "original"
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_last_message_parent_directory() {
    let temp_dir = tempdir().expect("tempdir");
    let temp_dir = temp_dir.path().canonicalize().expect("canonical tempdir");
    let target_dir = temp_dir.join("target");
    let symlinked_dir = temp_dir.join("link");
    std::fs::create_dir(&target_dir).expect("create target directory");
    let target_path = target_dir.join("output.md");
    std::fs::write(&target_path, "original").expect("write target");
    std::os::unix::fs::symlink(&target_dir, &symlinked_dir).expect("create symlink");

    write_last_message_to_path(&symlinked_dir.join("output.md"), "hello")
        .expect_err("symlinked parent should fail");

    assert_eq!(
        std::fs::read_to_string(&target_path).expect("read target"),
        "original"
    );
}
