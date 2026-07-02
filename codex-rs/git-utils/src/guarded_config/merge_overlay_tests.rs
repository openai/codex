use super::*;
use crate::git_command::GitRunner;
use crate::safe_git::isolate_git_command_environment;
use pretty_assertions::assert_eq;

fn run_git(cwd: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command.current_dir(cwd).args(args).output().expect("Git");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sealed_merge_override_rejects_another_operation_identity() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    run_git(repo.path(), &["config", "merge.unused.driver", "false"]);
    let first_git = GitRunner::for_cwd_io(repo.path()).expect("first runner");
    let second_git = GitRunner::for_cwd_io(repo.path()).expect("second runner");
    let mut first =
        GuardedGitConfig::authorize(&first_git, repo.path(), Vec::new()).expect("first config");
    let mut second =
        GuardedGitConfig::authorize(&second_git, repo.path(), Vec::new()).expect("second config");
    first
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("first apply snapshot");
    second
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("second apply snapshot");

    let snapshot = first.read_merge_config_snapshot().expect("merge snapshot");
    let neutralizer = first
        .build_merge_override(&snapshot)
        .expect("build merge override")
        .expect("physical merge override");
    let error = second
        .attach_merge_override(Some(neutralizer))
        .expect_err("cross-operation override must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}
