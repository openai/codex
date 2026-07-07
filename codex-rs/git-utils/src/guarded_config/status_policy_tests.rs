use std::io;
use std::path::Path;

use super::super::ApplyPolicyGateState;
use super::super::GuardedGitConfig;
use super::super::config_source_authorization_count;
use super::super::reset_config_source_authorization_count;
use super::super::status_filter_policy_read_count;
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

fn run_isolated_config_test(test_name: &str) {
    let environment = tempfile::tempdir().expect("isolated Git environment");
    let global_config = environment.path().join("global.gitconfig");
    let system_config = environment.path().join("system.gitconfig");
    std::fs::write(&global_config, "").expect("empty global config");
    std::fs::write(&system_config, "").expect("empty system config");

    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    let output = command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_SYSTEM", &system_config)
        .env("RUST_TEST_THREADS", "1")
        .output()
        .expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn zero_driver_status_occupies_its_slot_and_excludes_every_mutation_policy() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::status_policy::tests::zero_driver_status_occupies_its_slot_and_excludes_every_mutation_policy",
        );
        return;
    }
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    reset_config_source_authorization_count();
    crate::git_command::reset_git_runner_construction_count();
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config = GuardedGitConfig::authorize_status_async(&git)
        .await
        .expect("authorized status capability");
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(crate::git_command::git_runner_construction_count(), 1);
    config
        .verify_status_root_async(repo.path())
        .await
        .expect("matching selected-Git root");
    let error = config
        .status_output_async()
        .await
        .expect_err("status sink requires an installed policy");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    config
        .install_status_policy_async()
        .await
        .expect("zero-driver status policy");
    assert_eq!(status_filter_policy_read_count(), 1);
    let status = config.status.as_ref().expect("occupied status slot");
    status
        .context
        .context(&config.identity)
        .expect("sealed zero-driver Status context");
    assert_eq!(status.fsmonitor, None);

    let error = config
        .install_status_policy_async()
        .await
        .expect_err("second status installation must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(status_filter_policy_read_count(), 1);
    let error = config
        .authorize_filter_paths(&[])
        .expect_err("status plus Apply must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let error = config
        .authorize_git_add_filter_paths(&[])
        .expect_err("status plus GitAdd must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let error = config
        .install_three_way_merge_policy(&[])
        .expect_err("status plus merge must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let error = config
        .freeze_apply_policy()
        .expect_err("status plus frozen apply policy must fail");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(config.apply_policy.is_none());
    assert!(matches!(
        config.apply_policy_gate,
        ApplyPolicyGateState::NotRun
    ));
    let error = config
        .status_output_async()
        .await
        .expect_err("status sink requires a retained fsmonitor decision");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    let first = config.detect_status_fsmonitor_async().await;
    let second = config.detect_status_fsmonitor_async().await;
    assert_eq!(first, second);
    let output = config
        .status_output_async()
        .await
        .expect("closed status sink");
    assert!(output.status.success());

    let mut mutation =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("mutation capability");
    mutation
        .authorize_filter_paths(&[])
        .expect("Apply filter policy");
    let reads_before = status_filter_policy_read_count();
    let error = mutation
        .install_status_policy_async()
        .await
        .expect_err("Apply plus status must fail before a status read");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(status_filter_policy_read_count(), reads_before);

    let mut apply_policy_only =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("apply capability");
    apply_policy_only
        .freeze_apply_policy()
        .expect("frozen apply policy");
    let reads_before = status_filter_policy_read_count();
    let error = apply_policy_only
        .install_status_policy_async()
        .await
        .expect_err("frozen apply policy plus status must fail before a status read");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(status_filter_policy_read_count(), reads_before);
    assert!(apply_policy_only.status.is_none());
}

#[tokio::test]
async fn status_replaces_filter_config_with_an_owned_helper_free_context() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::status_policy::tests::status_replaces_filter_config_with_an_owned_helper_free_context",
        );
        return;
    }
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    std::fs::write(repo.path().join("file.txt"), "contents\n").expect("tracked file");
    std::fs::write(repo.path().join(".gitattributes"), "file.txt filter=demo\n")
        .expect("attributes");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["config", "filter.demo.smudge", "cat"]);

    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config = GuardedGitConfig::authorize_status_async(&git)
        .await
        .expect("authorized status capability");
    config
        .verify_status_root_async(repo.path())
        .await
        .expect("matching root");
    config
        .install_status_policy_async()
        .await
        .expect("optional smudge policy");
    let status = config.status.as_ref().expect("Status context");
    let owned_config = status
        .context
        .config_path(&config.identity)
        .expect("owned config path");
    let owned_attributes = status
        .context
        .attributes_path(&config.identity)
        .expect("owned attributes path");
    let config_contents = std::fs::read_to_string(&owned_config).expect("read owned config");
    assert!(!config_contents.contains("filter.demo"));
    assert!(
        !config_contents.contains("symlinks"),
        "unset core.symlinks must remain unset in the owned projection: {config_contents}"
    );
    let attribute_contents =
        std::fs::read_to_string(&owned_attributes).expect("read owned attributes");
    assert!(attribute_contents.contains("!filter"));

    config.detect_status_fsmonitor_async().await;
    let output = config.status_output_async().await.expect("guarded status");
    assert!(output.status.success());
    assert!(
        owned_config.is_file(),
        "owned config must outlive final child"
    );
    assert!(
        owned_attributes.is_file(),
        "owned attributes must outlive final child"
    );
    drop(config);
    assert!(!owned_config.exists(), "owned config must be removed");
    assert!(
        !owned_attributes.exists(),
        "owned attributes must be removed"
    );
}

#[tokio::test]
async fn status_projection_freezes_explicit_core_symlinks_values() {
    for value in ["true", "false"] {
        let repo = tempfile::tempdir().expect("repo");
        run_git(repo.path(), &["init", "-q"]);
        std::fs::write(repo.path().join("file.txt"), "contents\n").expect("tracked file");
        run_git(repo.path(), &["add", "file.txt"]);
        run_git(repo.path(), &["config", "core.symlinks", value]);

        let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
        let mut config = GuardedGitConfig::authorize_status_async(&git)
            .await
            .expect("authorized status capability");
        config
            .verify_status_root_async(repo.path())
            .await
            .expect("matching root");
        config
            .install_status_policy_async()
            .await
            .expect("installed status policy");
        let status = config.status.as_ref().expect("Status context");
        let owned_config = status
            .context
            .config_path(&config.identity)
            .expect("owned config path");
        let contents = std::fs::read_to_string(owned_config).expect("read owned config");
        assert!(
            contents.contains(&format!("symlinks = {value}")),
            "core.symlinks={value} was not frozen: {contents}"
        );
    }
}
