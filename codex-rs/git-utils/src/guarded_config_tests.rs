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

#[test]
fn frozen_apply_policy_overrides_later_repository_config_changes() {
    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "codex@example.com"]);
    run_git(root, &["config", "user.name", "Codex"]);
    run_git(root, &["config", "core.autocrlf", "false"]);
    run_git(root, &["config", "apply.whitespace", "warn"]);
    run_git(root, &["config", "apply.ignoreWhitespace", "change"]);
    run_git(root, &["config", "core.whitespace", "-trailing-space"]);
    std::fs::write(root.join("file.txt"), "old\n").expect("write base");
    run_git(root, &["add", "file.txt"]);
    run_git(root, &["commit", "-qm", "base"]);
    let patch_dir = tempfile::tempdir().expect("patch directory");
    let patch = patch_dir.path().join("change.diff");
    std::fs::write(
        &patch,
        "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new \n",
    )
    .expect("write patch");

    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config");
    config.freeze_apply_policy().expect("freeze apply policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("authorize filter path");
    let patch = patch.to_str().expect("UTF-8 patch path");
    let (rendered, gate) = config
        .run_apply_policy_gate(/*revert*/ false, patch)
        .expect("run policy gate");
    assert!(rendered.contains("apply.whitespace=warn"), "{rendered}");
    assert!(
        rendered.contains("apply.ignoreWhitespace=change"),
        "{rendered}"
    );
    assert!(
        rendered.contains("core.whitespace=-trailing-space"),
        "{rendered}"
    );
    assert!(
        gate.status.success(),
        "{}",
        String::from_utf8_lossy(&gate.stderr)
    );

    run_git(root, &["config", "apply.whitespace", "error"]);
    run_git(root, &["config", "apply.ignoreWhitespace", "false"]);
    run_git(root, &["config", "core.whitespace", "trailing-space"]);

    let final_apply = config
        .run_direct_apply(/*revert*/ false, patch)
        .expect("run final apply");
    assert!(
        final_apply.status.success(),
        "frozen policy must survive config mutation: {}",
        String::from_utf8_lossy(&final_apply.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read result"),
        "new \n"
    );
}

#[test]
fn frozen_apply_policy_materializes_git_defaults() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");
    config.freeze_apply_policy().expect("freeze apply policy");

    let rendered = config
        .render_apply_command_for_log(&["apply".to_string(), "--check".to_string()])
        .expect("render frozen defaults");
    assert!(rendered.contains("apply.whitespace=warn"), "{rendered}");
    assert!(
        rendered.contains("apply.ignoreWhitespace=false"),
        "{rendered}"
    );
    assert!(
        rendered.contains("core.whitespace=blank-at-eol,blank-at-eof,space-before-tab"),
        "{rendered}"
    );
}

#[test]
fn apply_whitespace_modes_are_typed_and_case_sensitive_at_the_gate() {
    for (value, expected) in [
        ("warn", ApplyWhitespaceMode::Warn),
        ("nowarn", ApplyWhitespaceMode::Nowarn),
        ("error", ApplyWhitespaceMode::Error),
        ("error-all", ApplyWhitespaceMode::ErrorAll),
        ("fix", ApplyWhitespaceMode::Fix),
        ("strip", ApplyWhitespaceMode::Fix),
        ("ERROR", ApplyWhitespaceMode::Invalid),
        ("unknown", ApplyWhitespaceMode::Invalid),
    ] {
        assert_eq!(ApplyWhitespaceMode::normalize(value), expected, "{value}");
    }

    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "codex@example.com"]);
    run_git(root, &["config", "user.name", "Codex"]);
    run_git(root, &["config", "apply.whitespace", "ERROR"]);
    std::fs::write(root.join("file.txt"), "old\n").expect("write base");
    run_git(root, &["add", "file.txt"]);
    run_git(root, &["commit", "-qm", "base"]);
    let patch_dir = tempfile::tempdir().expect("patch directory");
    let patch = patch_dir.path().join("change.diff");
    std::fs::write(
        &patch,
        "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n",
    )
    .expect("write patch");

    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config");
    config.freeze_apply_policy().expect("freeze invalid policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("authorize filter path");
    assert!(
        config
            .final_apply_whitespace_mode(
                /*revert*/ false,
                patch.to_str().expect("UTF-8 patch path"),
            )
            .is_err()
    );
    let (_, gate) = config
        .run_apply_policy_gate(
            /*revert*/ false,
            patch.to_str().expect("UTF-8 patch path"),
        )
        .expect("run invalid policy gate");
    assert!(!gate.status.success());
    assert!(
        String::from_utf8_lossy(&gate.stderr).contains("unrecognized whitespace"),
        "{}",
        String::from_utf8_lossy(&gate.stderr)
    );
    assert!(
        config
            .final_apply_whitespace_mode(
                /*revert*/ false,
                patch.to_str().expect("UTF-8 patch path"),
            )
            .is_err()
    );
}

#[test]
fn successful_apply_policy_gate_is_one_shot_and_bound_to_patch_orientation() {
    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "codex@example.com"]);
    run_git(root, &["config", "user.name", "Codex"]);
    run_git(root, &["config", "apply.whitespace", "error"]);
    std::fs::write(root.join("file.txt"), "old\n").expect("write base");
    run_git(root, &["add", "file.txt"]);
    run_git(root, &["commit", "-qm", "base"]);
    let patch_dir = tempfile::tempdir().expect("patch directory");
    let patch = patch_dir.path().join("change.diff");
    std::fs::write(
        &patch,
        "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n",
    )
    .expect("write patch");
    let patch = patch.to_str().expect("UTF-8 patch path");

    let git = GitRunner::for_cwd_io(root).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config");
    config.freeze_apply_policy().expect("freeze policy");
    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("authorize filter path");
    let (_, gate) = config
        .run_apply_policy_gate(/*revert*/ false, patch)
        .expect("run successful policy gate");
    assert!(gate.status.success());
    assert_eq!(
        config
            .final_apply_whitespace_mode(/*revert*/ false, patch)
            .expect("matching proof"),
        ApplyWhitespaceMode::Error
    );
    assert!(
        config
            .final_apply_whitespace_mode(/*revert*/ true, patch)
            .is_err()
    );
    assert!(
        config
            .final_apply_whitespace_mode(/*revert*/ false, "different.patch")
            .is_err()
    );
    assert!(
        config
            .run_apply_policy_gate(/*revert*/ false, patch)
            .is_err()
    );
}

#[test]
fn attached_filter_snapshots_retain_multiple_owned_overrides() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    std::fs::write(repo.path().join("second.txt"), "second\n").expect("write sink input");
    run_git(
        repo.path(),
        &["config", "filter.demo.clean", "git hash-object --stdin"],
    );
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");

    config
        .authorize_filter_paths(&["first.txt".to_string()])
        .expect("first filter snapshot");
    config
        .authorize_git_add_filter_paths(&["second.txt".to_string()])
        .expect("second filter snapshot");

    assert_eq!(config.filters.len(), 2);
    assert!(
        config
            .filters
            .iter()
            .all(|filter| filter.neutralizer().is_some())
    );

    let apply_include = config.filters[0]
        .neutralizer()
        .expect("apply overlay")
        .include_arg
        .clone();
    let staging_include = config.filters[1]
        .neutralizer()
        .expect("staging overlay")
        .include_arg
        .clone();
    assert_ne!(apply_include, staging_include);
    let apply_path = PathBuf::from(
        apply_include
            .strip_prefix("include.path=")
            .expect("apply include path"),
    );
    let staging_path = PathBuf::from(
        staging_include
            .strip_prefix("include.path=")
            .expect("staging include path"),
    );
    assert!(apply_path.is_file());
    assert!(staging_path.is_file());

    let rendered = config
        .render_command_for_log(&["update-index".to_string()])
        .expect("render ordered overlays");
    assert!(
        rendered.find(&apply_include).expect("render apply overlay")
            < rendered
                .find(&staging_include)
                .expect("render staging overlay")
    );

    let mut sink = config
        .update_index_literal_pathspecs_command()
        .expect("bound sink");
    sink.disable_optional_locks()
        .args(["--add", "--remove", "--", "second.txt"]);
    let output = sink.output().expect("run sink");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(apply_path.is_file(), "apply overlay must outlive the sink");
    assert!(
        staging_path.is_file(),
        "staging overlay must outlive the sink"
    );

    drop(config);
    assert!(!apply_path.exists());
    assert!(!staging_path.exists());
}

#[test]
fn reverse_staging_subset_proof_requires_exactly_one_apply_snapshot() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");

    let error = config
        .ensure_apply_filter_path_subset(&[])
        .expect_err("zero snapshots must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    config
        .authorize_filter_paths(&["allowed.txt".to_string()])
        .expect("apply snapshot");
    config
        .ensure_apply_filter_path_subset(&["allowed.txt".to_string()])
        .expect("authorized subset");
    let error = config
        .ensure_apply_filter_path_subset(&["outside.txt".to_string()])
        .expect_err("off-universe path must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    config
        .authorize_git_add_filter_paths(&[])
        .expect("Git-add snapshot");
    let error = config
        .ensure_apply_filter_path_subset(&[])
        .expect_err("two snapshots must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn zero_driver_merge_policy_still_occupies_the_single_install_slot() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::tests::zero_driver_merge_policy_still_occupies_the_single_install_slot",
        );
        return;
    }
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");

    let error = config
        .install_three_way_merge_policy(&[])
        .expect_err("merge policy without apply snapshot must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply snapshot");
    config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("zero-driver merge policy");
    assert!(config.merge_policy_installed);
    assert!(config.merge.is_some());

    let error = config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect_err("second merge policy must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn isolated_merge_config_is_not_attached_to_preparatory_commands() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::tests::isolated_merge_config_is_not_attached_to_preparatory_commands",
        );
        return;
    }
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    std::fs::write(repo.path().join("file.txt"), "file\n").expect("write file");
    run_git(
        repo.path(),
        &["config", "filter.demo.clean", "git hash-object --stdin"],
    );
    run_git(repo.path(), &["config", "merge.unused.driver", "false"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let mut config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");

    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply filter policy");
    config
        .install_three_way_merge_policy(&["file.txt".to_string()])
        .expect("merge policy");
    config
        .authorize_git_add_filter_paths(&["file.txt".to_string()])
        .expect("Git-add filter policy");

    let apply_include = config.filters[0]
        .neutralizer()
        .expect("apply filter overlay")
        .include_arg
        .clone();
    let merge_config = config
        .merge_common_config_path()
        .expect("isolated merge config");
    let git_add_include = config.filters[1]
        .neutralizer()
        .expect("Git-add filter overlay")
        .include_arg
        .clone();
    let overlay_paths = [&apply_include, &git_add_include]
        .map(|include| PathBuf::from(include.strip_prefix("include.path=").expect("include path")));

    let rendered = config
        .render_command_for_log(&["ls-files".to_string()])
        .expect("render command");
    let apply_offset = rendered.find(&apply_include).expect("apply include");
    let git_add_offset = rendered.find(&git_add_include).expect("Git-add include");
    assert!(apply_offset < git_add_offset);
    assert!(!rendered.contains(&merge_config.display().to_string()));

    let output = config
        .ls_files_command()
        .expect("bound command")
        .output()
        .expect("execute bound command");
    assert!(output.status.success());
    assert!(overlay_paths.iter().all(|path| path.is_file()));
    assert!(merge_config.is_file());

    drop(config);
    assert!(overlay_paths.iter().all(|path| !path.exists()));
    assert!(!merge_config.exists());
}

#[test]
fn global_builtin_query_is_a_fixed_no_argument_operation() {
    let repo = tempfile::tempdir().expect("repo");
    run_git(repo.path(), &["init", "-q"]);
    let git = GitRunner::for_cwd_io(repo.path()).expect("runner");
    let config =
        GuardedGitConfig::authorize(&git, repo.path(), Vec::new()).expect("authorized config");

    // The capability exposes only this no-argument execution method; callers
    // never receive the global-only command and therefore cannot append a
    // second global option or an external command name.
    let output = config
        .list_builtin_commands()
        .expect("fixed builtin-command query");
    assert!(output.status.success());
    let output = std::str::from_utf8(&output.stdout).expect("UTF-8 builtin names");
    assert!(output.lines().any(|name| name == "add"));
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

#[test]
fn sealed_filter_override_cannot_cross_operation_capabilities() {
    let fixture = tempfile::tempdir().expect("fixture");
    let first_root = fixture.path().join("first");
    let second_root = fixture.path().join("second");
    std::fs::create_dir_all(&first_root).expect("first");
    std::fs::create_dir_all(&second_root).expect("second");
    run_git(&first_root, &["init", "-q"]);
    run_git(&second_root, &["init", "-q"]);
    run_git(
        &first_root,
        &["config", "filter.demo.clean", "git hash-object --stdin"],
    );
    let first_git = GitRunner::for_cwd_io(&first_root).expect("first runner");
    let second_git = GitRunner::for_cwd_io(&second_root).expect("second runner");
    let mut first =
        GuardedGitConfig::authorize(&first_git, &first_root, Vec::new()).expect("first capability");
    let second = GuardedGitConfig::authorize(&second_git, &second_root, Vec::new())
        .expect("second capability");
    first
        .authorize_filter_paths(&["unselected.txt".to_string()])
        .expect("first filter policy");
    let sealed = first.filters[0].neutralizer().expect("first override");

    let error =
        match second.command_for_sentinel_filter_probe(sealed, "set", /*required*/ true) {
            Ok(_) => panic!("accepted another operation's sealed override"),
            Err(error) => error,
        };
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn zero_driver_status_occupies_its_slot_and_excludes_every_mutation_policy() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::tests::zero_driver_status_occupies_its_slot_and_excludes_every_mutation_policy",
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
            "guarded_config::tests::status_replaces_filter_config_with_an_owned_helper_free_context",
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
    let attribute_contents =
        std::fs::read_to_string(&owned_attributes).expect("read owned attributes");
    assert!(attribute_contents.contains("!filter"));

    config.detect_status_fsmonitor_async().await;
    let output = config.status_output_async().await.expect("guarded status");
    assert!(output.status.success());
    assert!(owned_config.is_file(), "owned config must outlive final child");
    assert!(
        owned_attributes.is_file(),
        "owned attributes must outlive final child"
    );
    drop(config);
    assert!(!owned_config.exists(), "owned config must be removed");
    assert!(!owned_attributes.exists(), "owned attributes must be removed");
}
