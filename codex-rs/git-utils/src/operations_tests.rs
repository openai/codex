use super::*;
use crate::safe_git::isolate_git_command_environment;
use std::process::Command;

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("tempdir");
    let mut command = Command::new("git");
    isolate_git_command_environment(&mut command);
    let status = command
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("initialize repository");
    assert!(status.success());
    repo
}

#[test]
fn caller_env_cannot_restore_repository_or_pathspec_selectors() {
    let target = init_repo();
    let alternate = init_repo();
    std::fs::write(target.path().join("target.txt"), "target\n").expect("target file");
    std::fs::write(alternate.path().join("alternate.txt"), "alternate\n").expect("alternate file");
    for (repo, path) in [(&target, "target.txt"), (&alternate, "alternate.txt")] {
        let mut command = Command::new("git");
        isolate_git_command_environment(&mut command);
        let status = command
            .args(["add", path])
            .current_dir(repo.path())
            .status()
            .expect("add file");
        assert!(status.success());
    }

    let alternate_git_dir = alternate.path().join(".git");
    let env = [
        (
            OsString::from("GIT_DIR"),
            alternate_git_dir.as_os_str().into(),
        ),
        (
            OsString::from("GIT_WORK_TREE"),
            alternate.path().as_os_str().into(),
        ),
        (
            OsString::from("GIT_COMMON_DIR"),
            alternate_git_dir.as_os_str().into(),
        ),
        (
            OsString::from("GIT_INDEX_FILE"),
            alternate_git_dir.join("index").into_os_string(),
        ),
        (OsString::from("GIT_PREFIX"), OsString::from("elsewhere/")),
        (OsString::from("GIT_LITERAL_PATHSPECS"), OsString::from("1")),
        (OsString::from("GIT_GLOB_PATHSPECS"), OsString::from("1")),
        (OsString::from("GIT_NOGLOB_PATHSPECS"), OsString::from("1")),
        (OsString::from("GIT_ICASE_PATHSPECS"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG"),
            alternate_git_dir.join("config").into_os_string(),
        ),
    ];
    let output = run_git_for_stdout(target.path(), ["ls-files"], Some(&env))
        .expect("query cwd-selected index");
    assert_eq!(output, "target.txt");
}
