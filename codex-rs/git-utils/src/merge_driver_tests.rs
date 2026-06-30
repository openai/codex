use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::git_config::GitConfigScope;
use std::ffi::OsStr;

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
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "new\n"
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

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow clean patch with selected merge driver");

    assert_eq!(result.exit_code, 0);
    assert!(!result.cmd_for_log.contains("--3way"));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "new\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = run(root, &["git", "status", "--porcelain"]);
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
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "old\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "merge driver must not run");
    let (status_code, status, status_err) = run(root, &["git", "status", "--porcelain"]);
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

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
        revert: false,
        preflight: true,
    })
    .expect("preflight must not probe merge drivers");
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read file"),
        "old\n"
    );
    let (marker_code, _, _) = run(root, &["git", "config", "--get", "codex.mergeran"]);
    assert_ne!(marker_code, 0, "preflight must not run merge driver");
}

#[test]
fn selected_driver_policy_allows_unused_global_and_rejects_selected() {
    let entries = config_entries([
        ("merge.unused.driver", "true"),
        ("merge.selected.driver", "helper %A %B"),
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
fn selected_driver_policy_rejects_untrusted_scope_even_when_unused() {
    let mut entries = config_entries([("merge.local.driver", "helper")]);
    entries
        .get_mut("merge.local.driver")
        .expect("local driver")
        .scope = GitConfigScope::Local;
    let attributes = BTreeMap::from([("file.txt".to_string(), "other".to_string())]);
    assert_eq!(
        untrusted_driver_selection(&entries, &attributes).expect("local driver"),
        Some(("local".to_string(), "<Git config>".to_string()))
    );
}

#[test]
fn merge_attribute_parser_is_strict() {
    let paths = vec!["a.txt".to_string(), "b.txt".to_string()];
    let parsed =
        parse_merge_attributes(b"a.txt\0merge\0unspecified\0b.txt\0merge\0custom\0", &paths)
            .expect("parse attributes");
    assert_eq!(parsed.get("a.txt").map(String::as_str), Some("unspecified"));
    assert_eq!(parsed.get("b.txt").map(String::as_str), Some("custom"));

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
                    origin: "file:/tmp/global.gitconfig".to_string(),
                    key: key.to_string(),
                    value: value.to_string(),
                },
            )
        })
        .collect()
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
