use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::git_command::GitRunner;
use crate::git_config::GitConfigOrigin;
use crate::git_config::GitConfigScope;
use crate::guarded_config::GuardedGitConfig;
use crate::guarded_config::config_source_authorization_count;
use crate::guarded_config::merge_attribute_read_count;
use crate::guarded_config::merge_config_read_count;
use crate::guarded_config::merge_overlay_count;
use crate::guarded_config::reset_config_source_authorization_count;
use crate::guarded_config::reset_merge_policy_counts;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::path::Path;

#[test]
fn apply_allows_unused_global_merge_driver() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[merge \"unused\"]\n\tdriver = git config codex.mergeran true && false\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_merge_test(
            "merge_driver::tests::apply_allows_unused_global_merge_driver",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add file: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "file"]);
    assert_eq!(commit_code, 0, "commit file: {commit_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow unrelated global merge driver");
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt"))
            .expect("read file")
            .replace("\r\n", "\n"),
        "new\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "unused merge driver must not run");
}

#[test]
fn apply_rejects_global_merge_driver_before_three_way() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[merge \"codex-test\"]\n\tdriver = git config codex.mergeran true && false\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_merge_test(
            "merge_driver::tests::apply_rejects_global_merge_driver_before_three_way",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add base: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base: {commit_err}");
    let (base_code, base, base_err) = run(root, &["git", "rev-parse", "HEAD"]);
    assert_eq!(base_code, 0, "resolve base: {base_err}");

    std::fs::write(root.join("file.txt"), "theirs\n").expect("write theirs");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add theirs: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "theirs"]);
    assert_eq!(commit_code, 0, "commit theirs: {commit_err}");
    let (diff_code, diff, diff_err) = run(
        root,
        &[
            "git",
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    );
    assert_eq!(diff_code, 0, "create full-index patch: {diff_err}");

    let (checkout_code, _, checkout_err) =
        run(root, &["git", "checkout", "-b", "ours", base.trim()]);
    assert_eq!(checkout_code, 0, "checkout base: {checkout_err}");
    std::fs::write(root.join("file.txt"), "ours\n").expect("write ours");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-am", "ours"]);
    assert_eq!(commit_code, 0, "commit ours: {commit_err}");

    reset_config_source_authorization_count();
    reset_merge_policy_counts();
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect_err("reject global merge driver");
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(merge_config_read_count(), 1);
    assert_eq!(merge_attribute_read_count(), 1);
    assert_eq!(merge_overlay_count(), 0);
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read ours"),
        "ours\n"
    );
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert!(status.is_empty(), "worktree/index changed: {status}");
}

#[test]
fn three_way_apply_allows_unrelated_local_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add base: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base: {commit_err}");

    std::fs::write(root.join("file.txt"), "base\npatched\n").expect("write patch side");
    let (diff_code, diff, diff_err) = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
    assert_eq!(diff_code, 0, "create patch: {diff_err}");
    let (restore_code, _, restore_err) = run(root, &["git", "checkout", "--", "file.txt"]);
    assert_eq!(restore_code, 0, "restore base: {restore_err}");

    std::fs::write(root.join("file.txt"), "current\nbase\n").expect("write current side");
    let (stage_code, _, stage_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(stage_code, 0, "stage current side: {stage_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.unused.driver",
            "git config codex.mergeran true && false",
        ],
    );
    assert_eq!(
        config_code, 0,
        "configure unused local driver: {config_err}"
    );

    reset_config_source_authorization_count();
    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow unrelated local merge driver during three-way fallback");
    assert_eq!(config_source_authorization_count(), 1);
    assert_eq!(merge_config_read_count(), 1);
    assert_eq!(merge_attribute_read_count(), 1);
    assert_eq!(merge_overlay_count(), 1);
    assert_eq!(result.exit_code, 0, "three-way apply: {}", result.stderr);
    assert!(result.cmd_for_log.contains("--3way"));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt"))
            .expect("read merged file")
            .replace("\r\n", "\n"),
        "current\nbase\npatched\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "unused merge driver must not run");
}

#[test]
fn apply_allows_clean_patch_with_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");

    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow clean patch with selected merge driver");

    assert_eq!(result.exit_code, 0);
    assert_eq!(merge_config_read_count(), 0);
    assert_eq!(merge_attribute_read_count(), 0);
    assert_eq!(merge_overlay_count(), 0);
    assert!(!result.cmd_for_log.contains("--3way"));
    let contents = std::fs::read_to_string(root.join("file.txt")).expect("read file");
    assert!(
        matches!(contents.as_str(), "new\n" | "new\r\n"),
        "expected the patched contents with a platform line ending, got {contents:?}"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert_eq!(
        status.trim(),
        "M  file.txt",
        "patch should update the index"
    );
}

#[test]
fn reverse_apply_allows_clean_patch_with_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");
    let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let patch_dir = tempfile::tempdir().expect("patch tempdir");
    let patch = patch_dir.path().join("change.diff");
    std::fs::write(&patch, diff).expect("write patch");
    let (apply_code, _, apply_err) = run(
        root,
        &[
            "git",
            "apply",
            "--index",
            patch.to_str().expect("patch path"),
        ],
    );
    assert_eq!(apply_code, 0, "prepare forward apply: {apply_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.to_string(),
        revert: true,
        preflight: false,
    })
    .expect("allow clean reverse patch with selected merge driver");

    assert_eq!(result.exit_code, 0);
    assert!(!result.cmd_for_log.contains("--3way"));
    let contents = std::fs::read_to_string(root.join("file.txt")).expect("read file");
    assert!(
        matches!(contents.as_str(), "old\n" | "old\r\n"),
        "expected the reverted contents with a platform line ending, got {contents:?}"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = status_with_apply_config(root);
    assert_eq!(status_code, 0, "status: {status_err}");
    assert!(
        status.is_empty(),
        "reverse apply should restore HEAD: {status}"
    );
}

#[test]
fn preflight_does_not_probe_or_run_selected_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=codex-test\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.codex-test.driver",
            "git config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");

    reset_merge_policy_counts();
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: true,
    })
    .expect("preflight must not probe merge drivers");
    assert_eq!(result.exit_code, 0);
    assert_eq!(merge_config_read_count(), 0);
    assert_eq!(merge_attribute_read_count(), 0);
    assert_eq!(merge_overlay_count(), 0);
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "old\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "preflight must not run merge driver");
}

#[test]
fn merge_override_rejects_process_temp_directory_inside_worktree() {
    let root_name = "CODEX_GIT_UTILS_MERGE_WORKTREE_TMP_ROOT";
    if let Some(root) = std::env::var_os(root_name) {
        let root = std::path::PathBuf::from(root);
        let patch = std::fs::read_to_string(root.join("fixture.patch")).expect("read patch");
        let before_tree = run(&root, &["git", "write-tree"]).1;
        let before_contents = std::fs::read_to_string(root.join("file.txt")).expect("read ours");
        let error = apply_git_patch(&ApplyGitRequest {
            cwd: root.clone(),
            diff: patch,
            revert: false,
            preflight: false,
        })
        .expect_err("reject worktree-owned merge override");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(run(&root, &["git", "write-tree"]).1, before_tree);
        assert_eq!(
            std::fs::read_to_string(root.join("file.txt")).expect("read after"),
            before_contents
        );
        let (marker_code, _, _) = run(&root, &["git", "config", "--get", "codex.mergeran"]);
        assert_ne!(marker_code, 0, "merge driver must not run");
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    let patch = build_three_way_fixture(root);
    std::fs::write(root.join("fixture.patch"), patch).expect("write fixture patch");
    let (config_code, _, config_err) = run(
        root,
        &[
            "git",
            "config",
            "merge.unused.driver",
            "git config codex.mergeran true && false",
        ],
    );
    assert_eq!(config_code, 0, "configure merge driver: {config_err}");
    let worktree_temp = root.join("process-temp");
    std::fs::create_dir(&worktree_temp).expect("worktree temp directory");
    run_isolated_merge_test(
        "merge_driver::tests::merge_override_rejects_process_temp_directory_inside_worktree",
        &[
            (root_name, root.as_os_str()),
            ("TMPDIR", worktree_temp.as_os_str()),
            #[cfg(windows)]
            ("TEMP", worktree_temp.as_os_str()),
            #[cfg(windows)]
            ("TMP", worktree_temp.as_os_str()),
        ],
    );
}

#[test]
fn empty_name_driver_follows_effective_git_selection() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "base\n").expect("write file");
    std::fs::write(root.join(".gitattributes"), "file.txt merge=other\n")
        .expect("write attributes");
    let (config_code, _, config_err) = run(
        root,
        &["git", "config", "merge..driver", "empty-name helper"],
    );
    assert_eq!(config_code, 0, "configure empty-name driver: {config_err}");
    let paths = ["file.txt".to_string()];

    install_merge_policy(root, &paths).expect("allow unused empty-name driver");

    std::fs::write(root.join(".gitattributes"), "file.txt merge=\n")
        .expect("select empty-name driver");
    let selected_error =
        install_merge_policy(root, &paths).expect_err("reject selected empty-name driver");
    assert_eq!(selected_error.kind(), io::ErrorKind::Unsupported);

    std::fs::write(root.join(".gitattributes"), "").expect("clear attributes");
    let (default_code, _, default_err) = run(root, &["git", "config", "merge.default", ""]);
    assert_eq!(
        default_code, 0,
        "configure empty merge.default: {default_err}"
    );
    let default_error = install_merge_policy(root, &paths)
        .expect_err("reject empty-name driver selected by empty default");
    assert_eq!(default_error.kind(), io::ErrorKind::Unsupported);

    let (empty_code, _, empty_err) = run(root, &["git", "config", "merge..driver", ""]);
    assert_eq!(empty_code, 0, "empty effective driver: {empty_err}");
    install_merge_policy(root, &paths).expect("allow selected empty command through empty default");
    std::fs::write(root.join(".gitattributes"), "file.txt merge=\n")
        .expect("select empty-name driver");
    install_merge_policy(root, &paths).expect("allow explicitly selected empty command");
}

#[test]
fn selected_driver_policy_allows_unused_global_and_rejects_selected() {
    let mut entries = config_entries([
        ("merge.unused.driver", "true"),
        ("merge.selected.driver", "helper %A %B"),
        ("merge..driver", "empty-name helper"),
        ("merge.default", ""),
    ]);
    let unused = BTreeMap::from([("file.txt".to_string(), "other".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &unused).expect("unused selection"),
        None
    );

    let selected = BTreeMap::from([("file.txt".to_string(), "selected".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected).expect("selected driver"),
        Some(("selected".to_string(), "file.txt".to_string()))
    );

    let selected_empty_name = BTreeMap::from([("file.txt".to_string(), String::new())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_empty_name)
            .expect("selected empty-name driver"),
        Some((String::new(), "file.txt".to_string()))
    );

    let selected_by_empty_default =
        BTreeMap::from([("file.txt".to_string(), "unspecified".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_by_empty_default)
            .expect("empty default selection"),
        Some((String::new(), "file.txt".to_string()))
    );

    entries
        .get_mut("merge..driver")
        .expect("empty-name driver")
        .value
        .clear();
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_empty_name).expect("empty selected command"),
        None
    );
    assert_eq!(
        untrusted_driver_selection(&entries, &selected_by_empty_default)
            .expect("empty defaulted command"),
        None
    );
}

#[test]
fn selected_driver_policy_handles_default_and_sentinel_ambiguity() {
    let mut entries = config_entries([
        ("merge.default", "defaulted"),
        ("merge.defaulted.driver", "helper"),
        ("merge.set.driver", "set helper"),
        ("merge.unset.driver", "unset helper"),
        ("merge.unspecified.driver", "unspecified helper"),
    ]);
    for value in ["set", "unset", "unspecified"] {
        let attributes = BTreeMap::from([("file.txt".to_string(), value.to_string())]);
        assert!(
            untrusted_driver_selection(&entries, &attributes)
                .expect("sentinel selection")
                .is_some(),
            "{value}"
        );
    }

    entries
        .get_mut("merge.unspecified.driver")
        .expect("unspecified driver")
        .value
        .clear();
    let attributes = BTreeMap::from([("file.txt".to_string(), "unspecified".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &attributes).expect("merge.default selection"),
        Some(("defaulted".to_string(), "file.txt".to_string()))
    );
}

#[test]
fn selected_driver_policy_ignores_scope_until_driver_is_selected() {
    for scope in [
        GitConfigScope::System,
        GitConfigScope::Global,
        GitConfigScope::Local,
        GitConfigScope::Worktree,
        GitConfigScope::Command,
    ] {
        let mut entries = config_entries([("merge.scoped.driver", "helper")]);
        entries
            .get_mut("merge.scoped.driver")
            .expect("scoped driver")
            .scope = scope;
        let attributes = BTreeMap::from([("file.txt".to_string(), "other".to_string())]);
        assert_eq!(
            untrusted_driver_selection(&entries, &attributes).expect("unused scoped driver"),
            None,
            "{scope:?}"
        );
        let attributes = BTreeMap::from([("file.txt".to_string(), "scoped".to_string())]);
        assert_eq!(
            untrusted_driver_selection(&entries, &attributes).expect("selected scoped driver"),
            Some(("scoped".to_string(), "file.txt".to_string())),
            "{scope:?}"
        );
    }
}

#[test]
fn merge_attribute_parser_is_strict() {
    let paths = vec!["a.txt".to_string(), "b.txt".to_string()];
    let parsed = parse_merge_attributes(b"a.txt\0merge\0unspecified\0b.txt\0merge\0\0", &paths)
        .expect("parse attributes");
    assert_eq!(parsed.get("a.txt").map(String::as_str), Some("unspecified"));
    assert_eq!(parsed.get("b.txt").map(String::as_str), Some(""));

    for malformed in [
        b"a.txt\0merge\0unspecified".as_slice(),
        b"a.txt\0filter\0unspecified\0b.txt\0merge\0custom\0".as_slice(),
        b"a.txt\0merge\0unspecified\0".as_slice(),
        b"a.txt\0merge\0unspecified\0a.txt\0merge\0custom\0".as_slice(),
    ] {
        assert!(
            parse_merge_attributes(malformed, &paths).is_err(),
            "{malformed:?}"
        );
    }
}

fn config_entries<const N: usize>(values: [(&str, &str); N]) -> BTreeMap<String, GitConfigEntry> {
    values
        .into_iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                GitConfigEntry {
                    scope: GitConfigScope::Global,
                    origin: GitConfigOrigin::File("/tmp/global.gitconfig".into()),
                    key: key.to_string(),
                    value: value.to_string(),
                },
            )
        })
        .collect()
}

fn install_merge_policy(root: &Path, paths: &[String]) -> io::Result<()> {
    let git = GitRunner::for_cwd_io(root)?;
    let mut config = GuardedGitConfig::authorize(&git, root, Vec::new())?;
    config.authorize_filter_paths(paths)?;
    config.install_three_way_merge_policy()
}

fn build_three_way_fixture(root: &Path) -> String {
    std::fs::write(root.join("file.txt"), "base\n").expect("write base");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    let base = run(root, &["git", "rev-parse", "HEAD"]).1;
    std::fs::write(root.join("file.txt"), "theirs\n").expect("write theirs");
    assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "theirs"]).0, 0);
    let patch = run(
        root,
        &[
            "git",
            "diff",
            "--full-index",
            base.trim(),
            "HEAD",
            "--",
            "file.txt",
        ],
    );
    assert_eq!(patch.0, 0, "create patch: {}", patch.2);
    assert_eq!(
        run(root, &["git", "checkout", "-b", "ours", base.trim()]).0,
        0
    );
    std::fs::write(root.join("file.txt"), "ours\n").expect("write ours");
    assert_eq!(run(root, &["git", "commit", "-am", "ours"]).0, 0);
    patch.1
}

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new(args[0]);
    isolate_git_command_environment(&mut command);
    let output = command
        .args(&args[1..])
        .current_dir(cwd)
        .output()
        .expect("run command");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn status_with_apply_config(cwd: &Path) -> (i32, String, String) {
    let mut args = vec!["git".to_string()];
    args.extend(crate::apply::configured_git_config_parts());
    args.extend(["status".to_string(), "--porcelain".to_string()]);
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run(cwd, &args)
}

fn run_isolated_merge_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_MERGE_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let root = repo.path();
    let _ = run(root, &["git", "init"]);
    let _ = run(root, &["git", "config", "user.email", "codex@example.com"]);
    let _ = run(root, &["git", "config", "user.name", "Codex"]);
    repo
}
