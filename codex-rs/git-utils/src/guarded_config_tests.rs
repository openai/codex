use super::*;
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
fn authorization_rejects_untyped_global_config_arguments() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");

    for args in [
        vec!["--config-env=include.path=UNSNAPSHOTTED".to_string()],
        vec!["--git-dir=elsewhere".to_string()],
        vec!["-c".to_string(), "missing-value".to_string()],
        vec!["-c".to_string()],
    ] {
        let error = match GuardedGitConfig::authorize(&git, repo.path(), args) {
            Ok(_) => panic!("accepted untyped base config arguments"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}

#[test]
fn authorization_rejects_another_runner_repository_and_nested_or_prefix_roots() {
    let fixture = tempfile::tempdir().expect("fixture");
    let outer = fixture.path().join("repo");
    let nested = outer.join("nested");
    let prefix = fixture.path().join("repo-evil");
    std::fs::create_dir_all(&nested).expect("nested");
    std::fs::create_dir_all(&prefix).expect("prefix");
    run_git(&outer, &["init", "-q"]);
    run_git(&nested, &["init", "-q"]);
    run_git(&prefix, &["init", "-q"]);
    let git = GitRunner::for_cwd_io(&outer).expect("outer runner");

    for wrong_root in [&nested, &prefix] {
        let error = match GuardedGitConfig::authorize(&git, wrong_root, Vec::new()) {
            Ok(_) => panic!("accepted another repository root"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
