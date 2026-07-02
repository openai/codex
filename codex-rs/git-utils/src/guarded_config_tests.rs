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
fn composed_staging_subset_proof_requires_exactly_one_apply_snapshot() {
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
        .install_three_way_merge_policy()
        .expect_err("merge policy without apply snapshot must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    config
        .authorize_filter_paths(&["file.txt".to_string()])
        .expect("apply snapshot");
    config
        .install_three_way_merge_policy()
        .expect("zero-driver merge policy");
    assert!(config.merge_policy_installed);
    assert!(config.merge.is_none());

    let error = config
        .install_three_way_merge_policy()
        .expect_err("second merge policy must refuse");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}

#[test]
fn command_order_is_apply_filter_then_merge_then_git_add_filter() {
    if std::env::var_os("CODEX_GIT_UTILS_GUARDED_CONFIG_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "guarded_config::tests::command_order_is_apply_filter_then_merge_then_git_add_filter",
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
        .install_three_way_merge_policy()
        .expect("merge policy");
    config
        .authorize_git_add_filter_paths(&["file.txt".to_string()])
        .expect("Git-add filter policy");

    let apply_include = config.filters[0]
        .neutralizer()
        .expect("apply filter overlay")
        .include_arg
        .clone();
    let merge_include = config
        .merge_include_arg()
        .expect("merge overlay")
        .to_string();
    let git_add_include = config.filters[1]
        .neutralizer()
        .expect("Git-add filter overlay")
        .include_arg
        .clone();
    let overlay_paths = [&apply_include, &merge_include, &git_add_include]
        .map(|include| PathBuf::from(include.strip_prefix("include.path=").expect("include path")));

    let rendered = config
        .render_command_for_log(&["ls-files".to_string()])
        .expect("render command");
    let apply_offset = rendered.find(&apply_include).expect("apply include");
    let merge_offset = rendered.find(&merge_include).expect("merge include");
    let git_add_offset = rendered.find(&git_add_include).expect("Git-add include");
    assert!(apply_offset < merge_offset && merge_offset < git_add_offset);
    assert_eq!(rendered.matches(&merge_include).count(), 1);

    let output = config
        .ls_files_command()
        .expect("bound command")
        .output()
        .expect("execute bound command");
    assert!(output.status.success());
    assert!(overlay_paths.iter().all(|path| path.is_file()));

    drop(config);
    assert!(overlay_paths.iter().all(|path| !path.exists()));
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
